use crate::{
    Context,
    error::AppError,
    modules::leveling::{cosmetics as catalogue, store},
};
use poise::serenity_prelude as serenity;

/// View and wear the cosmetics you own
#[poise::command(slash_command, guild_only, subcommands("list", "equip", "unequip"))]
pub async fn cosmetics(_: Context<'_>) -> Result<(), AppError> {
    Ok(())
}

/// List the cosmetics you own
#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), AppError> {
    let user_id = ctx.author().id.get() as i64;
    let owned = store::owned_cosmetics(&ctx.data().db, user_id).await?;

    if owned.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .content("You don't own any cosmetics yet — see `/shop list`.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let lines: Vec<String> = owned
        .iter()
        .filter_map(|owned| {
            let cosmetic = catalogue::find(&owned.cosmetic_id)?;
            let mark = if owned.equipped {
                "▸ worn"
            } else {
                "  spare"
            };

            Some(format!(
                "`{mark}` **{}** · {} · {}",
                cosmetic.name,
                cosmetic.slot().label(),
                cosmetic.description
            ))
        })
        .collect();

    let embed = serenity::CreateEmbed::new()
        .title("Your cosmetics")
        .description(lines.join("\n"))
        .footer(serenity::CreateEmbedFooter::new(
            "One accent and one card effect at a time · /cosmetics equip to change",
        ));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn owned_names<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let user_id = ctx.author().id.get() as i64;
    let Ok(owned) = store::owned_cosmetics(&ctx.data().db, user_id).await else {
        return Vec::new();
    };

    let partial = partial.to_lowercase();
    owned
        .iter()
        .filter_map(|owned| catalogue::find(&owned.cosmetic_id))
        .map(|cosmetic| cosmetic.name.to_string())
        .filter(|name| name.to_lowercase().contains(&partial))
        .collect()
}

/// Wear a cosmetic on your profile card
#[poise::command(slash_command)]
pub async fn equip(
    ctx: Context<'_>,
    #[description = "Which cosmetic to wear"]
    #[autocomplete = "owned_names"]
    cosmetic: String,
) -> Result<(), AppError> {
    let found = resolve(&cosmetic)?;

    let outcome = store::equip_cosmetic(
        &ctx.data().db,
        ctx.author().id.get() as i64,
        found.slot(),
        found.id,
    )
    .await?;

    let slot = found.slot().label();
    let message = match outcome {
        store::Worn::Changed => format!(
            "**{}** is now your {slot}. Run `/profile` to see it.",
            found.name
        ),
        store::Worn::NoChange => format!("**{}** is already your {slot}.", found.name),
        store::Worn::NotOwned => format!(
            "You don't own **{}** — buy it with `/shop buy {}`.",
            found.name, found.name
        ),
    };

    reply(ctx, message).await
}

/// Take a cosmetic off your profile card
#[poise::command(slash_command)]
pub async fn unequip(
    ctx: Context<'_>,
    #[description = "Which cosmetic to take off"]
    #[autocomplete = "owned_names"]
    cosmetic: String,
) -> Result<(), AppError> {
    let found = resolve(&cosmetic)?;

    let outcome = store::unequip_cosmetic(
        &ctx.data().db,
        ctx.author().id.get() as i64,
        found.slot(),
        found.id,
    )
    .await?;

    let slot = found.slot().label();
    let message = match outcome {
        store::Worn::Changed => format!("Your {slot} is back to the default."),
        _ => format!("You aren't wearing **{}**.", found.name),
    };

    reply(ctx, message).await
}

fn resolve(input: &str) -> Result<&'static catalogue::Cosmetic, AppError> {
    catalogue::resolve(input).ok_or_else(|| {
        AppError::Message(format!(
            "There's no cosmetic called `{}`.",
            input.chars().take(30).collect::<String>()
        ))
    })
}

async fn reply(ctx: Context<'_>, message: String) -> Result<(), AppError> {
    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
