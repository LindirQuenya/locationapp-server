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
pub(crate) struct Location {
    pub(crate) latitude: f64,
    pub(crate) longitude: f64,
    pub(crate) time: u64,
}

#[derive(Deserialize)]
pub(crate) struct LocationIn {
    apikey: String,
    latitude: f64,
    longitude: f64,
}

#[derive(Deserialize)]
pub(crate) struct LocationGetIn {
    session_key: u128,
    name: String,
}
#[post("/location/get")]
pub(crate) async fn post_location_get(
    info: web::Json<LocationGetIn>,
    data: web::Data<AppState>,
) -> impl Responder {
    // Don't bother reconstructing the durations each time, just keep them around.
    static SHORT_EXPIRY: Duration = Duration::from_secs(SHORT_EXPIRY_SECS);
    static LONG_EXPIRY: Duration = Duration::from_secs(LONG_EXPIRY_SECS);
    log::trace!(
        "/location/get: called with session key: {}",
        info.session_key
    );
    // Try to get the session key from the table of allowed ones.
    let expiry = {
        match data.session_tokens.get(&info.session_key) {
            None => {
                log::debug!("/location/get: Bad session key.");
                return forbidden();
            }
            Some(e) => TokenExpiry {
                issued: e.issued,
                last_used: e.last_used,
            },
        }
    };
    // Check if it's expired.
    if expiry.issued.elapsed() > LONG_EXPIRY || expiry.last_used.elapsed() > SHORT_EXPIRY {
        // If it is, remove it.
        data.session_tokens.remove(&info.session_key);
        log::debug!("/location/get: Expired session key.");
        return forbidden();
    }
    // We've gotten through authentication, update the token's last-used time.
    {
        data.session_tokens.insert(
            info.session_key,
            TokenExpiry {
                last_used: Instant::now(),
                issued: expiry.issued,
            },
        );
    }
    // Grab the last location measurement.
    let last_loc: Location = {
        match data.last_location.lock().get(&info.name) {
            Some(loc) => loc.to_owned(),
            None => Location {
                latitude: 0.0,
                longitude: 0.0,
                time: 0,
            },
        }
    };
    // Return our serialized data.
    HttpResponse::Ok().body(serde_json::to_string(&last_loc).unwrap())
}
#[derive(Deserialize)]
pub(crate) struct LocationListIn {
    session_key: u128,
}
#[post("/location/list")]
pub(crate) async fn post_location_list(
    info: web::Json<LocationListIn>,
    data: web::Data<AppState>,
) -> impl Responder {
    // Don't bother reconstructing the durations each time, just keep them around.
    static SHORT_EXPIRY: Duration = Duration::from_secs(SHORT_EXPIRY_SECS);
    static LONG_EXPIRY: Duration = Duration::from_secs(LONG_EXPIRY_SECS);
    log::trace!(
        "/location/list: called with session key: {}",
        info.session_key
    );
    // Try to get the session key from the table of allowed ones.
    let expiry = {
        match data.session_tokens.get(&info.session_key) {
            None => {
                log::debug!("/location/list: Bad session key.");
                return forbidden();
            }
            Some(e) => TokenExpiry {
                issued: e.issued,
                last_used: e.last_used,
            },
        }
    };
    // Check if it's expired.
    if expiry.issued.elapsed() > LONG_EXPIRY || expiry.last_used.elapsed() > SHORT_EXPIRY {
        // If it is, remove it.
        data.session_tokens.remove(&info.session_key);
        log::debug!("/location/list: Expired session key.");
        return forbidden();
    }
    // We've gotten through authentication, update the token's last-used time.
    {
        data.session_tokens.insert(
            info.session_key,
            TokenExpiry {
                last_used: Instant::now(),
                issued: expiry.issued,
            },
        );
    }
    // Grab the list of names.
    let names: Vec<String> = { data.last_location.lock().keys().cloned().collect() };
    // Return our serialized data.
    HttpResponse::Ok().body(serde_json::to_string(&names).unwrap())
}

#[post("/location/update")]
pub(crate) async fn post_location_update(
    info: web::Json<LocationIn>,
    data: web::Data<AppState>,
) -> impl Responder {
    let name = match db::verify_api_key(&data.pool, info.apikey.clone()).await {
        Ok(Some(name)) => name,
        _ => {
            log::debug!("/location/update: Bad API key.");
            return forbidden();
        }
    };
    let now = misc::unixtime_now();
    {
        data.last_location.lock().insert(
            name,
            Location {
                latitude: info.latitude,
                longitude: info.longitude,
                time: now,
            },
        );
    }
    HttpResponse::Ok().body(format!("{}", now))
}
