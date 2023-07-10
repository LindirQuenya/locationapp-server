use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::{http::header::ContentType, HttpResponse};

/// Gets the current unix time in seconds. Pretty self-explanatory.
pub fn unixtime_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock is broken.")
        .as_secs()
}

// This is the API's 403 page.
pub fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden()
        .insert_header(ContentType::json())
        .body("{\"err\":\"Authorization failed.\"}")
}
