use crate::{
    Context,
    error::AppError,
    guild_config::{self, GuildConfig, Setting},
};
use poise::serenity_prelude as serenity;

#[poise::command(
    slash_command,
    guild_only,
    subcommands(
        "show", "economy", "currency", "xp", "dj", "voice", "welcome", "farewell", "reset"
    ),
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
            "24/7 mode",
            if config.stays_connected() {
                "on — stays in voice".to_string()
            } else {
                format!(
                    "off — leaves after {}s idle",
                    config.idle_timeout().as_secs()
                )
            },
            true,
        )
        .field(
            "Welcome",
            config.welcome_channel_id.map_or_else(
                || "off".to_string(),
                |id| {
                    format!(
                        "<#{id}>{}",
                        if config.shows_welcome_card() {
                            " + card"
                        } else {
                            ""
                        }
                    )
                },
            ),
            true,
        )
        .field(
            "Farewell",
            config
                .farewell_channel_id
                .map_or_else(|| "off".to_string(), |id| format!("<#{id}>")),
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

#[poise::command(slash_command)]
pub async fn voice(
    ctx: Context<'_>,
    #[description = "Stay in voice instead of leaving when idle or alone"] always_on: Option<bool>,
    #[description = "Seconds of silence before leaving (ignored when 24/7 is on)"]
    #[min = 10]
    #[max = 86400]
    idle_timeout: Option<i32>,
) -> Result<(), AppError> {
    if always_on.is_none() && idle_timeout.is_none() {
        return Err(AppError::Message(
            "Give `always_on`, `idle_timeout`, or both. Use `/config reset` for the default."
                .into(),
        ));
    }

    if let Some(on) = always_on {
        let note = if on {
            "24/7 mode on — I'll stay in voice and rejoin after a restart. Use `/leave` to send me away."
        } else {
            "24/7 mode off — I'll leave when the queue runs dry or everyone goes."
        };

        save(ctx, Setting::StayConnected(Some(on)), note).await?;

        if !on {
            save(
                ctx,
                Setting::VoiceChannels(None, None),
                "Forgot the remembered voice channel.",
            )
            .await?;
        }
    }

    if let Some(secs) = idle_timeout {
        save(
            ctx,
            Setting::IdleTimeout(Some(secs)),
            "Idle timeout updated.",
        )
        .await?;
    }

    Ok(())
}

const PLACEHOLDERS: &str = "Placeholders: `{user}`, `{mention}`, `{server}`, `{count}`.";

#[poise::command(slash_command)]
pub async fn welcome(
    ctx: Context<'_>,
    #[description = "Where to post welcomes"] channel: Option<serenity::GuildChannel>,
    #[description = "What to say"] message: Option<String>,
    #[description = "Attach a welcome card"] card: Option<bool>,
) -> Result<(), AppError> {
    if channel.is_none() && message.is_none() && card.is_none() {
        return Err(AppError::Message(format!(
            "Give a channel, a message, or a card toggle. {PLACEHOLDERS} \
             Turn welcomes off with `/config reset`."
        )));
    }

    if let Some(channel) = channel {
        save(
            ctx,
            Setting::WelcomeChannel(Some(channel.id.get() as i64)),
            &format!("Welcoming new members in {channel}."),
        )
        .await?;
    }

    if let Some(message) = message {
        save(
            ctx,
            Setting::WelcomeMessage(Some(message.trim().to_string())),
            &format!("Welcome message updated. {PLACEHOLDERS}"),
        )
        .await?;
    }

    if let Some(card) = card {
        save(
            ctx,
            Setting::WelcomeCard(Some(card)),
            if card {
                "Welcomes will include a card."
            } else {
                "Welcomes will be text only."
            },
        )
        .await?;
    }

    Ok(())
}

#[poise::command(slash_command)]
pub async fn farewell(
    ctx: Context<'_>,
    #[description = "Where to post farewells"] channel: Option<serenity::GuildChannel>,
    #[description = "What to say"] message: Option<String>,
) -> Result<(), AppError> {
    if channel.is_none() && message.is_none() {
        return Err(AppError::Message(format!(
            "Give a channel, a message, or both. {PLACEHOLDERS} \
             Turn farewells off with `/config reset`."
        )));
    }

    if let Some(channel) = channel {
        save(
            ctx,
            Setting::FarewellChannel(Some(channel.id.get() as i64)),
            &format!("Announcing departures in {channel}."),
        )
        .await?;
    }

    if let Some(message) = message {
        save(
            ctx,
            Setting::FarewellMessage(Some(message.trim().to_string())),
            &format!("Farewell message updated. {PLACEHOLDERS}"),
        )
        .await?;
    }

    Ok(())
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
    #[name = "24/7 mode"]
    StayConnected,
    #[name = "idle timeout"]
    IdleTimeout,
    #[name = "welcomes"]
    Welcome,
    #[name = "welcome message"]
    WelcomeMessage,
    #[name = "farewells"]
    Farewell,
    #[name = "farewell message"]
    FarewellMessage,
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
        Resettable::StayConnected => Setting::StayConnected(None),
        Resettable::IdleTimeout => Setting::IdleTimeout(None),
        Resettable::Welcome => Setting::WelcomeChannel(None),
        Resettable::WelcomeMessage => Setting::WelcomeMessage(None),
        Resettable::Farewell => Setting::FarewellChannel(None),
        Resettable::FarewellMessage => Setting::FarewellMessage(None),
    };

    save(ctx, cleared, "Back on the bot default.").await
}
