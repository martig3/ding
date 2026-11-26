use crate::Result;
use crate::domain::character::Character;

/// Characters shown per page of a server's list.
pub const PAGE_SIZE: i64 = 10;

/// Persistence port for per-server character tracking; implemented in the infrastructure layer.
pub trait DiscordServerRepository {
    /// Links an already-saved character to a server.
    /// Returns `true` if newly tracked, `false` if the server already tracked it.
    fn track(
        &self,
        guild_id: u64,
        character: &Character,
        user_id: u64,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Stops a server tracking a character (names match case-insensitively). If no server
    /// tracks the character afterwards, the character itself is deleted.
    /// Returns `true` if the server was tracking it, `false` otherwise.
    fn untrack(
        &self,
        guild_id: u64,
        first_name: &str,
        last_name: &str,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Number of characters a server tracks.
    fn count_tracked(&self, guild_id: u64) -> impl Future<Output = Result<i64>> + Send;

    /// A slice of a server's characters, alphabetical by last then first name.
    fn list_tracked(
        &self,
        guild_id: u64,
        offset: i64,
        limit: i64,
    ) -> impl Future<Output = Result<Vec<Character>>> + Send;
}

/// One page of a server's alphabetical character list.
#[derive(Debug, Clone)]
pub struct CharacterPage {
    pub characters: Vec<Character>,
    /// Zero-based index of this page.
    pub page: i64,
    /// Total number of pages (at least 1, even when the list is empty).
    pub pages: i64,
    /// Total number of tracked characters across all pages.
    pub total: i64,
}

impl CharacterPage {
    pub fn has_prev(&self) -> bool {
        self.page > 0
    }

    pub fn has_next(&self) -> bool {
        self.page + 1 < self.pages
    }
}

/// A Discord server (guild) and the characters it tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscordServer {
    pub guild_id: u64,
}

impl DiscordServer {
    pub fn new(guild_id: u64) -> Self {
        Self { guild_id }
    }

    /// Tracks `character` on this server on behalf of Discord user `user_id`.
    /// Returns `true` if newly tracked, `false` if it was already tracked.
    pub async fn track_character(
        &self,
        repo: &impl DiscordServerRepository,
        character: &Character,
        user_id: u64,
    ) -> Result<bool> {
        repo.track(self.guild_id, character, user_id).await
    }

    /// Stops tracking the named character on this server, removing the character entirely
    /// if no other server tracks it. Returns `true` if it was tracked here.
    pub async fn untrack_character(
        &self,
        repo: &impl DiscordServerRepository,
        first_name: &str,
        last_name: &str,
    ) -> Result<bool> {
        repo.untrack(self.guild_id, first_name, last_name).await
    }

    /// Fetches one page of this server's characters. `page` is zero-based and is clamped
    /// into the valid range, so the returned `page` is the one actually loaded.
    pub async fn characters_page(
        &self,
        repo: &impl DiscordServerRepository,
        page: i64,
    ) -> Result<CharacterPage> {
        let total = repo.count_tracked(self.guild_id).await?;
        let pages = ((total + PAGE_SIZE - 1) / PAGE_SIZE).max(1);
        let page = page.clamp(0, pages - 1);
        let characters = repo.list_tracked(self.guild_id, page * PAGE_SIZE, PAGE_SIZE).await?;
        Ok(CharacterPage { characters, page, pages, total })
    }
}
