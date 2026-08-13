use crate::{Context, HttpKey, error::AppError, modules::voice::idle};
use poise::serenity_prelude as serenity;
use songbird::{Call, Songbird};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;

pub struct TrackMeta {
    pub title: String,
    pub artist: Option<String>,
    pub url: Option<String>,
    pub duration: Option<Duration>,
    pub thumbnail: Option<String>,
    pub requester: serenity::UserId,
}

pub async fn manager(ctx: Context<'_>) -> Result<Arc<Songbird>, AppError> {
    songbird::get(ctx.serenity_context())
        .await
        .ok_or_else(|| AppError::Message("Voice subsystem not initialised.".into()))
}

pub async fn http_client(ctx: Context<'_>) -> Result<reqwest::Client, AppError> {
    ctx.serenity_context()
        .data
        .read()
        .await
        .get::<HttpKey>()
        .cloned()
        .ok_or_else(|| AppError::Message("HTTP client not initialised.".into()))
}

pub fn author_voice_channel(ctx: Context<'_>) -> Option<serenity::ChannelId> {
    let guild = ctx.guild()?;
    guild
        .voice_states
        .get(&ctx.author().id)
        .and_then(|vs| vs.channel_id)
}

pub async fn current_call(ctx: Context<'_>) -> Result<Arc<Mutex<Call>>, AppError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))?;

    let call = manager(ctx)
        .await?
        .get(guild_id)
        .ok_or_else(|| AppError::Message("I'm not in a voice channel.".into()))?;

    let connected = call.lock().await.current_channel().is_some();
    if !connected {
        return Err(AppError::Message("I'm not in a voice channel.".into()));
    }

    Ok(call)
}

pub async fn join_or_get(ctx: Context<'_>) -> Result<Arc<Mutex<Call>>, AppError> {
    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))?;
    let manager = manager(ctx).await?;
    let author_channel = author_voice_channel(ctx);

    if let Some(call) = manager.get(guild_id) {
        let bot_channel = call.lock().await.current_channel();
        if let Some(bot_channel) = bot_channel {
            return match author_channel {
                Some(author_channel) if author_channel.get() == bot_channel.0.get() => {
                    idle::remember(&ctx.data().voice_sessions, guild_id, ctx.channel_id());
                    Ok(call)
                }
                _ => Err(AppError::Message(format!(
                    "I'm already in <#{}> — join me there first.",
                    bot_channel.0
                ))),
            };
        }
    }

    let Some(author_channel) = author_channel else {
        return Err(AppError::Message(
            "You need to be in a voice channel first.".into(),
        ));
    };

    let call = manager
        .join(guild_id, author_channel)
        .await
        .map_err(|e| AppError::Message(format!("Failed to join voice channel: {e}")))?;

    if let Err(e) = call.lock().await.deafen(true).await {
        tracing::warn!(?e, "failed to self-deafen");
    }

    idle::remember(&ctx.data().voice_sessions, guild_id, ctx.channel_id());

    Ok(call)
}

pub fn safe_reply(text: impl Into<String>) -> poise::CreateReply {
    poise::CreateReply::default()
        .content(text.into())
        .allowed_mentions(serenity::CreateAllowedMentions::new())
}

pub fn fmt_duration(d: Duration) -> String {
    let total = d.as_secs();
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn fmt_len(duration: Option<Duration>) -> String {
    duration.map_or_else(|| "live".to_string(), fmt_duration)
}

pub fn progress_bar(position: Duration, total: Option<Duration>) -> String {
    let Some(total) = total.filter(|t| !t.is_zero()) else {
        return format!("{} elapsed", fmt_duration(position));
    };

    const WIDTH: usize = 20;
    let ratio = (position.as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0);
    let filled = ((ratio * WIDTH as f64).round() as usize).min(WIDTH - 1);

    format!(
        "{} {}●{} {}",
        fmt_duration(position),
        "━".repeat(filled),
        "─".repeat(WIDTH - 1 - filled),
        fmt_duration(total),
    )
}
