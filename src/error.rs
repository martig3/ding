use crate::api::ParseError;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors from fetching, parsing and persisting characters.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{first} {last} not found")]
    CharacterNotFound { first: String, last: String },
    #[error("failed to fetch character: {0}")]
    Fetch(#[from] reqwest::Error),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("system clock error: {0}")]
    Clock(#[from] std::time::SystemTimeError),
}
