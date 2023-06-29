use std::collections::HashMap;

use actix_web::middleware::Logger;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use auth::{generate_oauth, get_auth_redirect, get_auth_url, OAuth};
use dashmap::DashMap;
use db::{create_pool, Pool};
use env_logger::Env;
use location::{get_location_get, get_location_list, post_location_update, Location, TokenExpiry};
use parking_lot::Mutex;
use primitive_types::U512;

mod auth;
mod db;
mod location;
mod misc;

const SHORT_EXPIRY_SECS: u64 = 60 * 5;
const LONG_EXPIRY_SECS: u64 = 60 * 60 * 3;
const LONG_EXPIRY_SECS_I: i64 = LONG_EXPIRY_SECS as i64;

struct AppState {
    // TODO: maybe u256?
    session_tokens: DashMap<U512, TokenExpiry>,
    // TODO: parking_lot
    /// The last location that we got from the client.
    last_location: Mutex<HashMap<String, Location>>,
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
        last_location: Mutex::new(HashMap::with_capacity(2)),
        auth: generate_oauth(),
        pool: create_pool(),
    });
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(hello)
            .service(get_location_get)
            .service(post_location_update)
            .service(get_location_list)
            .service(get_auth_url)
            .service(get_auth_redirect)
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
