use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::HttpResponse;

pub fn unixtime_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock is broken.")
        .as_secs()
}

pub fn forbidden() -> HttpResponse {
    HttpResponse::Forbidden().body("No. Absolutely not. I forbid it.")
}
