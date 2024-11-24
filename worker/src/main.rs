use chrono::{Datelike as _, Utc};
use redis::Commands;
use mongodb::{bson::doc, Client as MongoClient, Collection};
use redis::Client as RedisClient;
use serde::{Serialize, Deserialize};
use std::time::Duration;
use std::collections;
use reqwest::{Client, header};
use async_recursion::async_recursion;

pub mod config;
pub mod utils;

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[error("Invalid period: {0}")]
    InvalidPeriod(String),
    #[error("Card system error: {0}")]
    CardSystemError(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] mongodb::error::Error),
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    #[error("Parse int error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("Parse float error: {0}")]
    ParseFloatError(#[from] std::num::ParseFloatError),
}

const REFRESH_INTERVAL: u64 = 5;
const MEAL_TIME_RANGES: [(i64, i64, usize); 4] = [
    (60000, 90000, 0),   // breakfast
    (110000, 140000, 1), // lunch
    (170000, 200000, 2), // dinner
    (220000, 240000, 3), // midnight snack
];
const CAF_NAME: [&str; 14] = ["百惠", "百景", "集锦", "东一", "东二", "东三", "学一", "学二", "喻园", "食堂", "紫荆园", "西一", "西二", "东园"];
const GRO_NAME: [&str; 2] = ["超市", "商店"];

struct RedisConnections {
    main: redis::Connection,
    tag: redis::Connection,
    untagged: redis::Connection,
}

impl RedisConnections {
    fn new(config: &config::config::Config) -> Result<Self, redis::RedisError> {
        Ok(Self {
            main: RedisClient::open(config.redis.url.as_str())?.get_connection()?,
            tag: RedisClient::open(config.tags_db.url.as_str())?.get_connection()?,
            untagged: RedisClient::open(config.tags_db.url.as_str())?.get_connection()?,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Expense {
    time: String,
    location: String,
    amount: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
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
    date: String,
    balance: f64,
    total_expense: f64,
    total_topup: f64,
    total_count: i32,
    top_expense: Expense,
    top_count: Trans,
    trend: [Trend; 3],
    cafeteria_count: i32,
    cafeteria_amount: f64,
    groceries_count: i32,
    groceries_amount: f64,
    logistics_count: i32,
    logistics_amount: f64,
    other_count: i32,
    other_amount: f64,
    breakfast: Meal,
    lunch: Meal,
    dinner: Meal,
    midnight_snack: Meal,
}


#[tokio::main]
async fn main() {
    let config = config::config::init_config("config.toml").unwrap();
    let mongo_client = MongoClient::with_uri_str(&config.db.url).await.unwrap();
    let mut redis_conns = RedisConnections::new(&config).unwrap();

    loop {
        process_queue(&mongo_client, &mut redis_conns).await;
        tokio::time::sleep(Duration::from_secs(REFRESH_INTERVAL)).await;
    }
}

async fn process_queue(mongo_client: &MongoClient, redis_conns: &mut RedisConnections) {
    let queue: Vec<String> = redis_conns.main.keys("request:*").unwrap();
    for key in queue {
        let castgc: String = redis_conns.main.get(&key).unwrap();
        let castgc: &str = castgc.split(":").collect::<Vec<&str>>()[1];
        let t = key.split(":").collect::<Vec<&str>>();
        let period = t[2];
        let account = t[1];
        let key_res = format!("result:{}:{}", account, period);
        let _: () = redis_conns.main.del(&key).unwrap();
        let _:() = match process(castgc, period, account.to_string(), &mongo_client, &mut redis_conns.tag, None, &mut redis_conns.untagged).await{
            Ok(id) => {
                redis_conns.main.set(key_res, id).unwrap()
            },
            Err(e) => {
                redis_conns.main.set(key_res, format!("error: {}", e)).unwrap()
            }
        };
    }
}

async fn calculate_trends(period: &str, date: chrono::DateTime<Utc>, coll: &Collection<ReportData>, 
    castgc: &str, account: &str, db: &MongoClient, tag_db: &mut redis::Connection, untagged_db: &mut redis::Connection) 
    -> Result<[Trend; 3], WorkerError> {
    let mut trends = [Trend { count: 0, expense: 0.0 }; 3];
    
    match period {
        "week" => {
            for i in 1..=3 {
                let week_start = date - chrono::Duration::weeks(i);
                let week_id = week_start.format("%Y%U").to_string();
                if let Some(report) = coll.find_one(doc! { "date": &week_id }).await? {
                    trends[i as usize -1] = Trend { count: report.total_count, expense: report.total_expense };
                }
            }
        },
        "month" => {
            let mut month_start = date.with_day(1).unwrap();
            for i in 1..=3 {
                month_start = (month_start - chrono::Duration::days(1)).with_day(1).unwrap();
                let month_id = month_start.format("%Y%m").to_string();
                
                trends[i-1] = match coll.find_one(doc! { "date": &month_id }).await? {
                    Some(report) => Trend { count: report.total_count, expense: report.total_expense },
                    None => {
                        process(castgc, "month", account.to_string(), db, tag_db, 
                               Some(month_start), untagged_db).await?;
                        let report = coll.find_one(doc! { "date": month_id }).await?.unwrap();
                        Trend { count: report.total_count, expense: report.total_expense }
                    }
                };
            }
        },
        _ => return Err(WorkerError::InvalidPeriod(period.to_string()))
    }
    
    Ok(trends)
}

#[async_recursion]
async fn process(castgc: &str, period: &str, account: String, 
db: &MongoClient, tag_db: &mut redis::Connection, recursion: Option<chrono::DateTime<Utc>>, 
untagged_db: &mut redis::Connection) 
-> Result<String, WorkerError> {
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
            let date_status = match recursion.clone() {
                Some(t) => t.format("%Y-%m-01").to_string(),
                None => date.format("%Y-%m-01").to_string()
            };
            form.insert("dateStatus", date_status);
            db.database("report_month").collection(account.as_str())
        },
        _ => return Err(WorkerError::InvalidPeriod(period.to_string()))
    };
    let api = "http://ecard.m.hust.edu.cn/wechat-web/QueryController/select.html";
    let mut trans: collections::HashMap<String,(i32,f64)> = collections::HashMap::new();
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
    let mut cafeteria_amount: f64 = 0.0;
    let mut groceries_count: i32 = 0;
    let mut groceries_amount: f64 = 0.0;
    let mut logistics_count: i32 = 0;
    let mut logistics_amount: f64 = 0.0;
    let mut other_count: i32 = 0;
    let mut other_amount: f64 = 0.0;

    loop{
        let res = client.get(api).query(&form).send().await.unwrap();
        let text = &res.text().await.unwrap();
        let text = &text[9..text.len()-1];
        let data: serde_json::Value = serde_json::from_str(&text).unwrap();
        if balance == -1.0 {
            balance = data["total"][0]["cardbal"].as_str().unwrap().parse::<f64>()? / 100.0;
        }
        if data["retcode"].as_str().unwrap() != "0" {
            return Err(WorkerError::CardSystemError(data["errmsg"].as_str().unwrap().to_string()));
        }
        for item in data["total"].as_array().unwrap().to_vec() {
            if item["sign_tranamt"].as_str().unwrap().parse::<i64>()? > 0 {
                total_topup += item["tranamt"].as_str().unwrap().parse::<f64>()? / 100.0;
                continue;
            }
            let mut occtime = item["occtime"].as_str().unwrap().parse::<i64>()?;
            let mercname = item["mercname"].as_str().unwrap().to_string();
            let mercacc = item["mercacc"].as_str().unwrap();
            let mut tranamt = item["tranamt"].as_str().unwrap().parse::<f64>()?;
            tranamt /= 100.0;
            total_expense += tranamt;
            total_count += 1;
            if trans.contains_key(&mercname) {
                let t = trans.get_mut(mercname.as_str()).unwrap();
                t.0 += 1;
                t.1 += tranamt;
                if t.0 > top_count.count {
                    top_count = Trans {
                        location: mercname.clone(),
                        amount: t.1,
                        count: t.0,
                    };
                }
            }else{
                trans.insert(mercname.clone(), (1, tranamt));
            }

            if tranamt > top_expense.amount {
                top_expense = Expense {
                    time: item["occtime"].as_str().unwrap().to_string(),
                    location: mercname.clone(),
                    amount: tranamt,
                };
            }

            let tag: String = tag_db.get(mercacc).unwrap_or(process_untagged(untagged_db, tag_db, &mercacc.to_string(), &mercname));
            match tag.as_str() {
                "CAF" => {
                    occtime %= 1000000;
                    if let Some((_, _, idx)) = MEAL_TIME_RANGES.iter()
                        .find(|(start, end, _)| occtime >= *start && occtime <= *end) {
                        meals[*idx].count += 1;
                        meals[*idx].amount += tranamt;
                    }
                    cafeteria_amount += tranamt;
                    cafeteria_count += 1;
                },
                "GRO" => {
                    groceries_count += 1;
                    groceries_amount += tranamt;
                },
                "LOG" => {
                    logistics_count += 1;
                    logistics_amount += tranamt;
                },
                "OTH" => {
                    other_count += 1;
                    other_amount += tranamt;
                }
                _ => {}
            };
        }
        form.remove("curpage");
        form.insert("curpage", data["nextpage"].as_str().unwrap().to_string());
        if data["nextpage"].as_str().unwrap() == "0" {
            break;
        }
    };

    
    let mut trend = calculate_trends(period, date, &coll, castgc, &account, db, tag_db, untagged_db).await?;

    
    let fmtstr = match recursion {
        Some(t) => t.format("%Y%m").to_string(),
        None => {
            match period {
                "week" => {
                    for i in 1..=3 {
                        let week_start = date - chrono::Duration::weeks(i);
                        let week_id = week_start.format("%Y%U").to_string();
                        if let Some(report) = coll.find_one(doc! { "date": week_id.clone() }).await? {
                            trend[(i-1) as usize] = Trend {
                                count: report.total_count,
                                expense: report.total_expense,
                            };
                        };
                    };
                    date.format("%Y%U").to_string()
                },
                "month" => {
                    let mut month_start = date.with_day(1).unwrap();
                    for i in 1 as i64..=3 {
                        month_start -= chrono::Duration::days(1);
                        month_start = month_start.with_day(1).unwrap();
                        let month_id = month_start.format("%Y%m").to_string();
                        match coll.find_one(doc! { "date": month_id.clone() }).await.unwrap() {
                            Some(report) => {
                                trend[(i-1) as usize] = Trend {
                                    count: report.total_count,
                                    expense: report.total_expense,
                                };
                            },
                            None => {
                                process(castgc, "month", account.clone(), db, tag_db, Some(month_start), untagged_db).await?;
                                let report = coll.find_one(doc! { "date": month_id }).await?.unwrap();
                                trend[(i-1) as usize] = Trend {
                                    count: report.total_count,
                                    expense: report.total_expense,
                                };
                            }
                        }
                    };
                    date.format("%Y%m").to_string()
                },
                _ => return Err(WorkerError::InvalidPeriod(period.to_string()))
            }
        }
    };
    let result = ReportData {
        date: fmtstr,
        balance,
        total_expense,
        total_topup,
        total_count,
        top_expense,
        top_count,
        trend,
        cafeteria_count,
        cafeteria_amount,
        groceries_count,
        groceries_amount,
        logistics_count,
        logistics_amount,
        other_count,
        other_amount,
        breakfast: meals[0].clone(),
        lunch: meals[1].clone(),
        dinner: meals[2].clone(),
        midnight_snack: meals[3].clone(),
    };
    let id = coll.insert_one(result).await.unwrap().inserted_id.as_object_id().unwrap().to_hex();
    Ok(format!("report_{}/{}/{}", period, account, id))
}

fn process_untagged(untagged_db: &mut redis::Connection, tags_db: &mut redis::Connection, mercacc: &String, mercname: &String) -> String{
    for i in CAF_NAME.iter() {
        if mercname.contains(i) {
            let _:() = tags_db.set(mercacc, "CAF").unwrap();
            return "CAF".to_string();
        }
    };
    for i in GRO_NAME.iter() {
        if mercname.contains(i) {
            let _:() = tags_db.set(mercacc, "GRO").unwrap();
            return "GRO".to_string();
        }
    };
    let _:() = untagged_db.set(mercacc, mercname).unwrap();
    "OTH".to_string()
}