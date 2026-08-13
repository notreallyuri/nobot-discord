use crate::{
    Context,
    error::AppError,
    modules::voice::{idle, setup},
};

/// Leave the voice channel
#[poise::command(slash_command, guild_only)]
pub async fn leave(ctx: Context<'_>) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let guild_id = ctx
        .guild_id()
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))?;

    let manager = setup::manager(ctx).await?;

    if manager.get(guild_id).is_none() {
        return Err(AppError::Message("I'm not in a voice channel.".into()));
    }

    idle::forget(&ctx.data().voice_sessions, guild_id);

    manager
        .remove(guild_id)
        .await
        .map_err(|e| AppError::Message(format!("Failed to leave: {e}")))?;

    let data = ctx.data();
    let guild = guild_id.get() as i64;

    if data.guild_config(guild).await.stays_connected() {
        let forget = crate::guild_config::Setting::VoiceChannels(None, None);
        if let Err(e) =
            crate::guild_config::apply(&data.db, &data.guild_config, guild, forget).await
        {
            tracing::warn!(?e, guild, "failed to forget the 24/7 voice channel");
        }
    }

    ctx.send(setup::safe_reply(
        "Left the voice channel. I won't rejoin on restart until you `/play` again.",
    ))
    .await?;
    Ok(())
}
