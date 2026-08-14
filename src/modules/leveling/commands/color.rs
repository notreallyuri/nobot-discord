use crate::{Context, card::accent, error::AppError, modules::leveling::store};
use poise::serenity_prelude as serenity;

/// Set or clear your profile accent colour
#[poise::command(
    slash_command,
    guild_only,
    rename = "color",
    subcommands("set", "clear")
)]
pub async fn color(_: Context<'_>) -> Result<(), AppError> {
    Ok(())
}

/// Pick the accent colour used on your profile card
#[poise::command(slash_command)]
pub async fn set(
    ctx: Context<'_>,
    #[description = "A hex colour, e.g. #7c5cff"] hex: String,
) -> Result<(), AppError> {
    let colour = accent::parse(&hex)?;
    let resolved = accent::Accent::new(colour);

    store::set_accent(
        &ctx.data().db,
        ctx.author().id.get() as i64,
        colour.to_i32(),
    )
    .await?;

    let mut message = format!("Accent set to `{}`.", resolved.base);
    if resolved.adjusted {
        message.push_str(
            " That colour was too dark to read on the card, so it's been \
             lightened slightly.",
        );
    }
    message.push_str(" Run `/profile` to see it.");

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

/// Go back to the default accent colour
#[poise::command(slash_command)]
pub async fn clear(ctx: Context<'_>) -> Result<(), AppError> {
    let had_one = store::clear_accent(&ctx.data().db, ctx.author().id.get() as i64).await?;

    let message = if had_one {
        "Accent reset to the default."
    } else {
        "You're already using the default accent."
    };

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
