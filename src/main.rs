use actix_web::middleware::Logger;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use dashmap::DashMap;
use db::{create_pool, Pool};
use env_logger::Env;
use location::{post_location_get, post_location_update, Location, TokenExpiry};
use parking_lot::Mutex;

mod auth;
mod db;
mod location;
mod misc;

use auth::*;

const SHORT_EXPIRY_SECS: u64 = 60 * 5;
const LONG_EXPIRY_SECS: u64 = 60 * 60 * 3;

struct AppState {
    // TODO: maybe u256?
    session_tokens: DashMap<u128, TokenExpiry>,
    // TODO: parking_lot
    /// The last location that we got from the client.
    last_location: Mutex<Location>,
    auth: OAuth,
    pool: Pool,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = web::Data::new(AppState {
        session_tokens: DashMap::with_capacity(2),
        last_location: Mutex::new(Location {
            latitude: 0.0,
            longitude: 0.0,
            time: 0,
        }),
        auth: generate_oauth(),
        pool: create_pool(),
    });
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(hello)
            .service(post_location_get)
            .service(post_location_update)
            .service(get_auth_url)
            .service(get_auth_redirect)
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
