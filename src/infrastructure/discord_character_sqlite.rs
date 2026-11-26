use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::SqlitePool;

use crate::Result;
use crate::domain::character::Character;
use crate::domain::discord_server::{DiscordServerRepository, Subscriber};
use crate::infrastructure::character_sqlite::CharacterRecord;

/// Which characters each Discord server tracks (`server_characters`).
pub struct SqliteDiscordCharacterRepository {
    pool: SqlitePool,
}

impl SqliteDiscordCharacterRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

impl DiscordServerRepository for SqliteDiscordCharacterRepository {
    async fn track(
        &self,
        guild_id: u64,
        character: &Character,
        user_id: u64,
        channel_id: u64,
    ) -> Result<bool> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO server_characters (guild_id, character_id, added_by, added_at, channel_id)
             SELECT ?, id, ?, ?, ? FROM characters WHERE first_name = ? AND last_name = ?
             ON CONFLICT (guild_id, character_id) DO NOTHING",
        )
        .bind(guild_id.to_string())
        .bind(user_id.to_string())
        .bind(now)
        .bind(channel_id.to_string())
        .bind(&character.first_name)
        .bind(&character.last_name)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;

        if !inserted {
            // Already tracked: just point notifications at the channel used now.
            sqlx::query(
                "UPDATE server_characters SET channel_id = ?
                 WHERE guild_id = ?
                   AND character_id IN (
                       SELECT id FROM characters WHERE first_name = ? AND last_name = ?)",
            )
            .bind(channel_id.to_string())
            .bind(guild_id.to_string())
            .bind(&character.first_name)
            .bind(&character.last_name)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(inserted)
    }

    async fn list_subscribers(&self, character: &Character) -> Result<Vec<Subscriber>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT sc.guild_id, sc.channel_id
             FROM server_characters sc
             JOIN characters c ON c.id = sc.character_id
             WHERE c.first_name = ? AND c.last_name = ? AND sc.channel_id IS NOT NULL",
        )
        .bind(&character.first_name)
        .bind(&character.last_name)
        .fetch_all(&self.pool)
        .await?;
        // Ids are stored as text; skip any row that somehow isn't a valid snowflake.
        Ok(rows
            .into_iter()
            .filter_map(|(g, c)| {
                Some(Subscriber { guild_id: g.parse().ok()?, channel_id: c.parse().ok()? })
            })
            .collect())
    }

    async fn untrack(&self, guild_id: u64, first_name: &str, last_name: &str) -> Result<bool> {
        let mut tx = self.pool.begin().await?;
        let removed = sqlx::query(
            "DELETE FROM server_characters
             WHERE guild_id = ?
               AND character_id IN (
                   SELECT id FROM characters
                   WHERE first_name = ? COLLATE NOCASE AND last_name = ? COLLATE NOCASE)",
        )
        .bind(guild_id.to_string())
        .bind(first_name)
        .bind(last_name)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;

        if removed {
            // Drop the character once no server tracks it any more.
            sqlx::query(
                "DELETE FROM characters
                 WHERE first_name = ? COLLATE NOCASE AND last_name = ? COLLATE NOCASE
                   AND NOT EXISTS (
                       SELECT 1 FROM server_characters WHERE character_id = characters.id)",
            )
            .bind(first_name)
            .bind(last_name)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(removed)
    }

    async fn count_tracked(&self, guild_id: u64) -> Result<i64> {
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM server_characters WHERE guild_id = ?")
                .bind(guild_id.to_string())
                .fetch_one(&self.pool)
                .await?;
        Ok(n)
    }

    async fn list_tracked(&self, guild_id: u64, offset: i64, limit: i64) -> Result<Vec<Character>> {
        let rows: Vec<CharacterRecord> = sqlx::query_as(
            "SELECT c.first_name, c.last_name, c.name, c.level, c.item_level, c.updated_at
             FROM server_characters sc
             JOIN characters c ON c.id = sc.character_id
             WHERE sc.guild_id = ?
             ORDER BY c.last_name COLLATE NOCASE, c.first_name COLLATE NOCASE
             LIMIT ? OFFSET ?",
        )
        .bind(guild_id.to_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(Character::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::character::CharacterRepository;
    use crate::domain::discord_server::DiscordServer;
    use crate::infrastructure::character_sqlite::SqliteCharacterRepository;
    use crate::infrastructure::sqlite::memory_pool;

    async fn saved(repo: &SqliteCharacterRepository, first: &str, last: &str) -> Character {
        let mut c = Character::new(first, last);
        c.name = format!("{first} {last}");
        repo.save(&c).await.unwrap();
        c
    }

    #[tokio::test]
    async fn tracking_is_per_guild_sorted_paged_and_idempotent() {
        let pool = memory_pool().await;
        let characters = SqliteCharacterRepository::new(pool.clone());
        let repo = SqliteDiscordCharacterRepository::new(pool);
        let (one, two) = (DiscordServer::new(1), DiscordServer::new(2));

        for (f, l) in [("Zed", "Beta"), ("Amy", "alpha"), ("Bob", "Gamma")] {
            let c = saved(&characters, f, l).await;
            assert!(one.track_character(&repo, &c, 9, 100).await.unwrap());
        }
        let zed = saved(&characters, "Zed", "Beta").await;
        assert!(!one.track_character(&repo, &zed, 9, 100).await.unwrap());
        let other = saved(&characters, "Other", "Zulu").await;
        assert!(two.track_character(&repo, &other, 9, 100).await.unwrap());

        assert_eq!(repo.count_tracked(1).await.unwrap(), 3);
        assert_eq!(repo.count_tracked(2).await.unwrap(), 1);

        let page = repo.list_tracked(1, 0, 2).await.unwrap();
        let last_names: Vec<_> = page.iter().map(|c| c.last_name.as_str()).collect();
        assert_eq!(last_names, ["alpha", "Beta"]);
        let page = repo.list_tracked(1, 2, 2).await.unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].last_name, "Gamma");

        // Domain-level paging: one page, with out-of-range pages clamped.
        let p = one.characters_page(&repo, 5).await.unwrap();
        assert_eq!((p.page, p.pages, p.total, p.characters.len()), (0, 1, 3, 3));
        assert!(!p.has_prev() && !p.has_next());
        let empty = DiscordServer::new(3).characters_page(&repo, 0).await.unwrap();
        assert_eq!((empty.pages, empty.total), (1, 0));
    }

    #[tokio::test]
    async fn subscribers_use_latest_channel_and_skip_unset() {
        let pool = memory_pool().await;
        let characters = SqliteCharacterRepository::new(pool.clone());
        let repo = SqliteDiscordCharacterRepository::new(pool.clone());
        let (one, two, three) = (DiscordServer::new(1), DiscordServer::new(2), DiscordServer::new(3));

        let c = saved(&characters, "Amy", "Alpha").await;
        assert!(one.track_character(&repo, &c, 9, 100).await.unwrap());
        assert!(two.track_character(&repo, &c, 9, 200).await.unwrap());
        assert!(three.track_character(&repo, &c, 9, 300).await.unwrap());
        // Re-tracking from another channel moves guild 1's notifications.
        assert!(!one.track_character(&repo, &c, 9, 101).await.unwrap());
        // A row from before channels existed has no channel and is skipped.
        sqlx::query("UPDATE server_characters SET channel_id = NULL WHERE guild_id = '3'")
            .execute(&pool)
            .await
            .unwrap();

        let mut subs = repo.list_subscribers(&c).await.unwrap();
        subs.sort_by_key(|s| s.guild_id);
        assert_eq!(
            subs,
            [
                Subscriber { guild_id: 1, channel_id: 101 },
                Subscriber { guild_id: 2, channel_id: 200 },
            ]
        );

        let other = saved(&characters, "Other", "Zulu").await;
        assert!(repo.list_subscribers(&other).await.unwrap().is_empty());
    }

    async fn character_rows(pool: &SqlitePool) -> i64 {
        let (n,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM characters").fetch_one(pool).await.unwrap();
        n
    }

    #[tokio::test]
    async fn untrack_is_per_guild_and_deletes_orphaned_characters() {
        let pool = memory_pool().await;
        let characters = SqliteCharacterRepository::new(pool.clone());
        let repo = SqliteDiscordCharacterRepository::new(pool.clone());
        let (one, two) = (DiscordServer::new(1), DiscordServer::new(2));

        let shared = saved(&characters, "Amy", "Alpha").await;
        assert!(one.track_character(&repo, &shared, 9, 100).await.unwrap());
        assert!(two.track_character(&repo, &shared, 9, 100).await.unwrap());

        // Guild 1 stops tracking: guild 2 still does, so the character stays.
        assert!(one.untrack_character(&repo, "amy", "ALPHA").await.unwrap());
        assert_eq!(repo.count_tracked(1).await.unwrap(), 0);
        assert_eq!(repo.count_tracked(2).await.unwrap(), 1);
        assert_eq!(character_rows(&pool).await, 1);

        // Repeating, or removing an unknown character, is a no-op.
        assert!(!one.untrack_character(&repo, "Amy", "Alpha").await.unwrap());
        assert!(!one.untrack_character(&repo, "No", "Body").await.unwrap());

        // Last guild stops tracking: the character is deleted.
        assert!(two.untrack_character(&repo, "Amy", "Alpha").await.unwrap());
        assert_eq!(character_rows(&pool).await, 0);
    }
}
