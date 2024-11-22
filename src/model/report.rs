use serde::{Serialize, Deserialize};
use actix_web::web;
use redis::Commands;
use mongodb::{Client as MongoClient, Collection, bson::doc};
use redis::Client as RedisClient;


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
pub struct ReportData {
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

pub enum Status{
	Created,
	Processing,
	Finished(ReportData),
	Error(Box<dyn std::error::Error>),
}

pub async fn get_report(account_no: String, period: &str, castgc: &str, redis_client: web::Data<RedisClient>, mongo_client: web::Data<MongoClient>) -> Result<Status, Box<dyn std::error::Error>> {
    let mut con = redis_client.get_connection()?;
    let key = format!("request:{}:{}", account_no, period);
    match con.get::<_, String>(&key) {
        Ok(_) => {
            Ok(Status::Processing)
        },
        Err(_) => {
            let key_res = format!("result:{}:{}", account_no, period);
            match con.get::<_, String>(&key_res) {
                Ok(v) => {
                    if v.starts_with("error") {
                        let _: () = con.del(&key_res)?;
                        return Ok(Status::Error(Box::new(std::io::Error::new(std::io::ErrorKind::Other, v.replace("error:", "")))));
                    };
                    let path = v.split("/").collect::<Vec<&str>>();
                    println!("{:?}", path);
                    let db = mongo_client.database(path[0]);
                    let collection: Collection<ReportData> = db.collection(path[1]);
                    let report = collection.find_one(doc!{"_id": mongodb::bson::oid::ObjectId::parse_str(&path[2])?}).await?.unwrap();
                    Ok(Status::Finished(report))
                },
                Err(_) => {
                    let _: () = con.set(&key, "waiting:".to_string()+castgc)?;
                    Ok(Status::Created)
                }
            }
        }
    }
}
