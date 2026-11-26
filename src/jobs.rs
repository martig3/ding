use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use poise::serenity_prelude as serenity;
use tokio::time::{MissedTickBehavior, interval};

use crate::domain::discord_server::DiscordServerRepository;
use crate::infrastructure::character_sqlite::SqliteCharacterRepository;
use crate::infrastructure::discord_character_sqlite::SqliteDiscordCharacterRepository;

/// Env var holding the update interval in seconds.
const UPDATE_INTERVAL_ENV: &str = "UPDATE_INTERVAL_SECS";
const DEFAULT_UPDATE_INTERVAL: Duration = Duration::from_secs(60);

/// Update interval from `UPDATE_INTERVAL_SECS`, falling back to the default when the
/// variable is unset, unparsable, or zero (`interval` panics on a zero period).
fn update_interval() -> Duration {
    match std::env::var(UPDATE_INTERVAL_ENV) {
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(secs) if secs > 0 => Duration::from_secs(secs),
            _ => {
                tracing::warn!(value = %raw, "invalid {UPDATE_INTERVAL_ENV}, using default");
                DEFAULT_UPDATE_INTERVAL
            }
        },
        Err(_) => DEFAULT_UPDATE_INTERVAL,
    }
}
/// Upper bound of the random delay inserted before each request.
const MAX_JITTER: Duration = Duration::from_millis(900);

/// A random delay in `0..MAX_JITTER`. Sub-second clock nanos are plenty random for spacing
/// out requests, so this avoids pulling in an RNG dependency.
fn jitter() -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos() as u64);
    Duration::from_millis(nanos % MAX_JITTER.as_millis() as u64)
}

/// Refreshes every stored character once a minute, one at a time, and tells each server
/// tracking a character when its level changed. Runs forever; spawn it with `tokio::spawn`.
pub async fn run_update_loop(
    characters: SqliteCharacterRepository,
    discord: SqliteDiscordCharacterRepository,
    http: Arc<serenity::Http>,
) {
    let mut ticker = interval(update_interval());
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        update_all(&characters, &discord, &http).await;
    }
}

async fn update_all(
    characters: &SqliteCharacterRepository,
    discord: &SqliteDiscordCharacterRepository,
    http: &serenity::Http,
) {
    let all = match characters.list_all().await {
        Ok(all) => all,
        Err(e) => {
            tracing::error!(error = %e, "failed to list characters for update");
            return;
        }
    };

    let (mut failed, mut notified) = (0usize, 0usize);
    let all_len = all.len();
    for mut character in all.into_iter() {
        let before = character.clone();
        let who = format!("{} {}", character.first_name, character.last_name);

        // jitter the requests
        tokio::time::sleep(jitter()).await;

        if let Err(e) = character.update_character(characters).await {
            failed += 1;
            tracing::error!(error = %e, character = %who, "failed to update character");
            continue;
        }

        if character.level_changed_since(&before) {
            let message = format!(
                "**{}** reached level {}! Grats!",
                character.name, character.level
            );
            notified += notify(discord, http, &character, &message).await;
        }
    }

    tracing::info!(
        characters = all_len,
        failed,
        notified,
        "character update round finished"
    );
}

/// Posts `message` in every subscribed channel; returns how many posts succeeded.
async fn notify(
    discord: &SqliteDiscordCharacterRepository,
    http: &serenity::Http,
    character: &crate::domain::character::Character,
    message: &str,
) -> usize {
    let subscribers = match discord.list_subscribers(character).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, character = %character.name, "failed to list subscribers");
            return 0;
        }
    };

    let mut sent = 0;
    for sub in subscribers {
        let channel = serenity::ChannelId::new(sub.channel_id);
        match channel.say(http, message).await {
            Ok(_) => sent += 1,
            Err(e) => tracing::warn!(
                error = %e,
                guild_id = sub.guild_id,
                channel_id = sub.channel_id,
                "failed to send level notification"
            ),
        }
    }
    sent
}
