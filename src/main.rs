mod api;
mod domain;
mod infrastructure;

use infrastructure::sqlite::{SqliteCharacterRepository, connect};

#[tokio::main]
async fn main() {
    let pool = match connect().await {
        Ok(pool) => pool,
        Err(e) => return eprintln!("database error: {e}"),
    };
    let repo = SqliteCharacterRepository::new(pool);

    let mut character = domain::character::Character::new("Martige", "Sprockets");
    match character.update_character(&repo).await {
        Ok(()) => println!("{character:#?}"),
        Err(e) => eprintln!("{e}"),
    }
}
