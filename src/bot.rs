use std::time::Duration;

use crate::domain::character::Character;
use crate::infrastructure::character_sqlite::SqliteCharacterRepository;
use crate::domain::discord_server::{CharacterPage, DiscordServer};
use crate::infrastructure::discord_character_sqlite::SqliteDiscordCharacterRepository;
use poise::serenity_prelude as serenity;

/// Errors surfaced by bot commands.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Core(#[from] crate::Error),
    #[error("discord error: {0}")]
    Discord(#[from] serenity::Error),
    #[error("this command only works in a server")]
    GuildOnly,
    #[error("DISCORD_TOKEN is not set")]
    MissingToken,
}
pub type Context<'a> = poise::Context<'a, Data, Error>;

pub struct Data {
    pub characters: SqliteCharacterRepository,
    pub discord: SqliteDiscordCharacterRepository,
}

/// Manage tracked characters.
#[poise::command(slash_command, guild_only, subcommands("track", "remove"))]
pub async fn character(_: Context<'_>) -> Result<(), Error> {
    Ok(())
}

/// Stop tracking a character on this server.
#[poise::command(slash_command, guild_only)]
async fn remove(
    ctx: Context<'_>,
    #[description = "Character first name"] first_name: String,
    #[description = "Character last name"] last_name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(Error::GuildOnly)?;
    let (first, last) = (first_name.trim(), last_name.trim());
    if first.is_empty() || last.is_empty() {
        ctx.say("Please provide both a first and last name.").await?;
        return Ok(());
    }

    let removed = DiscordServer::new(guild_id.get())
        .untrack_character(&ctx.data().discord, first, last)
        .await?;
    let msg = if removed {
        format!("Stopped tracking **{first} {last}**.")
    } else {
        format!("**{first} {last}** isn't tracked on this server.")
    };
    ctx.say(msg).await?;
    Ok(())
}

/// Track a character on this server.
#[poise::command(slash_command, guild_only)]
async fn track(
    ctx: Context<'_>,
    #[description = "Character first name"] first_name: String,
    #[description = "Character last name"] last_name: String,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or(Error::GuildOnly)?;
    let (first, last) = (first_name.trim(), last_name.trim());
    if first.is_empty() || last.is_empty() {
        ctx.say("Please provide both a first and last name.").await?;
        return Ok(());
    }

    // Fetching from the armory can take longer than Discord's 3s response window.
    ctx.defer().await?;

    let data = ctx.data();
    let mut character = Character::new(first, last);
    if let Err(e) = character.update_character(&data.characters).await {
        tracing::error!(error = %e, character = %format!("{first} {last}"), "failed to fetch character");
        // The technical detail is in the log above; users just see that it wasn't found.
        ctx.say(format!("**{first} {last}** couldn't be found.")).await?;
        return Ok(());
    }

    let added = DiscordServer::new(guild_id.get())
        .track_character(
            &data.discord,
            &character,
            ctx.author().id.get(),
            ctx.channel_id().get(),
        )
        .await?;
    let channel = ctx.channel_id();
    let msg = if added {
        format!(
            "Now tracking **{}** (level {}, ilvl {}). Level-up updates will be posted in <#{channel}>.",
            character.name, character.level, character.item_level
        )
    } else {
        format!(
            "**{}** is already tracked on this server (info refreshed). Level-up updates will now be posted in <#{channel}>.",
            character.name
        )
    };
    ctx.say(msg).await?;
    Ok(())
}

fn page_embed(list: &CharacterPage) -> serenity::CreateEmbed {
    let body = list
        .characters
        .iter()
        .map(|c| format!("**{}** — level {}, ilvl {}", c.name, c.level, c.item_level))
        .collect::<Vec<_>>()
        .join("\n");
    serenity::CreateEmbed::new()
        .title("Tracked characters")
        .description(body)
        .footer(serenity::CreateEmbedFooter::new(format!(
            "Page {}/{} • {} characters",
            list.page + 1,
            list.pages,
            list.total
        )))
}

fn page_buttons(prev_id: &str, next_id: &str, list: &CharacterPage) -> serenity::CreateActionRow {
    serenity::CreateActionRow::Buttons(vec![
        serenity::CreateButton::new(prev_id).emoji('◀').disabled(!list.has_prev()),
        serenity::CreateButton::new(next_id).emoji('▶').disabled(!list.has_next()),
    ])
}

/// List the characters tracked on this server.
#[poise::command(slash_command, guild_only)]
pub async fn characters(ctx: Context<'_>) -> Result<(), Error> {
    let server = DiscordServer::new(ctx.guild_id().ok_or(Error::GuildOnly)?.get());
    let repo = &ctx.data().discord;

    let mut list = server.characters_page(repo, 0).await?;
    if list.total == 0 {
        ctx.send(
            poise::CreateReply::default()
                .content("No characters are tracked on this server yet. Use `/character track`.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }
    let ctx_id = ctx.id();
    let prev_id = format!("{ctx_id}prev");
    let next_id = format!("{ctx_id}next");

    let reply = poise::CreateReply::default()
        .embed(page_embed(&list))
        .components(vec![page_buttons(&prev_id, &next_id, &list)])
        .ephemeral(true);
    let handle = ctx.send(reply).await?;

    while let Some(press) = serenity::ComponentInteractionCollector::new(ctx)
        .filter(move |press| press.data.custom_id.starts_with(&ctx_id.to_string()))
        .timeout(Duration::from_secs(600))
        .await
    {
        let target = if press.data.custom_id == next_id {
            list.page + 1
        } else if press.data.custom_id == prev_id {
            list.page - 1
        } else {
            continue;
        };
        list = server.characters_page(repo, target).await?;

        press
            .create_response(
                ctx.serenity_context(),
                serenity::CreateInteractionResponse::UpdateMessage(
                    serenity::CreateInteractionResponseMessage::new()
                        .embed(page_embed(&list))
                        .components(vec![page_buttons(&prev_id, &next_id, &list)]),
                ),
            )
            .await?;
    }

    // Timed out: remove the buttons.
    handle.edit(ctx, poise::CreateReply::default().components(vec![])).await?;
    Ok(())
}
