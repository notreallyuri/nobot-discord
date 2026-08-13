use crate::{Context, error::AppError, modules::voice::setup};

#[poise::command(slash_command, guild_only)]
pub async fn skip(ctx: Context<'_>) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let call = setup::current_call(ctx).await?;
    let call = call.lock().await;
    let queue = call.queue();

    let Some(current) = queue.current() else {
        return Err(AppError::Message("Nothing is playing.".into()));
    };

    let title = current.data::<setup::TrackMeta>().title.clone();
    let remaining = queue.len().saturating_sub(1);

    queue
        .skip()
        .map_err(|e| AppError::Message(format!("Couldn't skip: {e}")))?;

    drop(call);

    let tail = if remaining == 0 {
        " Queue is now empty.".to_string()
    } else {
        format!(" {remaining} left in the queue.")
    };

    ctx.send(setup::safe_reply(format!("Skipped **{title}**.{tail}")))
        .await?;

    Ok(())
}
