use redis::Commands;
use mongodb::{Client as MongoClient, Collection, bson::doc};
use redis::Client as RedisClient;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use chrono::Datelike;
use reqwest::{Client, header};

pub mod config;

#[derive(Serialize, Deserialize)]
struct Expense {
    time: String,
    location: String,
    amount: f32,
}

#[derive(Serialize, Deserialize)]
struct Trend {
    time: String,
    amount: f32,
}

#[derive(Serialize, Deserialize)]
struct ReportData {
    id: String,
    name: String,
    balance: f32,
    total_expense: f32,
    total_count: i32,
    top_expense: Expense,
    top_count: Expense,
    trend: Vec<Trend>,
    cafeteria_count: i32,
    breakfast: Meal,
    lunch: Meal,
    dinner: Meal,
}

#[derive(Serialize, Deserialize)]
struct Meal {
    count: i32,
    amount: f32,
    location: String,
}


#[tokio::main]
async fn main() {
    let config = config::config::init_config("config.toml").unwrap();
    let mongo_client = MongoClient::with_uri_str(&config.db.url).await.unwrap();
    let mongo_db = mongo_client.database("hust_ledger");
    let mongo_collection: Collection<ReportData> = mongo_db.collection("report");
    let redis_client = RedisClient::open(config.redis.url.as_str()).unwrap();
    let mut redis_connection = redis_client.get_connection().unwrap();

    loop {
        let queue: Vec<String> = redis_connection.keys("request:*").unwrap();
        for key in queue {
            let jsession: String = redis_connection.get(&key).unwrap();
            let jsession: &str = jsession.split(":").collect::<Vec<&str>>()[1];
            let t = key.split(":").collect::<Vec<&str>>();
            let period = t[2];
            let account = t[1];
            let report_data = process(jsession, period, account);
            // let _:() = redis_connection.set(key, "accepted").unwrap();
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}


fn process(jsession: &str, period: &str, account: &str) -> Result<ReportData, Box<dyn std::error::Error>> {
    let api = "http://ecard.m.hust.edu.cn/wechat-web/QueryController/select.html";
    let cookie_store = reqwest::cookie::Jar::default();
	let url = reqwest::Url::parse("http://ecard.m.hust.edu.cn").unwrap();
	cookie_store.add_cookie_str(("JSESSIONID=".to_owned() + &jsession).as_str(), &url);
	let client = Client::builder()
		.cookie_provider(cookie_store.into())
		.default_headers({
			let mut headers = header::HeaderMap::new();
			headers.insert(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3".parse().unwrap());
			headers.insert(header::ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8".parse().unwrap());
			headers.insert(header::ACCEPT_ENCODING, "gzip, deflate, sdch".parse().unwrap());
			headers.insert(header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.8".parse().unwrap());
			headers.insert(header::CONNECTION, "keep-alive".parse().unwrap());
			headers
		})
		.build()
		.unwrap();
    let mut form = std::collections::HashMap::new();
    let date = chrono::Local::now();
    form.insert("account", account);
    form.insert("curpage", "1");
    form.insert("typeStatus", "1");
    match period {
        "week" => process_week(&mut form, client),
        "month" => format!("{}-{:02}-01", date.year(), date.month()).as_str(),
        "year" => format!("{}-01-01", date.year()).as_str(),
        _ => return Err("Invalid period".into()),
    }
}

fn process_week(form: &mut HashMap, client: reqwest::Client) -> Result<ReportData, Box<dyn std::error::Error>> {
    let api = "http://ecard.m.hust.edu.cn/wechat-web/QueryController/select.html";
    let client = reqwest::Client::new();
    let mut form = std::collections::HashMap::new();
    form.insert("account", account);
    form.insert("curpage", "1");
    form.insert("typeStatus", "1");
    let res = client.post(api).form(&form).header("Cookie", format!("JSESSIONID={}", jsession)).send().unwrap();
    let text = res.text().unwrap();
    let data: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut report_data = ReportData {
        id: account.to_string(),
        name: data["data"]["name"].as_str().unwrap().to_string(),
        balance: data["data"]["balance"].as_f64().unwrap() as f32,
        total_expense: 0.0,
        total_count: 0,
        top_expense: Expense {
            time: "".to_string(),
            location: "".to_string(),
            amount: 0.0,
        },
        top_count: Expense {
            time: "".to_string(),
            location: "".to_string(),
            amount: 0.0,
        },
        trend: vec![],
        cafeteria_count: 0,
        breakfast: Meal {
            count: 0,
            amount: 0.0,
            location: "".to_string(),
        },
        lunch: Meal {
            count: 0,
            amount: 0.0,
            location: "".to_string(),
        },
        dinner: Meal {
            count: 0,
            amount: 0.0,
            location: "".to_string(),
        },
    };
    let mut trend = vec![];
    for item in data["data"]["list"].as_array().unwrap() {
        let time = item["time"].as_str().unwrap().to_string();
        let amount = item["amount"].as_str().unwrap().parse::<f32>().unwrap();
        let location = item["location"].as_str().unwrap().to_string();
        let expense = Expense {
            time: time.clone(),
            location: location.clone(),
            amount: amount,
        };
        report_data.total_expense += amount;
        report