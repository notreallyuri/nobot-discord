use crate::{Context, error::AppError, modules::voice::setup};
use poise::serenity_prelude as serenity;

/// Show the current track and how far through it is
#[poise::command(slash_command, guild_only, rename = "nowplaying")]
pub async fn nowplaying(ctx: Context<'_>) -> Result<(), AppError> {
    let call = setup::current_call(ctx).await?;
    let current = {
        let call = call.lock().await;
        call.queue().current()
    };

    let Some(current) = current else {
        return Err(AppError::Message("Nothing is playing.".into()));
    };

    let meta = current.data::<setup::TrackMeta>();
    let position = current
        .get_info()
        .await
        .map(|state| state.position)
        .unwrap_or_default();

    let mut embed = serenity::CreateEmbed::new()
        .title("Now playing")
        .description(match &meta.url {
            Some(url) => format!("[{}]({})", meta.title, url),
            None => meta.title.clone(),
        })
        .field(
            "Progress",
            setup::progress_bar(position, meta.duration),
            false,
        )
        .field("Requested by", format!("<@{}>", meta.requester), true);

    if let Some(thumb) = &meta.thumbnail {
        embed = embed.thumbnail(thumb);
    }

    ctx.send(
        poise::CreateReply::default()
            .embed(embed)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}
