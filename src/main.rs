use ding::bot::{Data, Error, character, characters};
use ding::infrastructure::character_sqlite::SqliteCharacterRepository;
use ding::infrastructure::discord_character_sqlite::SqliteDiscordCharacterRepository;
use ding::infrastructure::sqlite::connect;
use poise::serenity_prelude as serenity;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = dotenvy::dotenv();
    let token = std::env::var("DISCORD_TOKEN").map_err(|_| Error::MissingToken)?;

    let pool = connect().await?;
    let character_repo = SqliteCharacterRepository::new(pool.clone());
    let discord_repo = SqliteDiscordCharacterRepository::new(pool);

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![character(), characters()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { characters: character_repo, discord: discord_repo })
            })
        })
        .build();

    let mut client = serenity::ClientBuilder::new(token, serenity::GatewayIntents::non_privileged())
        .framework(framework)
        .await?;
    client.start().await?;
    Ok(())
}
