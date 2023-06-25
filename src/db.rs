use std::env;

use actix_web::web;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;

use crate::misc::unixtime_now;

pub(crate) type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;

pub(crate) async fn verify_email(
    pool: &Pool,
    email: String,
) -> Result<Option<String>, actix_web::Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;
    web::block(move || {
        let mut statement = conn.prepare_cached(
            "SELECT username FROM web_users WHERE email IS ?1 AND expiration > ?2",
        )?;
        let mut rows = statement.query(params![email, unixtime_now()])?;
        if let Some(name) = rows.next()? {
            return Ok(Some(name.get(0)?));
        }
        Ok::<Option<String>, rusqlite::Error>(None)
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)
}
pub(crate) async fn verify_api_key(
    pool: &Pool,
    key: String,
) -> Result<Option<String>, actix_web::Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;
    web::block(move || {
        let mut statement = conn.prepare_cached(
            "SELECT username FROM api_keys WHERE key_base64 IS ?1 AND expiration > ?2",
        )?;
        let mut rows = statement.query(params![key, unixtime_now()])?;
        if let Some(name) = rows.next()? {
            return Ok(Some(name.get(0)?));
        }
        Ok::<Option<String>, rusqlite::Error>(None)
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)
}

pub(crate) fn create_pool() -> Pool {
    Pool::new(SqliteConnectionManager::file(
        env::var("DB_PATH").expect("Missing the DB_PATH environment variable."),
    ))
    .expect("Failed to open database.")
}
