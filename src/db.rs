use actix_web::web;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Rows};

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
        internal_get_one_string,
    )
    .await
}

/// Checks if an api_key is authorized, and returns the associated api_key id and username if so.
pub(crate) async fn verify_api_key(
    pool: &Pool,
    key: String,
) -> Result<Option<(u64, String)>, actix_web::Error> {
    query_internal(
        pool,
        key,
        unixtime_now(),
        "SELECT id, username FROM api_keys WHERE key_base64 IS ?1 AND expiration > ?2".to_string(),
        internal_get_u64_string,
    )
    .await
}

/// Gets the first column of the first row of the results, if it exists, treating
/// the first column as a string.
fn internal_get_one_string(mut rows: Rows<'_>) -> Result<Option<String>, rusqlite::Error> {
    // Grab the first row if it exists, and try to get the first column's value.
    if let Some(name) = rows.next()? {
        return Ok(Some(name.get(0)?));
    }

    // If no rows came back, return None.
    Ok(None)
}

/// Gets the first two columns of the first row of the results, if it exists, treating
/// the first column as a u64 and the second column as a string.
fn internal_get_u64_string(mut rows: Rows<'_>) -> Result<Option<(u64, String)>, rusqlite::Error> {
    // Grab the first row if it exists, and try to get the first two columns' values.
    if let Some(name) = rows.next()? {
        return Ok(Some((name.get(0)?, name.get(1)?)));
    }
    // If no rows came back, return None.
    Ok(None)
}

/// Performs a query that takes two parameters, a string and a u64, and returns some arbitrary function of the resulting rows.
/// DO NOT USE THIS outside of this file! It just happened to be a nice abstraction to reduce code repetition.
/// The query string is not sanitized at all. Please don't feed untrusted strings to it. Those should go in parameters.
async fn query_internal<V, F>(
    pool: &Pool,
    param1: String,
    param2: u64,
    query: String,
    do_with_query: F,
) -> Result<Option<V>, actix_web::Error>
where
    F: FnOnce(Rows<'_>) -> Result<Option<V>, rusqlite::Error> + Send + 'static,
    V: Send + 'static,
{
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
        let rows = statement.query(params![param1, param2])?;

        do_with_query(rows)
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)
}

/// Constructs a new pool from the configured options.
pub(crate) fn create_pool(config: &Config) -> Pool {
    Pool::new(SqliteConnectionManager::file(&config.db_path)).expect("Failed to open database.")
}
