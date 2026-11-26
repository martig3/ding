use ding::bot::{Data, Error, character, characters};
use ding::infrastructure::character_sqlite::SqliteCharacterRepository;
use ding::infrastructure::discord_character_sqlite::SqliteDiscordCharacterRepository;
use ding::infrastructure::sqlite::connect;
use ding::jobs;
use poise::serenity_prelude as serenity;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    if let Err(e) = run().await {
        tracing::error!(error = %e, "bot exited with error");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Error> {
    let _ = dotenvy::dotenv();
    let token = std::env::var("DISCORD_TOKEN").map_err(|_| Error::MissingToken)?;

    let pool = connect().await?;
    let character_repo = SqliteCharacterRepository::new(pool.clone());
    let discord_repo = SqliteDiscordCharacterRepository::new(pool.clone());
    // Separate repos for the background updater (they only wrap the shared pool).
    let job_characters = SqliteCharacterRepository::new(pool.clone());
    let job_discord = SqliteDiscordCharacterRepository::new(pool);

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![character(), characters()],
            on_error: |err| {
                Box::pin(async move {
                    match &err {
                        poise::FrameworkError::Command { error, ctx, .. } => {
                            tracing::error!(
                                command = %ctx.command().qualified_name,
                                error = %error,
                                "command failed"
                            );
                        }
                        other => tracing::error!(error = %other, "framework error"),
                    }
                    if let Err(e) = poise::builtins::on_error(err).await {
                        tracing::error!(error = %e, "failed to report error to user");
                    }
                })
            },
            ..Default::default()
        })
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                tracing::info!(user = %ready.user.name, guilds = ready.guilds.len(), "bot started");
                tokio::spawn(jobs::run_update_loop(job_characters, job_discord, ctx.http.clone()));
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
