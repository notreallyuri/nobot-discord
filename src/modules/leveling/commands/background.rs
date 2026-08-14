use crate::{Context, card, error::AppError, modules::leveling::store};
use poise::serenity_prelude as serenity;

/// Set or clear your profile card background
#[poise::command(slash_command, guild_only, subcommands("set", "clear"))]
pub async fn background(_: Context<'_>) -> Result<(), AppError> {
    Ok(())
}

/// Upload an image to use behind your profile card
#[poise::command(slash_command)]
pub async fn set(
    ctx: Context<'_>,
    #[description = "An image to use as your profile background"] image: serenity::Attachment,
) -> Result<(), AppError> {
    ctx.defer_ephemeral().await?;

    let prepared = card::background::prepare(&image).await?;
    let size = prepared.sharp.len() + prepared.blurred.len();

    store::set_background(
        &ctx.data().db,
        ctx.author().id.get() as i64,
        &prepared.sharp,
        &prepared.blurred,
    )
    .await?;

    tracing::info!(user = %ctx.author().id, bytes = size, "profile background set");

    ctx.send(
        poise::CreateReply::default()
            .content("Background updated — run `/profile` to see it.")
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Remove your profile card background
#[poise::command(slash_command)]
pub async fn clear(ctx: Context<'_>) -> Result<(), AppError> {
    let had_one = store::clear_background(&ctx.data().db, ctx.author().id.get() as i64).await?;

    let message = if had_one {
        "Background removed."
    } else {
        "You don't have a background set."
    };

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
