use chrono::Datelike as _;
use redis::Commands;
use mongodb::{Client as MongoClient, Collection, bson::doc};
use redis::Client as RedisClient;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use std::collections;
use reqwest::{Client, header};

pub mod config;
pub mod utils;

#[derive(Serialize, Deserialize, Debug)]
struct Expense {
    time: String,
    location: String,
    amount: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct Trend {
    count: i32,
    expense: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Meal {
    count: i32,
    amount: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct Trans{
    location: String,
    amount: f64,
    count: i32,
}


#[derive(Serialize, Deserialize, Debug)]
struct ReportData {
    date: i32,
    balance: f64,
    total_expense: f64,
    total_topup: f64,
    total_count: i32,
    top_expense: Expense,
    top_count: Trans,
    trend: [Trend; 3],
    cafeteria_count: i32,
    breakfast: Meal,
    lunch: Meal,
    dinner: Meal,
    midnight_snack: Meal,
}


#[tokio::main]
async fn main() {
    let config = config::config::init_config("config.toml").unwrap();
    let mongo_client = MongoClient::with_uri_str(&config.db.url).await.unwrap();
    let redis_client = RedisClient::open(config.redis.url.as_str()).unwrap();
    let mut redis_connection = redis_client.get_connection().unwrap();

    loop {
        let queue: Vec<String> = redis_connection.keys("request:*").unwrap();
        for key in queue {
            let castgc: String = redis_connection.get(&key).unwrap();
            let castgc: &str = castgc.split(":").collect::<Vec<&str>>()[1];
            let t = key.split(":").collect::<Vec<&str>>();
            let period = t[2];
            let account = t[1];
            let key_res = format!("result:{}:{}", account, period);
            let _: () = redis_connection.del(&key).unwrap();
            let _:() = match process(castgc, period, account.to_string(), &mongo_client).await{
                Ok(id) => {
                    redis_connection.set(key_res, id).unwrap()
                },
                Err(e) => {
                    redis_connection.set(key_res, format!("error: {}", e)).unwrap()
                }
            };
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}


async fn process(castgc: &str, period: &str, account: String, db: &mongodb::Client) -> Result<String, Box<dyn std::error::Error>> {
    let cookie_store = reqwest::cookie::Jar::default();
    let jsession = utils::hust_login::get_jsession(castgc).await.unwrap();
	let url = reqwest::Url::parse("http://ecard.m.hust.edu.cn").unwrap();
	cookie_store.add_cookie_str(("JSESSIONID=".to_owned()+&jsession).as_str(), &url);
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
    let mut form: collections::HashMap<&str, String> = collections::HashMap::new();
    form.insert("account", account.clone());
    form.insert("curpage", "1".to_string());
    form.insert("typeStatus", "1".to_string());
    let date = chrono::Utc::now();
    let coll: Collection<ReportData> = match period {
        "week" => {
            form.insert("dateStatus", "3".to_string());
            db.database("report_week").collection(account.as_str())
        }
        "month" => {
            let date_status = format!("{}-{:02}-01", date.year().clone(), date.month().clone());
            form.insert("dateStatus", date_status);
            db.database("report_month").collection(account.as_str())
        },
        _ => return Err("Invalid period".into())
    };
    let api = "http://ecard.m.hust.edu.cn/wechat-web/QueryController/select.html";
    let mut trans: collections::HashMap<&str,(i32,f64)> = collections::HashMap::new();
    let mut meals: [Meal; 4] = core::array::from_fn(|_| Meal { count: 0, amount: 0.0 });
    let mut balance: f64 = -1.0;
    let mut total_expense: f64 = 0.0;
    let mut total_topup: f64 = 0.0;
    let mut total_count: i32 = 0;
    let mut top_expense: Expense = Expense {
        time: "1".to_string(),
        location: "1".to_string(),
        amount: 0.0,
    };
    let mut top_count: Trans = Trans {
        location: "1".to_string(),
        amount: 0.0,
        count: 0,
    };
    let mut cafeteria_count: i32 = 0;
    loop{
        let res = client.get(api).query(&form).send().await.unwrap();
        let text = &res.text().await.unwrap();
        let text = &text[9..text.len()-1];
        let data: serde_json::Value = serde_json::from_str(&text).unwrap();
        let data = Box::leak(Box::new(data));
        if balance == -1.0 {
            balance = data["total"][0]["cardbal"].as_str().unwrap().parse::<f64>()? / 100.0;
        }
        if data["retcode"].as_str().unwrap() != "0" {
            return Err(("Card system error: ".to_owned()+data["errmsg"].as_str().unwrap()).into());
        }
        for item in data["total"].as_array().unwrap() {
            if item["sign_tranamt"].as_str().unwrap().parse::<i64>()? > 0 {
                total_topup += item["tranamt"].as_str().unwrap().parse::<f64>()? / 100.0;
                continue;
            }
            let mut occtime = item["occtime"].as_str().unwrap().parse::<i64>()?;
            let mercname = item["mercname"].as_str().unwrap();
            let mut tranamt = item["tranamt"].as_str().unwrap().parse::<f64>()?;
            tranamt /= 100.0;
            total_expense += tranamt;
            total_count += 1;
            if trans.contains_key(mercname){
                let t = trans.get_mut(mercname).unwrap();
                t.0 += 1;
                t.1 += tranamt;
                if t.0 > top_count.count {
                    top_count = Trans {
                        location: mercname.to_string(),
                        amount: t.1,
                        count: t.0,
                    };
                }
            }else{
                trans.insert(mercname, (1, tranamt));
            }

            if tranamt > top_expense.amount {
                top_expense = Expense {
                    time: item["occtime"].as_str().unwrap().to_string(),
                    location: mercname.to_string(),
                    amount: tranamt,
                };
            }

            if mercname.contains("组") || mercname.contains("百景") || mercname.contains("食堂") || mercname.contains("饭") {
                occtime %= 1000000;
                let idx = match occtime {
                    60000..=90000 => 0,
                    110000..=140000 => 1,
                    170000..=200000 => 2,
                    220000..=240000 => 3,
                    _ => continue,
                };
                cafeteria_count += 1;
                meals[idx].count += 1;
                meals[idx].amount += tranamt;
            }
        }
        form.remove("curpage");
        form.insert("curpage", data["nextpage"].as_str().unwrap().to_string());
        if data["nextpage"].as_str().unwrap() == "0" {
            break;
        }
    };

    // Retrieve reports from the previous three weeks
    let mut trend = [
        Trend { count: 0, expense: 0.0 },
        Trend { count: 0, expense: 0.0 },
        Trend { count: 0, expense: 0.0 },
    ];

    let fmtstr = match period {
        "week" => {
            for i in 1..=3 {
                let week_start = date - chrono::Duration::weeks(i);
                let week_id = week_start.format("%Y%U").to_string();
                if let Some(report) = coll.find_one(doc! { "date": week_id }).await.unwrap() {
                    trend[(i-1) as usize] = Trend {
                        count: report.total_count,
                        expense: report.total_expense,
                    };
                }
            };
            "%Y%U"
        },
        "month" => {
            let mut month_start = date.with_day(1).unwrap();
            for i in 1..=3 {
                month_start -= chrono::Duration::days(1);
                month_start = month_start.with_day(1).unwrap();
                let month_id = month_start.format("%Y%m").to_string();
                if let Some(report) = coll.find_one(doc! { "date": month_id }).await.unwrap() {
                    trend[(i-1) as usize] = Trend {
                        count: report.total_count,
                        expense: report.total_expense,
                    };
                }
            };
            "%Y%m"
        },
        _ => return Err("Invalid period".into())
    };

    let result = ReportData {
        date: date.format(fmtstr).to_string().parse().unwrap(),
        balance,
        total_expense,
        total_topup,
        total_count,
        top_expense,
        top_count,
        trend,
        cafeteria_count,
        breakfast: meals[0].clone(),
        lunch: meals[1].clone(),
        dinner: meals[2].clone(),
        midnight_snack: meals[3].clone(),
    };
    let id = coll.insert_one(result).await.unwrap().inserted_id.as_object_id().unwrap().to_hex();
    Ok(format!("report_{}/{}/{}", period, account, id))
}