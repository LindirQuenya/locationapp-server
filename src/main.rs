use std::collections::HashMap;
use std::fs::File;

use actix_web::middleware::Logger;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use auth::{generate_oauth, get_auth_redirect, get_auth_url, OAuth};
use config::Config;
use dashmap::DashMap;
use db::{create_pool, Pool};
use env_logger::Env;
use location::{get_location_get, get_location_list, post_location_update, Location, TokenExpiry};
use parking_lot::Mutex;
use primitive_types::U512;

mod auth;
mod config;
mod db;
mod location;
mod misc;

const SHORT_EXPIRY_SECS: u64 = 60 * 30;
const LONG_EXPIRY_SECS: u64 = 60 * 60 * 24;
const LONG_EXPIRY_SECS_I: i64 = LONG_EXPIRY_SECS as i64;

struct AppState {
    // TODO: maybe u256?
    session_tokens: DashMap<U512, TokenExpiry>,
    // TODO: parking_lot
    /// The last location that we got from the client.
    last_location: Mutex<HashMap<String, Location>>,
    auth: OAuth,
    pool: Pool,
    config: Config,
}

#[get("/api/")]
async fn hello() -> impl Responder {
    HttpResponse::Forbidden().body("Get out of my API, you silly goose!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // TODO clap-parse this path at runtime.
    let configfile =
        File::open("secret/config.json").expect("Config file secret/config.json doesn't exist.");
    let config: Config = serde_json::from_reader(configfile).expect("Bad config file format.");
    let listens = config.listen.clone();
    let state = web::Data::new(AppState {
        session_tokens: DashMap::with_capacity(2),
        last_location: Mutex::new(HashMap::with_capacity(2)),
        auth: generate_oauth(&config),
        pool: create_pool(&config),
        config,
    });
    env_logger::init_from_env(Env::default().default_filter_or("info"));
    let mut server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(hello)
            .service(get_location_get)
            .service(post_location_update)
            .service(get_location_list)
            .service(get_auth_url)
            .service(get_auth_redirect)
            .wrap(Logger::default())
    });
    for elem in listens {
        server = server.bind((elem.addr, elem.port))?;
    }
    server.run().await
}
