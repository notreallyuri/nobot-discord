use crate::{
    config::Config,
    guild_config,
    module::Module,
    modules::voice::{idle::Sessions, repeat, spotify::SpotifyClient},
};
use dashmap::DashMap;
use sqlx::PgPool;
use std::{sync::Arc, time::Instant};

#[derive(Clone)]
pub struct Data {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub xp_cooldown: Arc<DashMap<MemberId, Instant>>,
    pub modules: Arc<Vec<Box<dyn Module>>>,
    pub spotify: Option<Arc<SpotifyClient>>,
    pub voice_sessions: Sessions,
    pub guild_config: guild_config::Cache,
    pub repeat: repeat::Modes,
}

impl Data {
    pub async fn guild_config(&self, guild_id: i64) -> guild_config::GuildConfig {
        guild_config::get(&self.db, &self.guild_config, guild_id).await
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemberId {
    pub guild_id: i64,
    pub user_id: i64,
}
