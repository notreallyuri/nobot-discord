//! Validates the retry wrapper against YouTube's intermittent 403s.
//!
//! googlevideo throttles whole-file range requests, which is the only shape
//! songbird's `HttpRequest` sends; a bare `YoutubeDl` fails roughly 5-15% of
//! the time. This measures both so a regression in `Retrying` is visible.
//!
//!     cargo test --test http_client_probe -- --ignored --nocapture

use songbird::input::{
    Compose, Input, YoutubeDl,
    codecs::{get_codec_registry, get_probe},
};

const VIDEO: &str = "https://www.youtube.com/watch?v=uiBiKC0TC3I";
const ROUNDS: usize = 20;

/// Mirrors `dis_ru::modules::voice::sources::Retrying`, which lives in the
/// binary crate and so cannot be imported here.
struct Retrying<C> {
    inner: C,
    attempts: usize,
}

#[async_trait::async_trait]
impl<C: Compose> Compose for Retrying<C> {
    fn create(
        &mut self,
    ) -> Result<
        songbird::input::AudioStream<Box<dyn songbird::input::core::io::MediaSource>>,
        songbird::input::AudioStreamError,
    > {
        self.inner.create()
    }

    async fn create_async(
        &mut self,
    ) -> Result<
        songbird::input::AudioStream<Box<dyn songbird::input::core::io::MediaSource>>,
        songbird::input::AudioStreamError,
    > {
        let mut last = None;
        for attempt in 1..=self.attempts {
            match self.inner.create_async().await {
                Ok(stream) => return Ok(stream),
                Err(e) => {
                    last = Some(e);
                    if attempt < self.attempts {
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    }
                }
            }
        }
        Err(last.expect("at least one attempt"))
    }

    fn should_create_async(&self) -> bool {
        true
    }

    async fn aux_metadata(
        &mut self,
    ) -> Result<songbird::input::AuxMetadata, songbird::input::AudioStreamError> {
        self.inner.aux_metadata().await
    }
}

#[tokio::test]
#[ignore = "requires network access and yt-dlp; ~1 minute"]
async fn retrying_beats_a_bare_source() {
    let client = reqwest::Client::new();
    let mut bare_failures = 0;
    let mut retrying_failures = 0;

    for round in 1..=ROUNDS {
        let bare: Input = YoutubeDl::new(client.clone(), VIDEO).into();
        if bare
            .make_playable_async(get_codec_registry(), get_probe())
            .await
            .is_err()
        {
            bare_failures += 1;
            println!("round {round}: bare source failed");
        }

        let wrapped = Input::Lazy(Box::new(Retrying {
            inner: YoutubeDl::new(client.clone(), VIDEO),
            attempts: 4,
        }));
        if let Err(e) = wrapped
            .make_playable_async(get_codec_registry(), get_probe())
            .await
        {
            retrying_failures += 1;
            println!("round {round}: RETRYING source failed: {e:?}");
        }
    }

    println!(
        "\nover {ROUNDS} rounds: bare failed {bare_failures}, retrying failed {retrying_failures}"
    );

    assert_eq!(
        retrying_failures, 0,
        "retrying source should absorb the intermittent 403s (bare saw {bare_failures})"
    );
}
