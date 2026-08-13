use crate::{
    data::Data,
    guild_config,
    module::{EventFuture, Module},
};
use poise::serenity_prelude as serenity;

pub mod announce;
pub mod commands;
pub mod idle;
pub mod lyrics;
pub mod repeat;
pub mod setup;
pub mod sources;
pub mod spotify;
pub mod suggest;
pub mod ytdlp;

pub struct VoiceModule;

impl Module for VoiceModule {
    fn name(&self) -> &'static str {
        "Voice"
    }

    fn commands(&self) -> Vec<crate::Command> {
        vec![
            commands::play(),
            commands::pause(),
            commands::resume(),
            commands::skip(),
            commands::shuffle(),
            commands::repeat(),
            commands::stop(),
            commands::queue(),
            commands::nowplaying(),
            commands::lyrics(),
            commands::clear(),
            commands::remove(),
            commands::move_track(),
            commands::leave(),
        ]
    }

    fn setup(&self, ctx: serenity::Context, data: Data) {
        idle::watch(ctx, data);
    }

    fn handle_event<'a>(
        &'a self,
        ctx: &'a serenity::Context,
        event: &'a serenity::FullEvent,
        data: &'a Data,
    ) -> EventFuture<'a> {
        Box::pin(async move {
            if matches!(event, serenity::FullEvent::CacheReady { .. }) {
                resume_all(ctx, data).await;
            }
            Ok(())
        })
    }
}

async fn resume_all(ctx: &serenity::Context, data: &Data) {
    let sessions = match guild_config::resumable(&data.db).await {
        Ok(sessions) => sessions,
        Err(e) => {
            tracing::warn!(?e, "couldn't load 24/7 sessions to resume");
            return;
        }
    };

    if sessions.is_empty() {
        return;
    }

    let Some(manager) = songbird::get(ctx).await else {
        tracing::warn!("voice subsystem unavailable, cannot resume 24/7 sessions");
        return;
    };

    for session in sessions {
        let guild_id = serenity::GuildId::new(session.guild_id as u64);
        let channel = serenity::ChannelId::new(session.voice_channel_id as u64);

        if manager
            .get(guild_id)
            .is_some_and(|call| call.try_lock().is_ok_and(|c| c.current_channel().is_some()))
        {
            continue;
        }

        match manager.join(guild_id, channel).await {
            Ok(call) => {
                if let Err(e) = call.lock().await.deafen(true).await {
                    tracing::warn!(?e, %guild_id, "failed to self-deafen after resuming");
                }

                if let Some(text) = session.voice_text_channel_id {
                    idle::remember(
                        &data.voice_sessions,
                        guild_id,
                        serenity::ChannelId::new(text as u64),
                    );
                }

                tracing::info!(%guild_id, %channel, "resumed 24/7 voice connection");
            }
            Err(e) => {
                tracing::warn!(?e, %guild_id, %channel, "couldn't resume 24/7 connection");
            }
        }
    }
}
