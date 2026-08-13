use crate::{Context, error::AppError, modules::voice::setup};

#[poise::command(slash_command, guild_only)]
pub async fn clear(ctx: Context<'_>) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let call = setup::current_call(ctx).await?;

    let cleared = {
        let call = call.lock().await;
        let queue = call.queue();

        let removed = queue.modify_queue(|q| {
            if q.len() <= 1 {
                Vec::new()
            } else {
                q.drain(1..).collect::<Vec<_>>()
            }
        });

        let cleared = removed.len();
        for track in removed {
            drop(track.stop());
        }
        cleared
    };

    let message = if cleared == 0 {
        "Nothing was queued behind the current track.".to_string()
    } else {
        format!("Cleared {cleared} track(s) from the queue.")
    };

    ctx.send(setup::safe_reply(message)).await?;
    Ok(())
}
