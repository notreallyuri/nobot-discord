use crate::error::AppError;
use poise::serenity_prelude as serenity;
use std::fmt;

const DEFAULT_PLAYLIST_LIMIT: usize = 100;

#[derive(Clone)]
pub struct SpotifyCredentials {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Clone)]
pub struct Config {
    pub token: String,
    pub database_url: String,
    pub guild_ids: Vec<serenity::GuildId>,
    pub spotify: Option<SpotifyCredentials>,
    pub playlist_limit: usize,
    pub member_intent: bool,
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

fn without_password(raw: &str) -> String {
    let Ok(mut url) = url::Url::parse(raw) else {
        return "<unparseable>".to_string();
    };

    if url.password().is_some() && url.set_password(Some("redacted")).is_err() {
        return "<unparseable>".to_string();
    }

    url.to_string()
}

impl fmt::Debug for SpotifyCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpotifyCredentials")
            .field("client_id", &self.client_id)
            .field("client_secret", &Redacted)
            .finish()
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("token", &Redacted)
            .field("database_url", &without_password(&self.database_url))
            .field("guild_ids", &self.guild_ids)
            .field("spotify", &self.spotify)
            .field("playlist_limit", &self.playlist_limit)
            .field("member_intent", &self.member_intent)
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        let spotify = match (
            optional("SPOTIFY_CLIENT_ID"),
            optional("SPOTIFY_CLIENT_SECRET"),
        ) {
            (Some(client_id), Some(client_secret)) => Some(SpotifyCredentials {
                client_id,
                client_secret,
            }),
            (None, None) => None,
            _ => {
                return Err(AppError::Message(
                    "SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET must be set together.".into(),
                ));
            }
        };

        Ok(Self {
            token: required("TOKEN")?,
            database_url: required("DATABASE_URL")?,
            guild_ids: parse_guild_ids()?,
            spotify,
            playlist_limit: parse_limit()?,
            member_intent: flag("MEMBER_INTENT"),
        })
    }
}

fn flag(key: &str) -> bool {
    optional(key).is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn required(key: &str) -> Result<String, AppError> {
    optional(key).ok_or_else(|| AppError::Message(format!("missing required env var {key}")))
}

fn optional(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn parse_guild_ids() -> Result<Vec<serenity::GuildId>, AppError> {
    let (key, raw) = match optional("GUILD_IDS") {
        Some(raw) => ("GUILD_IDS", raw),
        None => match optional("GUILD_ID") {
            Some(raw) => ("GUILD_ID", raw),
            None => return Ok(Vec::new()),
        },
    };

    parse_id_list(key, &raw)
}

fn parse_id_list(key: &str, raw: &str) -> Result<Vec<serenity::GuildId>, AppError> {
    let mut ids = Vec::new();

    for piece in raw.split([',', ' ', '\t']).filter(|p| !p.is_empty()) {
        let id: u64 = piece
            .parse()
            .map_err(|_| AppError::Message(format!("{key} must be numeric ids, got `{piece}`")))?;

        if id == 0 {
            return Err(AppError::Message(format!("{key} must not contain zero.")));
        }

        let id = serenity::GuildId::new(id);

        if !ids.contains(&id) {
            ids.push(id);
        }
    }

    Ok(ids)
}

fn parse_limit() -> Result<usize, AppError> {
    let Some(raw) = optional("PLAYLIST_LIMIT") else {
        return Ok(DEFAULT_PLAYLIST_LIMIT);
    };

    let limit: usize = raw
        .parse()
        .map_err(|_| AppError::Message(format!("PLAYLIST_LIMIT must be a number, got `{raw}`")))?;

    if limit == 0 {
        return Err(AppError::Message(
            "PLAYLIST_LIMIT must be at least 1.".into(),
        ));
    }

    Ok(limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(raw: &str) -> Vec<u64> {
        parse_id_list("GUILD_IDS", raw)
            .expect("should parse")
            .iter()
            .map(|id| id.get())
            .collect()
    }

    #[test]
    fn parses_one_or_many() {
        assert_eq!(ids("123"), [123]);
        assert_eq!(ids("123,456,789"), [123, 456, 789]);
        assert_eq!(ids(" 123 , 456 "), [123, 456]);
        assert_eq!(ids("123 456"), [123, 456]);
        assert_eq!(ids("123,456,"), [123, 456]);
    }

    #[test]
    fn drops_duplicates() {
        assert_eq!(ids("123,456,123"), [123, 456]);
    }

    #[test]
    fn empty_means_global() {
        assert!(ids("").is_empty());
        assert!(ids(" , ").is_empty());
    }

    #[test]
    fn rejects_nonsense() {
        for raw in ["abc", "123,abc", "0", "123,0", "-5", "12.5"] {
            assert!(
                parse_id_list("GUILD_IDS", raw).is_err(),
                "should have rejected: {raw}"
            );
        }
    }

    fn sample() -> Config {
        Config {
            token: "MTIzNDU2Nzg5.GaBcDe.super-secret-bot-token".to_string(),
            database_url: "postgresql://postgres:hunter2@db.internal:5436/dis_ru".to_string(),
            guild_ids: vec![serenity::GuildId::new(123)],
            spotify: Some(SpotifyCredentials {
                client_id: "a_public_client_id".to_string(),
                client_secret: "a_very_secret_value".to_string(),
            }),
            playlist_limit: 100,
            member_intent: false,
        }
    }

    #[test]
    fn debug_never_prints_a_secret() {
        let rendered = format!("{:?}", sample());

        for secret in [
            "MTIzNDU2Nzg5",
            "super-secret-bot-token",
            "hunter2",
            "a_very_secret_value",
        ] {
            assert!(
                !rendered.contains(secret),
                "leaked {secret:?} in {rendered}"
            );
        }
    }

    #[test]
    fn debug_is_still_worth_reading() {
        let rendered = format!("{:?}", sample());

        for useful in ["db.internal:5436", "dis_ru", "a_public_client_id", "123"] {
            assert!(
                rendered.contains(useful),
                "missing {useful:?} in {rendered}"
            );
        }
    }

    #[test]
    fn urls_without_a_password_are_left_alone() {
        assert_eq!(
            without_password("postgresql://postgres@localhost/dis_ru"),
            "postgresql://postgres@localhost/dis_ru"
        );
        assert_eq!(without_password("not a url"), "<unparseable>");
    }
}
