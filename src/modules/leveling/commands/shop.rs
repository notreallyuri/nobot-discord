use crate::{
    Context,
    error::AppError,
    modules::leveling::{badges, boosters, store, storefront},
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

/// Browse badges and boosters you can buy
#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), AppError> {
    let user_id = ctx.author().id.get() as i64;
    let settings = ctx.data().guild_config(guild_of(ctx)?).await;
    let wallet = storefront::wallet(&ctx.data().db, user_id).await?;

    let aisle = storefront::Aisle::Badges;
    let shelf = storefront::shelf(aisle, 0).await?;

    ctx.send(
        poise::CreateReply::default()
            .ephemeral(true)
            .embed(storefront::embed(aisle, 0, &wallet, settings.currency()))
            .components(storefront::components(aisle, 0))
            .attachment(serenity::CreateAttachment::bytes(shelf, storefront::IMAGE)),
    )
    .await?;

    Ok(())
}

enum Item {
    Badge(&'static badges::Badge),
    Booster(&'static boosters::Booster),
}

impl Item {
    fn resolve(input: &str) -> Option<Self> {
        badges::resolve(input)
            .map(Item::Badge)
            .or_else(|| boosters::resolve(input).map(Item::Booster))
    }

    fn name(&self) -> &'static str {
        match self {
            Item::Badge(badge) => badge.name,
            Item::Booster(booster) => booster.name,
        }
    }
}

async fn purchasable_names<'a>(
    _ctx: Context<'a>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let names = badges::purchasable()
        .map(|badge| badge.name.to_string())
        .chain(boosters::catalogue().map(|booster| booster.name.to_string()));

    names.filter(move |name| name.to_lowercase().contains(&partial.to_lowercase()))
}

/// Buy a badge or an XP booster with your coins
#[poise::command(slash_command)]
pub async fn buy(
    ctx: Context<'_>,
    #[description = "Which badge or booster to buy"]
    #[autocomplete = "purchasable_names"]
    item: String,
) -> Result<(), AppError> {
    let Some(found) = Item::resolve(&item) else {
        return Err(AppError::Message(format!(
            "There's nothing called `{}` in the shop. Try /shop list.",
            item.chars().take(30).collect::<String>()
        )));
    };

    let settings = ctx.data().guild_config(guild_of(ctx)?).await;
    let currency = settings.currency();
    let user_id = ctx.author().id.get() as i64;
    let db = &ctx.data().db;

    let (outcome, price, tail) = match found {
        Item::Badge(badge) => {
            let Some(price) = badge.price else {
                return Err(AppError::Message(format!(
                    "**{}** can't be bought — it's earned through achievements.",
                    badge.name
                )));
            };

            (
                store::buy_badge(db, user_id, badge.id, price).await?,
                price,
                "Equip it with `/badges equip`.".to_string(),
            )
        }
        Item::Booster(booster) => (
            store::buy_booster(
                db,
                user_id,
                booster.multiplier_pct,
                booster.duration().as_secs() as i64,
                booster.price,
            )
            .await?,
            booster.price,
            format!(
                "{} for the next {}.",
                booster.label(),
                spell_hours(booster.hours)
            ),
        ),
    };

    let name = found.name();
    let message = match outcome {
        store::Purchase::Bought { balance } => {
            format!("Bought **{name}** for {price} {currency}. {balance} left. {tail}")
        }
        store::Purchase::AlreadyOwned => format!("You already own **{name}**."),
        store::Purchase::TooPoor { balance, price } => {
            format!("**{name}** costs {price} {currency} and you have {balance}.")
        }
    };

    ctx.send(
        poise::CreateReply::default()
            .content(message)
            .allowed_mentions(serenity::CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}

fn spell_hours(hours: i64) -> String {
    match hours {
        1 => "hour".to_string(),
        24 => "day".to_string(),
        h if h % 24 == 0 => format!("{} days", h / 24),
        h => format!("{h} hours"),
    }
}
