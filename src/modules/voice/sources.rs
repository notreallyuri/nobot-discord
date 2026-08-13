use crate::modules::voice::{
    spotify::{SpotifyRef, SpotifyTrack},
    ytdlp,
};
use songbird::input::{
    AudioStream, AudioStreamError, AuxMetadata, Compose, YoutubeDl, core::io::MediaSource,
};
use std::time::Duration;

pub struct Retrying<C> {
    inner: C,
    attempts: usize,
}

const ATTEMPTS: usize = 4;
const BACKOFF: Duration = Duration::from_millis(250);

impl<C: Compose> Retrying<C> {
    pub fn new(inner: C) -> Self {
        Self {
            inner,
            attempts: ATTEMPTS,
        }
    }
}

#[async_trait::async_trait]
impl<C: Compose> Compose for Retrying<C> {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        self.inner.create()
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        let mut last = None;

        for attempt in 1..=self.attempts {
            match self.inner.create_async().await {
                Ok(stream) => {
                    if attempt > 1 {
                        tracing::info!(attempt, "audio stream created after retry");
                    }
                    return Ok(stream);
                }
                Err(AudioStreamError::RetryIn(delay)) => {
                    return Err(AudioStreamError::RetryIn(delay));
                }
                Err(e) => {
                    tracing::warn!(attempt, ?e, "failed to create audio stream");
                    last = Some(e);
                    if attempt < self.attempts {
                        tokio::time::sleep(BACKOFF).await;
                    }
                }
            }
        }

        Err(last.unwrap_or(AudioStreamError::Unsupported))
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        self.inner.aux_metadata().await
    }
}

pub enum Query {
    Url(String),
    Search(String),
    Spotify(SpotifyRef),
}

impl Query {
    pub fn is_search(input: &str) -> bool {
        matches!(Self::classify(input), Self::Search(_))
    }

    pub fn classify(input: &str) -> Self {
        let input = input.trim();

        if let Some(reference) = SpotifyRef::parse(input) {
            return Self::Spotify(reference);
        }

        if input.starts_with("http://") || input.starts_with("https://") {
            return Self::Url(input.to_string());
        }

        Self::Search(input.to_string())
    }
}

pub struct DeferredSearch {
    client: reqwest::Client,
    query: String,
    metadata: AuxMetadata,
    resolved: Option<YoutubeDl<'static>>,
}

impl DeferredSearch {
    pub fn new(client: reqwest::Client, track: &SpotifyTrack) -> Self {
        let metadata = AuxMetadata {
            title: Some(track.title.clone()),
            artist: (!track.artists.is_empty()).then(|| track.artists.join(", ")),
            duration: track.duration,
            source_url: track.url.clone(),
            thumbnail: track.art.clone(),
            ..Default::default()
        };

        Self {
            client,
            query: track.search_query(),
            metadata,
            resolved: None,
        }
    }
}

#[async_trait::async_trait]
impl Compose for DeferredSearch {
    fn create(&mut self) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        Err(AudioStreamError::Unsupported)
    }

    async fn create_async(
        &mut self,
    ) -> Result<AudioStream<Box<dyn MediaSource>>, AudioStreamError> {
        if self.resolved.is_none() {
            tracing::debug!(query = %self.query, "resolving deferred track on YouTube");
            self.resolved = Some(YoutubeDl::new_search_ytdl_like(
                ytdlp::program(),
                self.client.clone(),
                self.query.clone(),
            ));
        }

        let source = self
            .resolved
            .as_mut()
            .expect("just initialised above if absent");

        source.create_async().await
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(&mut self) -> Result<AuxMetadata, AudioStreamError> {
        Ok(self.metadata.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bare_words_count_as_a_search() {
        for input in ["daft punk", "  lo-fi beats  ", "日本の音楽", "not.a.url"] {
            assert!(Query::is_search(input), "should search for: {input:?}");
        }
    }

    #[test]
    fn links_are_never_treated_as_a_search() {
        for input in [
            "https://www.youtube.com/watch?v=abc",
            "http://soundcloud.com/artist/track",
            "https://open.spotify.com/track/abc123",
            "spotify:album:xyz789",
            "  https://youtu.be/abc  ",
        ] {
            assert!(!Query::is_search(input), "should not search for: {input:?}");
        }
    }
}
