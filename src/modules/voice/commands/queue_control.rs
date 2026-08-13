use crate::{
    Context,
    error::AppError,
    modules::voice::{repeat, setup},
};

fn guild_of(ctx: Context<'_>) -> Result<i64, AppError> {
    ctx.guild_id()
        .map(|id| id.get() as i64)
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))
}

#[poise::command(slash_command, guild_only)]
pub async fn shuffle(ctx: Context<'_>) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let call = setup::current_call(ctx).await?;

    let moved = {
        let call = call.lock().await;
        call.queue().modify_queue(repeat::shuffle)
    };

    let message = match moved {
        0 => "There aren't enough queued tracks to shuffle.".to_string(),
        moved => format!("Shuffled {moved} track(s). The current one keeps playing."),
    };

    ctx.send(setup::safe_reply(message)).await?;
    Ok(())
}

#[poise::command(slash_command, guild_only)]
pub async fn repeat(
    ctx: Context<'_>,
    #[description = "What to repeat"] mode: repeat::Mode,
) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let guild_id = guild_of(ctx)?;
    let call = setup::current_call(ctx).await?;

    let current = {
        let call = call.lock().await;
        call.queue().current()
    };

    if let Some(handle) = current {
        let looping = if mode == repeat::Mode::Track {
            handle.enable_loop()
        } else {
            handle.disable_loop()
        };

        if let Err(e) = looping {
            tracing::warn!(?e, "failed to change the track loop");
        }
    }

    repeat::set(&ctx.data().repeat, guild_id, mode);

    ctx.send(setup::safe_reply(mode.describe())).await?;
    Ok(())
}
