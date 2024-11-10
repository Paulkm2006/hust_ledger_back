mod router;
pub mod utils;

use actix_web::{App, HttpServer};

#[actix_web::main]
async fn main() -> std::io::Result<()>{
    let server = HttpServer::new(|| {
        App::new().configure(router::router::config)
    });
    server.bind("0.0.0.0:8080")?.run().await
}