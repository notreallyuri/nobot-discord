use crate::{
    Context,
    error::AppError,
    modules::leveling::{badges, store},
};
use poise::serenity_prelude as serenity;

fn guild_of(ctx: Context<'_>) -> Result<i64, AppError> {
    ctx.guild_id()
        .map(|id| id.get() as i64)
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))
}

/// Browse and buy badges
#[poise::command(slash_command, guild_only, subcommands("list", "buy"))]
pub async fn shop(_: Context<'_>) -> Result<(), AppError> {
    Ok(())
}

/// Browse badges you can buy
#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), AppError> {
    let user_id = ctx.author().id.get() as i64;
    let db = &ctx.data().db;

    let settings = ctx.data().guild_config(guild_of(ctx)?).await;
    let currency = settings.currency();

    let balance = store::balance(db, user_id).await?;
    let owned: Vec<String> = store::owned_badges(db, user_id)
        .await?
        .into_iter()
        .map(|badge| badge.badge_id)
        .collect();

    let lines: Vec<String> = badges::purchasable()
        .map(|badge| {
            let price = badge.price.expect("purchasable");
            let status = if owned.iter().any(|id| id == badge.id) {
                "owned".to_string()
            } else if balance >= price {
                format!("{price} {currency}")
            } else {
                format!("{price} {currency} — need {} more", price - balance)
            };

            format!("**{}** · {}\n{}", badge.name, status, badge.description)
        })
        .collect();

    let embed = serenity::CreateEmbed::new()
        .title("Badge shop")
        .description(lines.join("\n\n"))
        .footer(serenity::CreateEmbedFooter::new(format!(
            "You have {balance} {currency} · buy with /shop buy"
        )));

    ctx.send(poise::CreateReply::default().embed(embed)).await?;
    Ok(())
}

async fn purchasable_names<'a>(
    _ctx: Context<'a>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    badges::purchasable()
        .map(|badge| badge.name.to_string())
        .filter(move |name| name.to_lowercase().contains(&partial.to_lowercase()))
}

/// Buy a badge with your coins
#[poise::command(slash_command)]
pub async fn buy(
    ctx: Context<'_>,
    #[description = "Which badge to buy"]
    #[autocomplete = "purchasable_names"]
    badge: String,
) -> Result<(), AppError> {
    let Some(badge) = badges::resolve(&badge) else {
        return Err(AppError::Message(format!(
            "There's no badge called `{}`. Try /shop list.",
            badge.chars().take(30).collect::<String>()
        )));
    };

    let Some(price) = badge.price else {
        return Err(AppError::Message(format!(
            "**{}** can't be bought — it's earned through achievements.",
            badge.name
        )));
    };

    let settings = ctx.data().guild_config(guild_of(ctx)?).await;
    let currency = settings.currency();

    let user_id = ctx.author().id.get() as i64;
    let outcome = store::buy_badge(&ctx.data().db, user_id, badge.id, price).await?;

    let message = match outcome {
        store::Purchase::Bought { balance } => format!(
            "Bought **{}** for {price} {currency}. {balance} left. Equip it with `/badges equip`.",
            badge.name
        ),
        store::Purchase::AlreadyOwned => format!("You already own **{}**.", badge.name),
        store::Purchase::TooPoor { balance, price } => format!(
            "**{}** costs {price} {currency} and you have {balance}.",
            badge.name
        ),
    };

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}
