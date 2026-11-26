use std::error::Error;

use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;

use crate::domain::character::{Character, CharacterRepository};

/// Database file, relative to the working directory.
const DB_FILE: &str = "ding.db";

/// Opens (creating if needed) the database and applies pending migrations.
pub async fn connect() -> Result<SqlitePool, Box<dyn Error>> {
    let options = SqliteConnectOptions::new().filename(DB_FILE).create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

/// Persistence schema for a row in `characters`. Kept separate from the domain entity.
#[derive(Debug, sqlx::FromRow)]
pub struct CharacterRecord {
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    pub level: i64,
    pub item_level: i64,
    pub updated_at: i64,
}

impl From<&Character> for CharacterRecord {
    fn from(c: &Character) -> Self {
        Self {
            first_name: c.first_name.clone(),
            last_name: c.last_name.clone(),
            name: c.name.clone(),
            level: c.level.into(),
            item_level: c.item_level.into(),
            updated_at: c.updated_at,
        }
    }
}

pub struct SqliteCharacterRepository {
    pool: SqlitePool,
}

impl SqliteCharacterRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl CharacterRepository for SqliteCharacterRepository {
    async fn save(&self, character: &Character) -> Result<(), Box<dyn Error>> {
        let r = CharacterRecord::from(character);
        sqlx::query!(
            "INSERT INTO characters (first_name, last_name, name, level, item_level, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (first_name, last_name) DO UPDATE SET
                 name = excluded.name,
                 level = excluded.level,
                 item_level = excluded.item_level,
                 updated_at = excluded.updated_at",
            r.first_name,
            r.last_name,
            r.name,
            r.level,
            r.item_level,
            r.updated_at,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn repo() -> (SqliteCharacterRepository, SqlitePool) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        (SqliteCharacterRepository::new(pool.clone()), pool)
    }

    #[tokio::test]
    async fn save_upserts_by_first_and_last_name() {
        let (repo, pool) = repo().await;
        let mut c = Character::new("Foo", "Bar");
        c.name = "Foo Bar".into();
        c.level = 10;
        c.item_level = 50;
        c.updated_at = 100;
        repo.save(&c).await.unwrap();

        c.level = 11;
        c.updated_at = 200;
        repo.save(&c).await.unwrap();

        let rows: Vec<CharacterRecord> = sqlx::query_as("SELECT * FROM characters")
            .fetch_all(&pool)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].level, rows[0].updated_at), (11, 200));
    }
}
