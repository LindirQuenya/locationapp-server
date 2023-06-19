use std::time::{Duration, Instant};

use actix_web::{post, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::{
    db,
    misc::{self, forbidden},
    AppState, LONG_EXPIRY_SECS, SHORT_EXPIRY_SECS,
};
pub(crate) struct TokenExpiry {
    /// If a token is unused for a certain duration, it should expire.
    pub(crate) last_used: Instant,
    /// A token has a maximum lifetime, after which it will finally expire.
    pub(crate) issued: Instant,
}

#[derive(Serialize, Clone)]
pub struct Location {
    latitude: f64,
    longitude: f64,
    time: u64,
}

#[derive(Deserialize)]
pub(crate) struct LocationIn {
    apikey: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize)]
pub(crate) struct SessionKeyIn {
    sessionkey: u128,
}
#[post("/location/get")]
pub(crate) async fn location_get(
    info: web::Json<SessionKeyIn>,
    data: web::Data<AppState>,
) -> impl Responder {
    // Don't bother reconstructing the durations each time, just keep them around.
    static SHORT_EXPIRY: Duration = Duration::from_secs(SHORT_EXPIRY_SECS);
    static LONG_EXPIRY: Duration = Duration::from_secs(LONG_EXPIRY_SECS);
    // Try to get the session key from the table of allowed ones.
    let expiry = match data.session_tokens.get(&info.sessionkey) {
        None => return forbidden(),
        Some(e) => e,
    };
    // Check if it's expired.
    if expiry.issued.elapsed() > LONG_EXPIRY || expiry.last_used.elapsed() > SHORT_EXPIRY {
        // If it is, remove it.
        data.session_tokens.remove(&info.sessionkey);
        return forbidden();
    }
    // We've gotten through authentication, update the token's last-used time.
    data.session_tokens.insert(
        info.sessionkey,
        TokenExpiry {
            last_used: Instant::now(),
            issued: expiry.issued,
        },
    );
    // Grab the last location measurement.
    let last_loc: Location = { data.last_location.lock().clone() };
    // Return our serialized data.
    HttpResponse::Ok().body(serde_json::to_string(&last_loc).unwrap())
}

#[post("/location/update")]
pub(crate) async fn location_update(
    info: web::Json<LocationIn>,
    data: web::Data<AppState>,
) -> impl Responder {
    if !db::verify_api_key(&data.pool, info.apikey.clone())
        .await
        .unwrap_or(false)
    {
        return forbidden();
    }
    let now = misc::unixtime_now();
    {
        *data.last_location.lock() = Location {
            latitude: info.latitude,
            longitude: info.longitude,
            time: now,
        };
    }
    HttpResponse::Ok().body(format!("{}", now))
}
