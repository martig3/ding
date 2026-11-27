use sqlx::SqlitePool;

use crate::Result;
use crate::domain::character::{Character, CharacterRepository};

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
            // The column is NOT NULL; 0 stands for "unknown".
            item_level: c.item_level.map_or(0, i64::from),
            updated_at: c.updated_at,
        }
    }
}

impl From<CharacterRecord> for Character {
    fn from(r: CharacterRecord) -> Self {
        Self {
            first_name: r.first_name,
            last_name: r.last_name,
            name: r.name,
            level: r.level.try_into().unwrap_or_default(),
            item_level: u32::try_from(r.item_level).ok().filter(|&n| n > 0),
            updated_at: r.updated_at,
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

    /// Every stored character. Not part of the domain port; used by the background updater.
    pub async fn list_all(&self) -> Result<Vec<Character>> {
        let rows: Vec<CharacterRecord> = sqlx::query_as(
            "SELECT first_name, last_name, name, level, item_level, updated_at
             FROM characters ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Character::from).collect())
    }
}

impl CharacterRepository for SqliteCharacterRepository {
    async fn save(&self, character: &Character) -> Result<()> {
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
    use crate::infrastructure::sqlite::memory_pool;

    #[tokio::test]
    async fn save_upserts_by_first_and_last_name() {
        let pool = memory_pool().await;
        let repo = SqliteCharacterRepository::new(pool.clone());
        let mut c = Character::new("Foo", "Bar");
        c.name = "Foo Bar".into();
        c.level = 10;
        c.item_level = Some(50);
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

    #[tokio::test]
    async fn list_all_returns_every_character() {
        let repo = SqliteCharacterRepository::new(memory_pool().await);
        assert!(repo.list_all().await.unwrap().is_empty());

        for (f, l, level) in [("Foo", "Bar", 10), ("Baz", "Qux", 20)] {
            let mut c = Character::new(f, l);
            c.level = level;
            repo.save(&c).await.unwrap();
        }
        let all = repo.list_all().await.unwrap();
        let got: Vec<_> = all.iter().map(|c| (c.first_name.as_str(), c.level)).collect();
        assert_eq!(got, [("Foo", 10), ("Baz", 20)]);
    }
}
