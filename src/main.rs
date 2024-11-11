mod router;
pub mod utils;

use actix_web::{App, HttpServer, middleware::Logger};
use env_logger::Env;

#[actix_web::main]
async fn main() -> std::io::Result<()>{
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    let server = HttpServer::new(|| {
        App::new()
            .wrap(Logger::new("%{r}a %r %s"))
            .configure(router::router::config)
    });
    server.bind("0.0.0.0:8080")?.run().await
}