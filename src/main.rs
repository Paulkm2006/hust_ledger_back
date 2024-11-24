pub mod router;
pub mod utils;
pub mod model;
pub mod config;
pub mod controller;

use actix_web::{web, App, HttpServer, middleware::Logger};
use env_logger::Env;
use mongodb::{Client as MongoClient, options::ClientOptions};
use redis::Client as RedisClient;

type TagsClient = Option<RedisClient>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = match config::config::init_config("config.toml") {
        Ok(config) => config,
        Err(e) => {
            panic!("Failed to load config: {}", e);
        },
    };
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    let mongo_client_options = ClientOptions::parse(&config.db.url).await.unwrap();
    let mongo_client = MongoClient::with_options(mongo_client_options).unwrap();
    let redis_client = RedisClient::open(config.redis.url.as_str()).unwrap();
    let tags_client: TagsClient = Some(RedisClient::open(config.tags_db.url.as_str()).unwrap());

    let server_host = config.server.host.clone();
    let server_port = config.server.port;

    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(config.clone()))
            .app_data(web::Data::new(mongo_client.clone()))
            .app_data(web::Data::new(redis_client.clone()))
            .app_data(web::Data::new(tags_client.clone()))
            .wrap(Logger::new("%{r}a %r %s"))
            .configure(router::router::config)
    });
    server.bind(server_host + ":" + &server_port.to_string())?.run().await
}