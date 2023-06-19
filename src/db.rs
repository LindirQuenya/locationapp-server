use actix_web::web;
use rusqlite::params;

use crate::misc::unixtime_now;

pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub async fn verify_email(pool: &Pool, email: String) -> Result<Option<String>, actix_web::Error> {
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
pub async fn verify_api_key(pool: &Pool, key: String) -> Result<bool, actix_web::Error> {
    let pool = pool.clone();
    let conn = web::block(move || pool.get())
        .await?
        .map_err(actix_web::error::ErrorInternalServerError)?;
    web::block(move || {
        let mut statement = conn.prepare_cached(
            "SELECT COUNT(*) FROM api_keys WHERE key_base64 IS ?1 AND expiration > ?2",
        )?;
        statement.query_row(params![key, unixtime_now()], |row| {
            let count: i32 = row.get(0)?;
            Ok(count != 0)
        })
    })
    .await?
    .map_err(actix_web::error::ErrorInternalServerError)
}
