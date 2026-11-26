use std::time::{SystemTime, UNIX_EPOCH};

use crate::{Result, api};

/// Persistence port for characters; implemented in the infrastructure layer.
pub trait CharacterRepository {
    /// Inserts the character, or updates it if one with the same first/last name exists.
    fn save(
        &self,
        character: &Character,
    ) -> impl Future<Output = Result<()>> + Send;
}

/// Holds a character's data. Create with `new`, then call `update_character` to fill it in.
#[derive(Debug, Clone)]
pub struct Character {
    pub first_name: String,
    pub last_name: String,
    pub name: String,
    pub level: u32,
    pub item_level: u32,
    /// Unix timestamp (seconds) of the last successful update; 0 if never updated.
    pub updated_at: i64,
}

impl Character {
    pub fn new(first_name: &str, last_name: &str) -> Self {
        Self {
            first_name: first_name.to_string(),
            last_name: last_name.to_string(),
            name: String::new(),
            level: 0,
            item_level: 0,
            updated_at: 0,
        }
    }

    /// Fetches the armory page, refreshes `name`, `level`, `item_level` and `updated_at`,
    /// and persists the result through `repo`.
    /// On error (fetch or save) the struct is left unchanged.
    pub async fn update_character(
        &mut self,
        repo: &impl CharacterRepository,
    ) -> Result<()> {
        let profile = api::fetch_profile(&self.first_name, &self.last_name).await?;

        let mut updated = self.clone();
        updated.name = profile.name;
        updated.level = profile.level;
        updated.item_level = profile.item_level;
        updated.updated_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        repo.save(&updated).await?;
        *self = updated;
        Ok(())
    }
}
