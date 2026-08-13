use crate::modules::voice::{
    repeat,
    setup::{self, TrackMeta},
    sources::Retrying,
    ytdlp,
};
use poise::serenity_prelude as serenity;
use songbird::{
    Call,
    input::{Input, YoutubeDl},
    tracks::Track,
};
use songbird::{
    Event, EventContext, EventHandler, TrackEvent,
    tracks::{PlayMode, TrackHandle},
};
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

pub fn attach(
    handle: &TrackHandle,
    http: Arc<serenity::Http>,
    channel: serenity::ChannelId,
    meta: Arc<TrackMeta>,
) {
    let events = [
        (
            TrackEvent::Play,
            Notify {
                http: http.clone(),
                channel,
                meta: meta.clone(),
                kind: Kind::Started,
            },
        ),
        (
            TrackEvent::Error,
            Notify {
                http,
                channel,
                meta,
                kind: Kind::Failed,
            },
        ),
    ];

    for (event, handler) in events {
        if let Err(e) = handle.add_event(Event::Track(event), handler) {
            tracing::warn!(?e, ?event, "failed to attach track notifier");
        }
    }
}

pub struct Recycle {
    pub call: Arc<AsyncMutex<Call>>,
    pub http: reqwest::Client,
    pub modes: repeat::Modes,
    pub guild_id: i64,
    pub meta: Arc<TrackMeta>,
}

pub fn attach_repeat(handle: &TrackHandle, recycle: Recycle) {
    if let Err(e) = handle.add_event(Event::Track(TrackEvent::End), recycle) {
        tracing::warn!(?e, "failed to attach the repeat handler");
    }
}

fn rebuild(http: reqwest::Client, meta: &TrackMeta) -> Input {
    let source = match meta.url.as_deref().filter(|url| url.starts_with("http")) {
        Some(url) => YoutubeDl::new_ytdl_like(ytdlp::program(), http, url.to_string()),
        None => YoutubeDl::new_search_ytdl_like(ytdlp::program(), http, meta.title.clone()),
    };

    Input::Lazy(Box::new(Retrying::new(source)))
}

#[async_trait::async_trait]
impl EventHandler for Recycle {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        if repeat::get(&self.modes, self.guild_id) != repeat::Mode::Queue {
            return None;
        }

        let EventContext::Track(states) = ctx else {
            return None;
        };

        if !matches!(states.first()?.0.playing, PlayMode::End) {
            return None;
        }

        let input = rebuild(self.http.clone(), &self.meta);
        let mut call = self.call.lock().await;
        let handle = call
            .enqueue(Track::new_with_data(input, self.meta.clone()))
            .await;

        attach_repeat(
            &handle,
            Recycle {
                call: self.call.clone(),
                http: self.http.clone(),
                modes: self.modes.clone(),
                guild_id: self.guild_id,
                meta: self.meta.clone(),
            },
        );

        tracing::debug!(track = %self.meta.title, "recycled for queue repeat");
        None
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Started,
    Failed,
}

struct Notify {
    http: Arc<serenity::Http>,
    channel: serenity::ChannelId,
    meta: Arc<TrackMeta>,
    kind: Kind,
}

impl Notify {
    fn title(&self) -> String {
        match &self.meta.url {
            Some(url) => format!("[{}]({})", self.meta.title, url),
            None => self.meta.title.clone(),
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for Notify {
    async fn act(&self, ctx: &EventContext<'_>) -> Option<Event> {
        let body = match self.kind {
            Kind::Started => format!(
                "🎵 Now playing **{}** · {}",
                self.title(),
                setup::fmt_len(self.meta.duration)
            ),
            Kind::Failed => {
                let reason = failure_reason(ctx);
                tracing::warn!(track = %self.meta.title, %reason, "track failed to play");
                format!("⚠️ Couldn't play **{}** — {reason}.", self.title())
            }
        };

        let message = serenity::CreateMessage::new()
            .content(body)
            .flags(serenity::MessageFlags::SUPPRESS_EMBEDS)
            .allowed_mentions(serenity::CreateAllowedMentions::new());

        if let Err(e) = self.channel.send_message(&self.http, message).await {
            tracing::warn!(?e, channel = %self.channel, "failed to send track notification");
        }

        None
    }
}

fn failure_reason(ctx: &EventContext<'_>) -> String {
    let EventContext::Track(states) = ctx else {
        return "playback failed".to_string();
    };

    let Some((state, _)) = states.first() else {
        return "playback failed".to_string();
    };

    let PlayMode::Errored(error) = &state.playing else {
        return "playback failed".to_string();
    };

    let detail = error.to_string();

    if detail.contains("403") {
        "the source refused the download (it may be age-restricted or region-locked)".to_string()
    } else if detail.contains("404") {
        "the source is no longer available".to_string()
    } else if detail.contains("no suitable format reader") {
        "its audio format isn't supported".to_string()
    } else {
        format!("playback failed ({detail})")
    }
}
