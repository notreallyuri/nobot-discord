use crate::{
    Context,
    error::AppError,
    modules::voice::setup::{self, TrackMeta},
};

#[poise::command(slash_command, guild_only)]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Position in the queue, as shown by /queue"]
    #[min = 1]
    position: u32,
) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let call = setup::current_call(ctx).await?;
    let index = position as usize;

    let title = {
        let call = call.lock().await;
        let queue = call.queue();

        let upcoming = queue.len().saturating_sub(1);
        if upcoming == 0 {
            return Err(AppError::Message(
                "Nothing is queued behind the current track.".into(),
            ));
        }
        if index > upcoming {
            return Err(AppError::Message(format!(
                "There's no #{position} in the queue — it holds {upcoming} track(s)."
            )));
        }

        let removed = queue
            .dequeue(index)
            .ok_or_else(|| AppError::Message("That track just went away.".into()))?;

        let title = removed.data::<TrackMeta>().title.clone();
        drop(removed.stop());
        title
    };

    ctx.send(setup::safe_reply(format!(
        "Removed **{title}** from the queue."
    )))
    .await?;

    Ok(())
}
