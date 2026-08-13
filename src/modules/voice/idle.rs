use crate::data::Data;
use dashmap::DashMap;
use poise::serenity_prelude as serenity;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

const EMPTY_GRACE: Duration = Duration::from_secs(30);
const IDLE_GRACE: Duration = Duration::from_secs(60);
const SWEEP_INTERVAL: Duration = Duration::from_secs(5);

pub type Sessions = Arc<DashMap<serenity::GuildId, VoiceSession>>;

pub struct VoiceSession {
    text_channel: serenity::ChannelId,
    empty_since: Option<Instant>,
    idle_since: Option<Instant>,
}

#[derive(Debug, Clone, Copy)]
enum LeaveReason {
    Empty,
    Idle,
}

impl LeaveReason {
    fn message(self) -> &'static str {
        match self {
            Self::Empty => "Left the voice channel — everyone else left.",
            Self::Idle => "Left the voice channel — nothing has been queued for a minute.",
        }
    }
}

impl VoiceSession {
    fn new(text_channel: serenity::ChannelId) -> Self {
        Self {
            text_channel,
            empty_since: None,
            idle_since: None,
        }
    }

    fn observe(&mut self, now: Instant, alone: bool, queue_empty: bool) -> Option<LeaveReason> {
        if alone {
            let since = *self.empty_since.get_or_insert(now);
            if now.duration_since(since) >= EMPTY_GRACE {
                return Some(LeaveReason::Empty);
            }
        } else {
            self.empty_since = None;
        }

        if queue_empty {
            let since = *self.idle_since.get_or_insert(now);
            if now.duration_since(since) >= IDLE_GRACE {
                return Some(LeaveReason::Idle);
            }
        } else {
            self.idle_since = None;
        }

        None
    }
}

pub fn remember(
    sessions: &Sessions,
    guild_id: serenity::GuildId,
    text_channel: serenity::ChannelId,
) {
    sessions
        .entry(guild_id)
        .and_modify(|session| session.text_channel = text_channel)
        .or_insert_with(|| VoiceSession::new(text_channel));
}

pub fn forget(sessions: &Sessions, guild_id: serenity::GuildId) {
    sessions.remove(&guild_id);
}

pub fn watch(ctx: serenity::Context, data: Data) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            tick.tick().await;
            sweep(&ctx, &data).await;
        }
    });
}

async fn sweep(ctx: &serenity::Context, data: &Data) {
    let Some(manager) = songbird::get(ctx).await else {
        return;
    };

    let guilds: Vec<_> = data.voice_sessions.iter().map(|e| *e.key()).collect();

    for guild_id in guilds {
        check(ctx, data, &manager, guild_id).await;
    }
}

async fn check(
    ctx: &serenity::Context,
    data: &Data,
    manager: &songbird::Songbird,
    guild_id: serenity::GuildId,
) {
    let Some(call) = manager.get(guild_id) else {
        forget(&data.voice_sessions, guild_id);
        return;
    };

    let (channel, queue_empty) = {
        let call = call.lock().await;
        (call.current_channel(), call.queue().is_empty())
    };

    let Some(channel) = channel else {
        forget(&data.voice_sessions, guild_id);
        return;
    };

    let Some(others) = others_present(ctx, guild_id, channel) else {
        return;
    };

    let outcome = {
        let Some(mut session) = data.voice_sessions.get_mut(&guild_id) else {
            return;
        };
        session
            .observe(Instant::now(), others == 0, queue_empty)
            .map(|reason| (reason, session.text_channel))
    };

    let Some((reason, text_channel)) = outcome else {
        return;
    };

    forget(&data.voice_sessions, guild_id);

    if let Err(e) = manager.remove(guild_id).await {
        tracing::warn!(?e, %guild_id, "failed to leave voice channel while idle");
        return;
    }

    tracing::info!(%guild_id, ?reason, "left voice channel");

    if let Err(e) = text_channel.say(&ctx.http, reason.message()).await {
        tracing::warn!(?e, %text_channel, "failed to announce idle disconnect");
    }
}

fn others_present(
    ctx: &serenity::Context,
    guild_id: serenity::GuildId,
    channel: songbird::id::ChannelId,
) -> Option<usize> {
    let guild = ctx.cache.guild(guild_id)?;
    let me = ctx.cache.current_user().id;

    Some(
        guild
            .voice_states
            .values()
            .filter(|state| state.user_id != me)
            .filter(|state| state.channel_id.is_some_and(|c| c.get() == channel.0.get()))
            .count(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> VoiceSession {
        VoiceSession::new(serenity::ChannelId::new(1))
    }

    #[test]
    fn empty_channel_leaves_after_grace() {
        let mut session = session();
        let start = Instant::now();

        assert!(session.observe(start, true, false).is_none());
        assert!(
            session
                .observe(start + EMPTY_GRACE - Duration::from_secs(1), true, false)
                .is_none()
        );
        assert!(matches!(
            session.observe(start + EMPTY_GRACE, true, false),
            Some(LeaveReason::Empty)
        ));
    }

    #[test]
    fn someone_returning_resets_the_timer() {
        let mut session = session();
        let start = Instant::now();

        session.observe(start, true, false);
        session.observe(start + Duration::from_secs(29), false, false);
        assert!(
            session
                .observe(start + Duration::from_secs(31), true, false)
                .is_none()
        );
    }

    #[test]
    fn idle_queue_leaves_after_longer_grace() {
        let mut session = session();
        let start = Instant::now();

        assert!(session.observe(start, false, true).is_none());
        assert!(
            session.observe(start + EMPTY_GRACE, false, true).is_none(),
            "the empty-channel grace must not apply while people are present"
        );
        assert!(matches!(
            session.observe(start + IDLE_GRACE, false, true),
            Some(LeaveReason::Idle)
        ));
    }

    #[test]
    fn playing_to_an_empty_room_still_leaves() {
        let mut session = session();
        let start = Instant::now();

        session.observe(start, true, false);
        assert!(matches!(
            session.observe(start + EMPTY_GRACE, true, false),
            Some(LeaveReason::Empty)
        ));
    }
}
