use super::super::model::tags;
use actix_web::{web, HttpResponse, Responder};
use redis::Client as RedisClient;

type TagsClient = Option<RedisClient>;


pub async fn get_tags(
	tags_client: web::Data<TagsClient>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let tags_client = tags_client.as_ref().as_ref().unwrap();
	let tags = tags::dump_tags(tags_client).await?;
	Ok(HttpResponse::Ok().json(tags))
}