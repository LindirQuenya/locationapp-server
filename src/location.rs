use std::time::{Duration, Instant};

use actix_web::{get, http::header::ContentType, post, web, HttpRequest, HttpResponse, Responder};
use dashmap::DashMap;
use primitive_types::U512;
use serde::{Deserialize, Serialize};

use crate::{
    auth::SessionToken,
    db,
    misc::{self, forbidden},
    AppState, LONG_EXPIRY_SECS, SHORT_EXPIRY_SECS,
};

#[derive(Clone)]
pub(crate) struct TokenExpiry {
    /// If a token is unused for a certain duration, it should expire.
    pub(crate) last_used: Instant,
    /// A token has a maximum lifetime, after which it will finally expire.
    pub(crate) issued: Instant,
}

#[derive(Serialize, Clone)]
pub(crate) struct Location {
    /// Degrees.
    pub(crate) latitude: f64,
    /// Degrees.
    pub(crate) longitude: f64,
    /// Meters.
    pub(crate) accuracy: f64,
    /// Seconds since the unix epoch.
    pub(crate) time: u64,
}

#[derive(Deserialize)]
pub(crate) struct LocationIn {
    api_key: String,
    latitude: f64,
    longitude: f64,
    accuracy: f64,
}

#[derive(Deserialize)]
pub(crate) struct LocationGetIn {
    id: u64,
}

fn read_session_token(req: HttpRequest) -> Option<SessionToken> {
    match req.cookies() {
        Ok(cookievec) => {
            for cookie in cookievec.iter() {
                if cookie.name() == "session" {
                    return serde_json::from_str(cookie.value()).ok();
                }
            }
            None
        }
        Err(_) => None,
    }
}

#[get("/api/location/get")]
pub(crate) async fn get_location_get(
    info: web::Query<LocationGetIn>,
    data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    // Read the session token from the cookies.
    let token = match read_session_token(req) {
        Some(t) => t,
        None => return forbidden(),
    };

    log::trace!(
        "/api/location/get: called with session key: {}",
        token.session_key
    );

    // Confirm that the session key is authentic.
    if !verify_session_key(token.session_key, &data.session_tokens) {
        return forbidden();
    }

    // Grab the last location measurement.
    let last_loc: Location = {
        match data.last_location.get(&info.id) {
            Some(loc) => loc.value().to_owned(),
            None => Location {
                latitude: 0.0,
                longitude: 0.0,
                accuracy: 0.0,
                time: 0,
            },
        }
    };
    // Return our serialized data.
    HttpResponse::Ok()
        .insert_header(ContentType::json())
        .body(serde_json::to_string(&last_loc).unwrap())
}

#[get("/api/location/list")]
pub(crate) async fn get_location_list(
    data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    // Read the session token from the cookies.
    let token = match read_session_token(req) {
        Some(t) => t,
        None => return forbidden(),
    };

    log::trace!(
        "/api/location/list: called with session key: {}",
        token.session_key
    );

    // Confirm that the session key is authentic.
    if !verify_session_key(token.session_key, &data.session_tokens) {
        return forbidden();
    }

    // Grab the list of api_key ids and names.
    let names: Vec<(u64, String)> = { data.names.lock().clone() };

    // Serialize it and we're off to the races.
    HttpResponse::Ok()
        .insert_header(ContentType::json())
        .body(serde_json::to_string(&names).unwrap())
}

fn verify_session_key(session_key: U512, session_tokens: &DashMap<U512, TokenExpiry>) -> bool {
    // Don't bother reconstructing the durations each time, just keep them around.
    static SHORT_EXPIRY: Duration = Duration::from_secs(SHORT_EXPIRY_SECS);
    static LONG_EXPIRY: Duration = Duration::from_secs(LONG_EXPIRY_SECS);

    // Try to get the session key from the table of allowed ones.
    let expiry = {
        match session_tokens.get(&session_key) {
            None => {
                log::debug!("/api/location/*: Bad session key.");
                return false;
            }
            Some(e) => e.value().to_owned(),
        }
    };

    // Check if it's expired.
    if expiry.issued.elapsed() > LONG_EXPIRY || expiry.last_used.elapsed() > SHORT_EXPIRY {
        // If it is, remove it.
        session_tokens.remove(&session_key);
        log::debug!("/api/location/*: Expired session key.");
        return false;
    }

    // We've gotten through authentication, update the token's last-used time.
    {
        session_tokens.insert(
            session_key,
            TokenExpiry {
                last_used: Instant::now(),
                issued: expiry.issued,
            },
        );
    }
    true
}

#[derive(Serialize)]
struct LocationUpdateOut {
    time: u64,
}

#[post("/api/location/update")]
pub(crate) async fn post_location_update(
    info: web::Json<LocationIn>,
    data: web::Data<AppState>,
) -> impl Responder {
    // Verify the API key with the database and get the associated api_key id and name.
    let id_name = match db::verify_api_key(&data.pool, info.api_key.clone()).await {
        Ok(Some(name)) => name,
        _ => {
            log::debug!("/api/location/update: Bad API key.");
            return forbidden();
        }
    };

    // Record the current time.
    let now = misc::unixtime_now();

    // Update the last-seen location.
    let already_existed = data
        .last_location
        .insert(
            id_name.0,
            Location {
                latitude: info.latitude,
                longitude: info.longitude,
                accuracy: info.accuracy,
                time: now,
            },
        )
        .is_some();

    // If we hadn't seen that client before, push their name and id into the list.
    if !already_existed {
        log::debug!("Never-before-seen client: ({}, {})", id_name.0, id_name.1);
        data.names.lock().push(id_name);
    }

    // Let the client know that it was successful, and what time was recorded.
    HttpResponse::Ok()
        .insert_header(ContentType::json())
        .body(serde_json::to_string(&LocationUpdateOut { time: now }).unwrap())
}
