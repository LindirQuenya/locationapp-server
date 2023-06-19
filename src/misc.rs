use std::time::{SystemTime, UNIX_EPOCH};

pub fn unixtime_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Your system clock is broken.")
        .as_secs()
}
