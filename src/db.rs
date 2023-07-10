use actix_web::web;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::{config::Config, misc::unixtime_now};

pub(crate) type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

/// Checks if an email is authorized to be a web_user, and returns the associated username if so.
pub(crate) async fn verify_email(
    pool: &Pool,
    email: String,
) -> Result<Option<String>, actix_web::Error> {
    query_internal(
        pool,
        email,
        unixtime_now(),
        "SELECT username FROM web_users WHERE email IS ?1 AND expiration > ?2".to_string(),
    )
    .await
}

/// Checks if an api_key is authorized, and returns the associated username if so.
pub(crate) async fn verify_api_key(
    pool: &Pool,
    key: String,
) -> Result<Option<String>, actix_web::Error> {
    query_internal(
        pool,
        key,
        unixtime_now(),
        "SELECT username FROM api_keys WHERE key_base64 IS ?1 AND expiration > ?2".to_string(),
    )
    .await
}

/// Performs a query that takes two parameters, a string and a u64,
/// and returns the first column of the first row returned as a result if it exists.
/// If it doesn't, it returns `Ok(None)`. `Err(_)` signifies that something went wrong with the DB.
/// DO NOT USE THIS outside of this file! It just happened to be exactly what I needed for two separate DB methods.
async fn query_internal(
    pool: &Pool,
    param1: String,
    param2: u64,
    query: String,
) -> Result<Option<String>, actix_web::Error> {
    // Grab a connection from the pool.
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;

    // rusqlite only has blocking methods, so we'll offload the remainder of this task
    // to the actix-web thread pool and asynchronously await its completion on this thread.
    web::block(move || {
        // Parse the query, and cache the result.
        let mut statement = conn.prepare_cached(&query)?;

        // Perform the query with the given parameters.
        let mut rows = statement.query(params![param1, param2])?;

        // Grab the first row if it exists, and try to get the first column's value.
        if let Some(name) = rows.next()? {
            return Ok(Some(name.get(0)?));
        }

        // If no rows came back, return None.
        Ok::<Option<String>, rusqlite::Error>(None)
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)
}

pub(crate) fn create_pool(config: &Config) -> Pool {
    Pool::new(SqliteConnectionManager::file(&config.db_path)).expect("Failed to open database.")
}
