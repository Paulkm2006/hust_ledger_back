use serde::{Serialize, Deserialize};
use actix_web::web;
use redis::Commands;
use mongodb::{Client as MongoClient, Collection, bson::doc};
use redis::Client as RedisClient;


#[derive(Serialize, Deserialize)]
pub struct Expense {
    time: String,
    location: String,
    amount: f32,
}

#[derive(Serialize, Deserialize)]
pub struct Trend {
    time: String,
    amount: f32,
}

#[derive(Serialize, Deserialize)]
pub struct ReportData {
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
pub struct Meal {
    count: i32,
    amount: f32,
    location: String,
}

pub enum Status{
	Created,
	Processing,
	Finished(ReportData),
	Error(Box<dyn std::error::Error>),
}

pub async fn get_report(account_no: String, period: &str, jsession: &str, redis_client: web::Data<RedisClient>, mongo_client: web::Data<MongoClient>) -> Result<Status, Box<dyn std::error::Error>> {
    let mut con = redis_client.get_connection()?;
    let key = format!("request:{}:{}", account_no, period);
    match con.get::<_, String>(&key) {
        Ok(v) => {
            if v.starts_with("waiting") || v == "accepted" {
                Ok(Status::Processing)
            } else {
                Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Report generation failed. ".to_string() + &v)))
            }
        },
        Err(_) => {
            let key_res = format!("result:{}:{}", account_no, period);
            match con.get::<_, String>(&key_res) {
                Ok(v) => {
                    let report_no = &v;
                    let db = mongo_client.database("hust_ledger");
                    let collection: Collection<ReportData> = db.collection("report");
                    let report = collection.find_one(doc!{"_id": report_no}).await?.unwrap();
                    Ok(Status::Finished(report))
                },
                Err(_) => {
                    let _: () = con.set(&key, "waiting:".to_string()+jsession)?;
                    Ok(Status::Created)
                }
            }
        }
    }
}
