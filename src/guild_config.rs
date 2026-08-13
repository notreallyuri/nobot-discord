use crate::{error::AppError, modules::leveling::setup::xp};
use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};

pub const DEFAULT_CURRENCY: &str = "coins";
pub const MAX_LABEL_LEN: usize = 32;
pub const MAX_XP_PER_MESSAGE: i64 = 10_000;
pub const MAX_COOLDOWN_SECS: i64 = 86_400;

pub type Cache = Arc<DashMap<i64, GuildConfig>>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildConfig {
    pub economy_enabled: Option<bool>,
    pub currency_name: Option<String>,
    pub currency_emoji: Option<String>,
    pub xp_per_message: Option<i64>,
    pub xp_cooldown_secs: Option<i32>,
    pub dj_role_id: Option<i64>,
}

impl GuildConfig {
    pub fn economy(&self) -> bool {
        self.economy_enabled.unwrap_or(true)
    }

    pub fn currency(&self) -> &str {
        self.currency_name.as_deref().unwrap_or(DEFAULT_CURRENCY)
    }

    pub fn emoji(&self) -> Option<&str> {
        self.currency_emoji.as_deref()
    }

    pub fn xp_award(&self) -> i64 {
        self.xp_per_message.unwrap_or(xp::XP_PER_MESSAGE)
    }

    pub fn xp_cooldown(&self) -> Duration {
        self.xp_cooldown_secs.map_or(xp::XP_COOLDOWN, |secs| {
            Duration::from_secs(secs.max(0) as u64)
        })
    }

    pub fn dj_role(&self) -> Option<serenity::RoleId> {
        self.dj_role_id
            .filter(|id| *id > 0)
            .map(|id| serenity::RoleId::new(id as u64))
    }

    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

pub enum Setting {
    Economy(Option<bool>),
    CurrencyName(Option<String>),
    CurrencyEmoji(Option<String>),
    XpPerMessage(Option<i64>),
    XpCooldown(Option<i32>),
    DjRole(Option<i64>),
}

impl Setting {
    pub fn validate(&self) -> Result<(), AppError> {
        let label = |value: &Option<String>, what: &str| match value {
            Some(text) if text.chars().count() > MAX_LABEL_LEN || text.trim().is_empty() => {
                Err(AppError::Message(format!(
                    "A {what} must be between 1 and {MAX_LABEL_LEN} characters."
                )))
            }
            _ => Ok(()),
        };

        match self {
            Self::CurrencyName(value) => label(value, "currency name"),
            Self::CurrencyEmoji(value) => label(value, "currency emoji"),
            Self::XpPerMessage(Some(amount)) if !(1..=MAX_XP_PER_MESSAGE).contains(amount) => {
                Err(AppError::Message(format!(
                    "XP per message must be between 1 and {MAX_XP_PER_MESSAGE}."
                )))
            }
            Self::XpCooldown(Some(secs))
                if !(0..=MAX_COOLDOWN_SECS).contains(&i64::from(*secs)) =>
            {
                Err(AppError::Message(format!(
                    "The XP cooldown must be between 0 and {MAX_COOLDOWN_SECS} seconds."
                )))
            }
            _ => Ok(()),
        }
    }
}

async fn fetch(db: &PgPool, guild_id: i64) -> Result<GuildConfig, AppError> {
    let row = sqlx::query_as!(
        GuildConfig,
        "SELECT economy_enabled, currency_name, currency_emoji, xp_per_message,
                xp_cooldown_secs, dj_role_id
         FROM guild_config WHERE guild_id = $1",
        guild_id
    )
    .fetch_optional(db)
    .await?;

    Ok(row.unwrap_or_default())
}

pub async fn get(db: &PgPool, cache: &Cache, guild_id: i64) -> GuildConfig {
    if let Some(cached) = cache.get(&guild_id) {
        return cached.clone();
    }

    let config = fetch(db, guild_id).await.unwrap_or_else(|e| {
        tracing::warn!(?e, guild_id, "failed to load guild config, using defaults");
        GuildConfig::default()
    });

    cache.insert(guild_id, config.clone());
    config
}

pub async fn apply(
    db: &PgPool,
    cache: &Cache,
    guild_id: i64,
    setting: Setting,
) -> Result<GuildConfig, AppError> {
    setting.validate()?;

    match setting {
        Setting::Economy(value) => {
            sqlx::query!(
                "INSERT INTO guild_config (guild_id, economy_enabled) VALUES ($1, $2)
                 ON CONFLICT (guild_id) DO UPDATE
                 SET economy_enabled = $2, updated_at = now()",
                guild_id,
                value,
            )
            .execute(db)
            .await?;
        }
        Setting::CurrencyName(value) => {
            sqlx::query!(
                "INSERT INTO guild_config (guild_id, currency_name) VALUES ($1, $2)
                 ON CONFLICT (guild_id) DO UPDATE
                 SET currency_name = $2, updated_at = now()",
                guild_id,
                value,
            )
            .execute(db)
            .await?;
        }
        Setting::CurrencyEmoji(value) => {
            sqlx::query!(
                "INSERT INTO guild_config (guild_id, currency_emoji) VALUES ($1, $2)
                 ON CONFLICT (guild_id) DO UPDATE
                 SET currency_emoji = $2, updated_at = now()",
                guild_id,
                value,
            )
            .execute(db)
            .await?;
        }
        Setting::XpPerMessage(value) => {
            sqlx::query!(
                "INSERT INTO guild_config (guild_id, xp_per_message) VALUES ($1, $2)
                 ON CONFLICT (guild_id) DO UPDATE
                 SET xp_per_message = $2, updated_at = now()",
                guild_id,
                value,
            )
            .execute(db)
            .await?;
        }
        Setting::DjRole(value) => {
            sqlx::query!(
                "INSERT INTO guild_config (guild_id, dj_role_id) VALUES ($1, $2)
                 ON CONFLICT (guild_id) DO UPDATE
                 SET dj_role_id = $2, updated_at = now()",
                guild_id,
                value,
            )
            .execute(db)
            .await?;
        }
        Setting::XpCooldown(value) => {
            sqlx::query!(
                "INSERT INTO guild_config (guild_id, xp_cooldown_secs) VALUES ($1, $2)
                 ON CONFLICT (guild_id) DO UPDATE
                 SET xp_cooldown_secs = $2, updated_at = now()",
                guild_id,
                value,
            )
            .execute(db)
            .await?;
        }
    }

    let refreshed = fetch(db, guild_id).await?;
    cache.insert(guild_id, refreshed.clone());

    Ok(refreshed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_untouched_guild_gets_the_bot_defaults() {
        let config = GuildConfig::default();

        assert!(config.is_default());
        assert!(config.economy());
        assert_eq!(config.currency(), DEFAULT_CURRENCY);
        assert_eq!(config.emoji(), None);
        assert_eq!(config.xp_award(), xp::XP_PER_MESSAGE);
        assert_eq!(config.xp_cooldown(), xp::XP_COOLDOWN);
        assert_eq!(config.dj_role(), None);
    }

    #[test]
    fn overrides_win_over_defaults() {
        let config = GuildConfig {
            economy_enabled: Some(false),
            currency_name: Some("gems".to_string()),
            currency_emoji: Some("💎".to_string()),
            xp_per_message: Some(5),
            xp_cooldown_secs: Some(120),
            dj_role_id: Some(4242),
        };

        assert!(!config.is_default());
        assert!(!config.economy());
        assert_eq!(config.currency(), "gems");
        assert_eq!(config.emoji(), Some("💎"));
        assert_eq!(config.xp_award(), 5);
        assert_eq!(config.xp_cooldown(), Duration::from_secs(120));
        assert_eq!(config.dj_role().map(|r| r.get()), Some(4242));
    }

    #[test]
    fn a_zero_cooldown_is_honoured_rather_than_falling_back() {
        let config = GuildConfig {
            xp_cooldown_secs: Some(0),
            ..Default::default()
        };

        assert_eq!(config.xp_cooldown(), Duration::ZERO);
    }

    #[test]
    fn labels_must_be_short_and_non_empty() {
        let too_long = "x".repeat(MAX_LABEL_LEN + 1);

        for setting in [
            Setting::CurrencyName(Some(too_long.clone())),
            Setting::CurrencyName(Some("   ".to_string())),
            Setting::CurrencyEmoji(Some(too_long)),
            Setting::CurrencyEmoji(Some(String::new())),
        ] {
            assert!(setting.validate().is_err());
        }

        assert!(
            Setting::CurrencyName(Some("gems".into()))
                .validate()
                .is_ok()
        );
        assert!(Setting::CurrencyName(None).validate().is_ok());
        assert!(
            Setting::CurrencyEmoji(Some("💎".repeat(MAX_LABEL_LEN)))
                .validate()
                .is_ok(),
            "the limit counts characters, not bytes"
        );
    }

    #[test]
    fn numeric_settings_are_bounded() {
        assert!(Setting::XpPerMessage(Some(0)).validate().is_err());
        assert!(Setting::XpPerMessage(Some(-5)).validate().is_err());
        assert!(
            Setting::XpPerMessage(Some(MAX_XP_PER_MESSAGE + 1))
                .validate()
                .is_err()
        );
        assert!(Setting::XpPerMessage(Some(75)).validate().is_ok());
        assert!(Setting::XpPerMessage(None).validate().is_ok());

        assert!(Setting::XpCooldown(Some(-1)).validate().is_err());
        assert!(Setting::XpCooldown(Some(0)).validate().is_ok());
        assert!(Setting::XpCooldown(Some(30)).validate().is_ok());
    }
}
