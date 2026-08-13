use crate::{Context, error::AppError, modules::voice::setup};
use songbird::tracks::{PlayMode, TrackHandle};

struct Current {
    handle: TrackHandle,
    title: String,
    paused: bool,
}

async fn current_track(ctx: Context<'_>) -> Result<Current, AppError> {
    let call = setup::current_call(ctx).await?;

    let handle = {
        let call = call.lock().await;
        call.queue().current()
    };

    let Some(handle) = handle else {
        return Err(AppError::Message("Nothing is playing.".into()));
    };

    let title = handle.data::<setup::TrackMeta>().title.clone();
    let paused = handle
        .get_info()
        .await
        .is_ok_and(|state| matches!(state.playing, PlayMode::Pause));

    Ok(Current {
        handle,
        title,
        paused,
    })
}

#[poise::command(slash_command, guild_only)]
pub async fn pause(ctx: Context<'_>) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let current = current_track(ctx).await?;

    if current.paused {
        return Err(AppError::Message(format!(
            "**{}** is already paused — use `/resume` to continue.",
            current.title
        )));
    }

    current
        .handle
        .pause()
        .map_err(|e| AppError::Message(format!("Couldn't pause: {e}")))?;

    ctx.send(setup::safe_reply(format!(
        "Paused **{}** — use `/resume` to continue.",
        current.title
    )))
    .await?;

    Ok(())
}

#[poise::command(slash_command, guild_only)]
pub async fn resume(ctx: Context<'_>) -> Result<(), AppError> {
    setup::require_dj(ctx).await?;

    let current = current_track(ctx).await?;

    if !current.paused {
        return Err(AppError::Message(format!(
            "**{}** is already playing.",
            current.title
        )));
    }

    current
        .handle
        .play()
        .map_err(|e| AppError::Message(format!("Couldn't resume: {e}")))?;

    ctx.send(setup::safe_reply(format!("Resumed **{}**.", current.title)))
        .await?;

    Ok(())
}
