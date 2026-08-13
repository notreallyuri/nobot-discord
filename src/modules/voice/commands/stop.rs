use crate::{Context, error::AppError, modules::voice::setup};

#[poise::command(slash_command, guild_only)]
pub async fn stop(ctx: Context<'_>) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let call = setup::current_call(ctx).await?;
    let cleared = {
        let call = call.lock().await;
        let queue = call.queue();
        let cleared = queue.len();
        queue.stop();
        cleared
    };

    let message = if cleared == 0 {
        "Nothing was playing.".to_string()
    } else {
        format!("Stopped playback and cleared {cleared} track(s).")
    };

    ctx.send(setup::safe_reply(message)).await?;
    Ok(())
}
