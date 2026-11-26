use std::str::FromStr;

use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

use crate::Result;

/// Default database URL, relative to the working directory.
const DEFAULT_DATABASE_URL: &str = "sqlite:ding.db";

/// Opens (creating if needed) the database at `DATABASE_URL` (default `sqlite:ding.db`)
/// and applies pending migrations.
pub async fn connect() -> Result<SqlitePool> {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
    let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// In-memory database with migrations applied, for tests.
#[cfg(test)]
pub(crate) async fn memory_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}
