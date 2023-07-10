use std::collections::HashMap;
use std::fs::File;

use actix_web::middleware::Logger;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use auth::{generate_oauth, get_auth_redirect, get_auth_url, OAuth};
use clap::Parser;
use cli::Cli;
use config::Config;
use dashmap::DashMap;
use db::{create_pool, Pool};
use env_logger::Env;
use location::{get_location_get, get_location_list, post_location_update, Location, TokenExpiry};
use parking_lot::Mutex;
use primitive_types::U512;

mod auth;
mod cli;
mod config;
mod db;
mod location;
mod misc;

const SHORT_EXPIRY_SECS: u64 = 60 * 30;
const LONG_EXPIRY_SECS: u64 = 60 * 60 * 24;
const LONG_EXPIRY_SECS_I: i64 = LONG_EXPIRY_SECS as i64;

struct AppState {
    // The session valid tokens and when they expire.
    session_tokens: DashMap<U512, TokenExpiry>,
    /// The last location that we got from each client, by client name.
    /// I decided not to use a DashMap because I need to iterate the keys.
    last_location: Mutex<HashMap<String, Location>>,
    /// A collection of opaque authentication state things.
    auth: OAuth,
    /// The connection pool for the database.
    pool: Pool,
    /// The configuration options, parsed at startup.
    config: Config,
}

#[get("/api/")]
async fn hello() -> impl Responder {
    HttpResponse::Forbidden().body("Get out of my API, you silly goose!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Parse the flags to get the config file location.
    let cli = Cli::parse();

    // Open the config file and parse it.
    let configfile = File::open(cli.config).expect("Config file doesn't exist.");
    let config: Config = serde_json::from_reader(configfile).expect("Bad config file format.");

    // Clone the configured listen addresses, we'll need them in a moment.
    let listens = config.listen.clone();

    // Build the global state.
    let state = web::Data::new(AppState {
        session_tokens: DashMap::with_capacity(2),
        last_location: Mutex::new(HashMap::with_capacity(2)),
        auth: generate_oauth(&config),
        pool: create_pool(&config),
        config,
    });

    // Initialize the log level from environment variables.
    env_logger::init_from_env(Env::default().default_filter_or("info"));

    // Construct the server object with all the APIs,
    // the global data, and the logger.
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

    // Iterate the configured listen addresses and bind the server
    // to each one.
    for elem in listens {
        server = server.bind((elem.addr, elem.port))?;
    }

    // Aaand we're home-free.
    server.run().await
}
