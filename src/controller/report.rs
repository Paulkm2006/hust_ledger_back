use serde::{Serialize, Deserialize};
use super::super::model::report::{Status, ReportData, get_report};
use super::super::utils::hust_login::get_account_no;
use actix_web::{web, HttpResponse, Responder};
use mongodb::Client as MongoClient;
use redis::Client as RedisClient;

#[derive(Deserialize)]
pub struct Query {
	jsessionid: String,
	period: String,
}

#[derive(Serialize)]
pub struct Report{
	status: i32,
	msg: String,
	data: Option<ReportData>,
}

pub async fn report(q: web::Query<Query>, redis_client: web::Data<RedisClient>, mongo_client: web::Data<MongoClient>) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let account_no = match get_account_no(&q.jsessionid).await{
		Ok(t) => t,
		Err(e) => {return Ok(HttpResponse::Forbidden().json(Report{
			status: 403,
			msg: e.to_string(),
			data: None,
		}));},
	};
	match get_report(account_no, &q.period, &q.jsessionid, redis_client, mongo_client).await?{
		Status::Created => Ok(HttpResponse::Created().json(Report{
			status: 201,
			msg: "Report generation queued".to_string(),
			data: None,
		})),
		Status::Processing => Ok(HttpResponse::Created().json(Report{
			status: 201,
			msg: "Report is being generated".to_string(),
			data: None,
		})),
		Status::Finished(data) => Ok(HttpResponse::Ok().json(Report{
			status: 200,
			msg: "Success".to_string(),
			data: Some(data),
		})),
		Status::Error(e) => Ok(HttpResponse::InternalServerError().json(Report{
			status: 500,
			msg: e.to_string(),
			data: None,
		})),
	}
}