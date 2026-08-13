use crate::{
    Context,
    error::AppError,
    guild_config::{self, GuildConfig, Setting},
};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    guild_only,
    subcommands("show", "economy", "currency", "xp", "dj", "reset"),
    default_member_permissions = "MANAGE_GUILD",
    required_permissions = "MANAGE_GUILD"
)]
pub async fn config(_: Context<'_>) -> Result<(), AppError> {
    Ok(())
}

fn guild_of(ctx: Context<'_>) -> Result<i64, AppError> {
    ctx.guild_id()
        .map(|id| id.get() as i64)
        .ok_or_else(|| AppError::Message("This command can only be used in a server.".into()))
}

fn describe(config: &GuildConfig) -> serenity::CreateEmbed {
    let shown = |value: Option<String>| value.unwrap_or_else(|| "default".to_string());

    let currency = match config.emoji() {
        Some(emoji) => format!("{emoji} {}", config.currency()),
        None => config.currency().to_string(),
    };

    serenity::CreateEmbed::new()
        .title("Server settings")
        .description(if config.is_default() {
            "Everything here is on the bot default. Change one with `/config`."
        } else {
            "Anything left as `default` follows the bot default."
        })
        .field(
            "Economy",
            if config.economy() {
                "enabled"
            } else {
                "disabled"
            },
            true,
        )
        .field("Currency", currency, true)
        .field(
            "XP per message",
            shown(config.xp_per_message.map(|xp| xp.to_string())),
            true,
        )
        .field(
            "DJ role",
            config.dj_role().map_or_else(
                || "anyone can control playback".to_string(),
                |role| format!("<@&{role}>"),
            ),
            true,
        )
        .field(
            "XP cooldown",
            shown(
                config
                    .xp_cooldown_secs
                    .map(|secs| format!("{secs} second(s)")),
            ),
            true,
        )
}

async fn save(ctx: Context<'_>, setting: Setting, note: &str) -> Result<(), AppError> {
    let guild_id = guild_of(ctx)?;
    let data = ctx.data();
    let updated = guild_config::apply(&data.db, &data.guild_config, guild_id, setting).await?;

    tracing::info!(guild_id, note, "guild config changed");

    ctx.send(
        poise::CreateReply::default()
            .content(note.to_string())
            .embed(describe(&updated))
            .allowed_mentions(serenity::CreateAllowedMentions::new())
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn show(ctx: Context<'_>) -> Result<(), AppError> {
    let config = ctx.data().guild_config(guild_of(ctx)?).await;

    ctx.send(
        poise::CreateReply::default()
            .embed(describe(&config))
            .ephemeral(true),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn economy(
    ctx: Context<'_>,
    #[description = "Whether messages earn XP and coins here"] enabled: bool,
) -> Result<(), AppError> {
    let note = if enabled {
        "Economy enabled — messages earn XP and coins again."
    } else {
        "Economy disabled — messages no longer earn XP or coins. Existing balances are untouched."
    };

    save(ctx, Setting::Economy(Some(enabled)), note).await
}

#[poise::command(slash_command)]
pub async fn currency(
    ctx: Context<'_>,
    #[description = "What to call coins here, e.g. gems"] name: Option<String>,
    #[description = "An emoji to show beside the amount"] emoji: Option<String>,
) -> Result<(), AppError> {
    if name.is_none() && emoji.is_none() {
        return Err(AppError::Message(
            "Give a name, an emoji, or both. Use `/config reset` to go back to the default.".into(),
        ));
    }

    if let Some(name) = name {
        save(
            ctx,
            Setting::CurrencyName(Some(name.trim().to_string())),
            "Currency name updated.",
        )
        .await?;
    }

    if let Some(emoji) = emoji {
        save(
            ctx,
            Setting::CurrencyEmoji(Some(emoji.trim().to_string())),
            "Currency emoji updated.",
        )
        .await?;
    }

    Ok(())
}

#[poise::command(slash_command)]
pub async fn xp(
    ctx: Context<'_>,
    #[description = "XP awarded per message"]
    #[min = 1]
    #[max = 10000]
    per_message: Option<i64>,
    #[description = "Seconds between XP awards for the same member"]
    #[min = 0]
    #[max = 86400]
    cooldown: Option<i32>,
) -> Result<(), AppError> {
    if per_message.is_none() && cooldown.is_none() {
        return Err(AppError::Message(
            "Give an amount, a cooldown, or both. Use `/config reset` to go back to the default."
                .into(),
        ));
    }

    if let Some(amount) = per_message {
        save(
            ctx,
            Setting::XpPerMessage(Some(amount)),
            "XP per message updated.",
        )
        .await?;
    }

    if let Some(secs) = cooldown {
        save(ctx, Setting::XpCooldown(Some(secs)), "XP cooldown updated.").await?;
    }

    Ok(())
}

#[poise::command(slash_command)]
pub async fn dj(
    ctx: Context<'_>,
    #[description = "Role allowed to control playback"] role: serenity::Role,
) -> Result<(), AppError> {
    save(
        ctx,
        Setting::DjRole(Some(role.id.get() as i64)),
        &format!("Only **{}** can control playback now. Server managers and anyone listening alone are still allowed.", role.name),
    )
    .await
}

#[derive(Debug, poise::ChoiceParameter)]
pub enum Resettable {
    #[name = "economy"]
    Economy,
    #[name = "currency name"]
    CurrencyName,
    #[name = "currency emoji"]
    CurrencyEmoji,
    #[name = "xp per message"]
    XpPerMessage,
    #[name = "xp cooldown"]
    XpCooldown,
    #[name = "dj role"]
    DjRole,
}

#[poise::command(slash_command)]
pub async fn reset(
    ctx: Context<'_>,
    #[description = "Which setting to put back on the bot default"] setting: Resettable,
) -> Result<(), AppError> {
    let cleared = match setting {
        Resettable::Economy => Setting::Economy(None),
        Resettable::CurrencyName => Setting::CurrencyName(None),
        Resettable::CurrencyEmoji => Setting::CurrencyEmoji(None),
        Resettable::XpPerMessage => Setting::XpPerMessage(None),
        Resettable::XpCooldown => Setting::XpCooldown(None),
        Resettable::DjRole => Setting::DjRole(None),
    };

    save(ctx, cleared, "Back on the bot default.").await
}
