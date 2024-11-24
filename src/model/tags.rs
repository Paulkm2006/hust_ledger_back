use redis::Commands;
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize, Debug)]
pub struct Tag{
	mercacc: String,
	tag: String,
}

pub async fn dump_tags(redis_client: &redis::Client) -> Result<Vec<Tag>, Box<dyn std::error::Error>> {
	let mut con = redis_client.get_connection()?;
	let keys: Vec<String> = con.scan()?.collect();
	let tags = keys.into_iter()
		.map(|key| {
			let tag: String = con.get(&key)?;
			Ok(Tag { mercacc: key, tag: tag })
		})
		.collect::<Result<Vec<Tag>, redis::RedisError>>()?;
	Ok(tags)
}