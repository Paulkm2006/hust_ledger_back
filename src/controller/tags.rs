use super::super::model::tags;
use actix_web::{web, HttpResponse, Responder};
use redis::Client as RedisClient;


pub async fn get_tags(
	tags_client: web::Data<RedisClient>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let tags = tags::dump_tags(&tags_client).await?;
	Ok(HttpResponse::Ok().json(tags))
}