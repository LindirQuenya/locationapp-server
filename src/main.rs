use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use dashmap::DashMap;
use db::{create_pool, Pool};
use location::{location_get, Location, TokenExpiry};
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

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
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
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(hello)
            .service(echo)
            .service(location_get)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
