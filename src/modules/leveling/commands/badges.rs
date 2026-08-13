use crate::{
    Context,
    error::AppError,
    modules::leveling::{badges as catalogue, store},
};
use poise::serenity_prelude as serenity;

#[poise::command(slash_command, guild_only, subcommands("list", "equip", "unequip"))]
pub async fn badges(_: Context<'_>) -> Result<(), AppError> {
    Ok(())
}

#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), AppError> {
    let user_id = ctx.author().id.get() as i64;
    let owned = store::owned_badges(&ctx.data().db, user_id).await?;

    if owned.is_empty() {
        ctx.send(
            poise::CreateReply::default()
                .content("You don't own any badges yet — see `/shop` or `/achievements`.")
                .ephemeral(true),
        )
        .await?;
        return Ok(());
    }

    let equipped = owned.iter().filter(|badge| badge.equipped).count();

    let lines: Vec<String> = owned
        .iter()
        .filter_map(|owned| {
            let badge = catalogue::find(&owned.badge_id)?;
            let mark = if owned.equipped {
                "▸ shown"
            } else {
                "  hidden"
            };
            Some(format!(
                "`{mark}` **{}** · {}",
                badge.name, badge.description
            ))
        })
        .collect();

    let embed = serenity::CreateEmbed::new()
        .title("Your badges")
        .description(lines.join("\n"))
        .footer(serenity::CreateEmbedFooter::new(format!(
            "{equipped} of {} slots used · /badges equip to change",
            catalogue::MAX_EQUIPPED
        )));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn owned_names<'a>(ctx: Context<'a>, partial: &'a str) -> Vec<String> {
    let user_id = ctx.author().id.get() as i64;
    let Ok(owned) = store::owned_badges(&ctx.data().db, user_id).await else {
        return Vec::new();
    };

    let partial = partial.to_lowercase();
    owned
        .iter()
        .filter_map(|owned| catalogue::find(&owned.badge_id))
        .map(|badge| badge.name.to_string())
        .filter(|name| name.to_lowercase().contains(&partial))
        .collect()
}

#[poise::command(slash_command)]
pub async fn equip(
    ctx: Context<'_>,
    #[description = "Which badge to show"]
    #[autocomplete = "owned_names"]
    badge: String,
) -> Result<(), AppError> {
    change(ctx, &badge, true).await
}

#[poise::command(slash_command)]
pub async fn unequip(
    ctx: Context<'_>,
    #[description = "Which badge to hide"]
    #[autocomplete = "owned_names"]
    badge: String,
) -> Result<(), AppError> {
    change(ctx, &badge, false).await
}

async fn change(ctx: Context<'_>, input: &str, equipped: bool) -> Result<(), AppError> {
    let Some(badge) = catalogue::resolve(input) else {
        return Err(AppError::Message(format!(
            "There's no badge called `{}`.",
            input.chars().take(30).collect::<String>()
        )));
    };

    let outcome = store::set_equipped(
        &ctx.data().db,
        ctx.author().id.get() as i64,
        badge.id,
        equipped,
        catalogue::MAX_EQUIPPED,
    )
    .await?;

    let verb = if equipped { "shown" } else { "hidden" };
    let message = match outcome {
        store::Equip::Changed => format!("**{}** is now {verb} on your profile.", badge.name),
        store::Equip::NoChange => format!("**{}** is already {verb}.", badge.name),
        store::Equip::NotOwned => format!("You don't own **{}**.", badge.name),
        store::Equip::TooMany { limit } => format!(
            "You can only show {limit} badges at once — hide one first with `/badges unequip`."
        ),
    };

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;

    Ok(())
}
