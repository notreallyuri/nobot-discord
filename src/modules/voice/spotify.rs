use crate::{config::SpotifyCredentials, error::AppError};
use reqwest::StatusCode;
use serde::{Deserialize, de::DeserializeOwned};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API: &str = "https://api.spotify.com/v1";
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);

pub struct SpotifyClient {
    http: reqwest::Client,
    creds: SpotifyCredentials,
    token: Mutex<Option<CachedToken>>,
}

struct CachedToken {
    value: String,
    expires_at: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyRef {
    Track(String),
    Album(String),
    Playlist(String),
}

impl SpotifyRef {
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim();

        if let Some(rest) = input.strip_prefix("spotify:") {
            let mut parts = rest.split(':');
            return Self::build(parts.next()?, parts.next()?);
        }

        let rest = input
            .strip_prefix("https://")
            .or_else(|| input.strip_prefix("http://"))?;
        let rest = rest.strip_prefix("open.spotify.com/")?;
        let rest = rest.split(['?', '#']).next()?;

        let mut segments = rest.split('/').filter(|s| !s.is_empty());
        let mut kind = segments.next()?;
        if kind.starts_with("intl-") {
            kind = segments.next()?;
        }

        Self::build(kind, segments.next()?)
    }

    fn build(kind: &str, id: &str) -> Option<Self> {
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
            return None;
        }

        let id = id.to_string();
        match kind {
            "track" => Some(Self::Track(id)),
            "album" => Some(Self::Album(id)),
            "playlist" => Some(Self::Playlist(id)),
            _ => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Track(_) => "track",
            Self::Album(_) => "album",
            Self::Playlist(_) => "playlist",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpotifyTrack {
    pub title: String,
    pub artists: Vec<String>,
    pub duration: Option<Duration>,
    pub url: Option<String>,
    pub art: Option<String>,
}

impl SpotifyTrack {
    pub fn search_query(&self) -> String {
        if self.artists.is_empty() {
            self.title.clone()
        } else {
            format!("{} - {}", self.artists.join(", "), self.title)
        }
    }
}

#[derive(Debug)]
pub struct Resolution {
    pub collection: Option<String>,
    pub art: Option<String>,
    pub tracks: Vec<SpotifyTrack>,
    pub truncated: bool,
}

impl SpotifyClient {
    pub fn new(http: reqwest::Client, creds: SpotifyCredentials) -> Self {
        Self {
            http,
            creds,
            token: Mutex::new(None),
        }
    }

    pub async fn resolve(&self, target: &SpotifyRef, limit: usize) -> Result<Resolution, AppError> {
        match target {
            SpotifyRef::Track(id) => {
                let track: ApiTrack = self.get(&format!("{API}/tracks/{id}")).await?;
                Ok(Resolution {
                    collection: None,
                    art: track.art(),
                    tracks: vec![track.into_track()],
                    truncated: false,
                })
            }
            SpotifyRef::Album(id) => {
                let album: ApiAlbumFull = self.get(&format!("{API}/albums/{id}")).await?;
                let art = album.images.first().map(|i| i.url.clone());
                let (tracks, truncated) = self
                    .drain(album.tracks, limit, |t: ApiTrack| Some(t.into_track()))
                    .await?;

                Ok(Resolution {
                    collection: Some(album.name),
                    art,
                    tracks,
                    truncated,
                })
            }
            SpotifyRef::Playlist(id) => {
                let list: ApiPlaylist = self.get(&format!("{API}/playlists/{id}")).await?;
                let art = list.images.first().map(|i| i.url.clone());
                let (tracks, truncated) = self
                    .drain(list.tracks, limit, |i: PlaylistItem| {
                        i.track.map(ApiTrack::into_track)
                    })
                    .await?;

                Ok(Resolution {
                    collection: Some(list.name),
                    art,
                    tracks,
                    truncated,
                })
            }
        }
    }

    async fn drain<T, F>(
        &self,
        first: Page<T>,
        limit: usize,
        convert: F,
    ) -> Result<(Vec<SpotifyTrack>, bool), AppError>
    where
        T: DeserializeOwned,
        F: Fn(T) -> Option<SpotifyTrack>,
    {
        let mut out = Vec::new();
        let mut page = first;

        loop {
            out.extend(page.items.into_iter().filter_map(&convert));

            if out.len() >= limit {
                let dropped = out.len() > limit || page.next.is_some();
                out.truncate(limit);
                return Ok((out, dropped));
            }

            let Some(next) = page.next else {
                return Ok((out, false));
            };
            page = self.get(&next).await?;
        }
    }

    async fn get<T: DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        for attempt in 0..2 {
            let token = self.access_token().await?;
            let response = self.http.get(url).bearer_auth(&token).send().await?;

            return match response.status() {
                s if s.is_success() => Ok(response.json().await?),
                StatusCode::UNAUTHORIZED if attempt == 0 => {
                    *self.token.lock().await = None;
                    continue;
                }
                StatusCode::NOT_FOUND => Err(AppError::Message(
                    "Couldn't find that on Spotify — the link may be private or region-locked."
                        .into(),
                )),
                StatusCode::TOO_MANY_REQUESTS => Err(AppError::Message(
                    "Spotify is rate-limiting us. Try again in a moment.".into(),
                )),
                status => {
                    tracing::warn!(%status, %url, "unexpected Spotify response");
                    Err(AppError::Message(format!(
                        "Spotify returned an error ({status})."
                    )))
                }
            };
        }

        Err(AppError::Message(
            "Spotify rejected our credentials. Check SPOTIFY_CLIENT_ID/SECRET.".into(),
        ))
    }

    async fn access_token(&self) -> Result<String, AppError> {
        let mut cached = self.token.lock().await;

        if let Some(token) = cached.as_ref()
            && Instant::now() < token.expires_at
        {
            return Ok(token.value.clone());
        }

        let response = self
            .http
            .post(TOKEN_URL)
            .basic_auth(&self.creds.client_id, Some(&self.creds.client_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            tracing::warn!(%status, "Spotify token request rejected");
            return Err(AppError::Message(
                "Spotify rejected our credentials. Check SPOTIFY_CLIENT_ID/SECRET.".into(),
            ));
        }

        let token: TokenResponse = response.json().await?;
        let expires_at =
            Instant::now() + Duration::from_secs(token.expires_in).saturating_sub(EXPIRY_MARGIN);

        *cached = Some(CachedToken {
            value: token.access_token.clone(),
            expires_at,
        });

        Ok(token.access_token)
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct Page<T> {
    items: Vec<T>,
    #[serde(default)]
    next: Option<String>,
}

#[derive(Deserialize)]
struct PlaylistItem {
    track: Option<ApiTrack>,
}

#[derive(Deserialize)]
struct ApiPlaylist {
    name: String,
    #[serde(default)]
    images: Vec<ApiImage>,
    tracks: Page<PlaylistItem>,
}

#[derive(Deserialize)]
struct ApiAlbumFull {
    name: String,
    #[serde(default)]
    images: Vec<ApiImage>,
    tracks: Page<ApiTrack>,
}

#[derive(Deserialize)]
struct ApiTrack {
    name: String,
    #[serde(default)]
    artists: Vec<ApiArtist>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    album: Option<ApiAlbumRef>,
    #[serde(default)]
    external_urls: Option<ExternalUrls>,
}

impl ApiTrack {
    fn art(&self) -> Option<String> {
        self.album
            .as_ref()?
            .images
            .first()
            .map(|image| image.url.clone())
    }

    fn into_track(self) -> SpotifyTrack {
        SpotifyTrack {
            art: self.art(),
            duration: self.duration_ms.map(Duration::from_millis),
            url: self.external_urls.and_then(|u| u.spotify),
            artists: self.artists.into_iter().map(|a| a.name).collect(),
            title: self.name,
        }
    }
}

#[derive(Deserialize)]
struct ApiAlbumRef {
    #[serde(default)]
    images: Vec<ApiImage>,
}

#[derive(Deserialize)]
struct ApiArtist {
    name: String,
}

#[derive(Deserialize)]
struct ExternalUrls {
    #[serde(default)]
    spotify: Option<String>,
}

#[derive(Deserialize)]
struct ApiImage {
    url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_link_shapes() {
        let cases = [
            (
                "https://open.spotify.com/track/abc123",
                SpotifyRef::Track("abc123".into()),
            ),
            (
                "https://open.spotify.com/intl-pt/track/abc123",
                SpotifyRef::Track("abc123".into()),
            ),
            (
                "https://open.spotify.com/album/xyz789?si=1",
                SpotifyRef::Album("xyz789".into()),
            ),
            ("spotify:playlist:pl42", SpotifyRef::Playlist("pl42".into())),
        ];

        for (input, expected) in cases {
            assert_eq!(SpotifyRef::parse(input), Some(expected), "input: {input}");
        }
    }

    fn offline_client() -> SpotifyClient {
        SpotifyClient::new(
            reqwest::Client::new(),
            SpotifyCredentials {
                client_id: "id".into(),
                client_secret: "secret".into(),
            },
        )
    }

    fn page(count: usize, next: Option<&str>) -> Page<ApiTrack> {
        Page {
            items: (0..count)
                .map(|i| ApiTrack {
                    name: format!("track {i}"),
                    artists: Vec::new(),
                    duration_ms: None,
                    album: None,
                    external_urls: None,
                })
                .collect(),
            next: next.map(str::to_string),
        }
    }

    async fn drained(count: usize, next: Option<&str>, limit: usize) -> (usize, bool) {
        let (tracks, truncated) = offline_client()
            .drain(page(count, next), limit, |t: ApiTrack| Some(t.into_track()))
            .await
            .expect("the first page is already in hand, so no request is made");

        (tracks.len(), truncated)
    }

    #[tokio::test]
    async fn a_page_that_exactly_fills_the_limit_is_not_truncated() {
        assert_eq!(drained(5, None, 5).await, (5, false));
    }

    #[tokio::test]
    async fn a_page_short_of_the_limit_is_not_truncated() {
        assert_eq!(drained(3, None, 5).await, (3, false));
    }

    #[tokio::test]
    async fn overflowing_the_limit_reports_truncation() {
        assert_eq!(drained(10, None, 5).await, (5, true));
    }

    #[tokio::test]
    async fn another_page_behind_a_filled_limit_reports_truncation() {
        assert_eq!(
            drained(5, Some("https://api.spotify.com/v1/next"), 5).await,
            (5, true)
        );
    }

    const TRACK_ID: &str = "1pKYYY0dkg23sQQXi0Q5zN";
    const ALBUM_ID: &str = "5uRdvUR7xCnHmUW8n64n9y";

    fn live_client() -> Option<SpotifyClient> {
        dotenvy::dotenv().ok();
        let client_id = std::env::var("SPOTIFY_CLIENT_ID").ok()?;
        let client_secret = std::env::var("SPOTIFY_CLIENT_SECRET").ok()?;

        Some(SpotifyClient::new(
            reqwest::Client::new(),
            SpotifyCredentials {
                client_id,
                client_secret,
            },
        ))
    }

    #[tokio::test]
    #[ignore = "requires network access and Spotify credentials"]
    async fn resolves_a_single_track() {
        let client = live_client().expect("SPOTIFY_CLIENT_ID/SECRET must be set");
        let resolution = client
            .resolve(&SpotifyRef::Track(TRACK_ID.into()), 100)
            .await
            .expect("track should resolve");

        assert_eq!(resolution.tracks.len(), 1);
        assert!(
            resolution.collection.is_none(),
            "a track is not a collection"
        );

        let track = &resolution.tracks[0];
        assert_eq!(track.title, "Around the World");
        assert_eq!(track.artists, ["Daft Punk"]);
        assert!(track.duration.is_some(), "duration_ms should deserialise");
        assert!(track.url.is_some(), "external_urls should deserialise");
        assert!(track.art.is_some(), "album art should deserialise");
        assert_eq!(track.search_query(), "Daft Punk - Around the World");
    }

    #[tokio::test]
    #[ignore = "requires network access and Spotify credentials"]
    async fn resolves_an_album_and_respects_the_limit() {
        let client = live_client().expect("SPOTIFY_CLIENT_ID/SECRET must be set");

        let full = client
            .resolve(&SpotifyRef::Album(ALBUM_ID.into()), 100)
            .await
            .expect("album should resolve");

        assert_eq!(full.collection.as_deref(), Some("Homework"));
        assert!(full.tracks.len() > 10, "got {} tracks", full.tracks.len());
        assert!(!full.truncated);
        assert!(full.art.is_some(), "album art should deserialise");
        assert!(full.tracks.iter().all(|t| !t.title.is_empty()));

        let capped = client
            .resolve(&SpotifyRef::Album(ALBUM_ID.into()), 5)
            .await
            .expect("album should resolve");

        assert_eq!(capped.tracks.len(), 5);
        assert!(capped.truncated, "hitting the cap must be reported");
    }

    #[test]
    fn rejects_non_spotify_and_malformed() {
        for input in [
            "https://www.youtube.com/watch?v=abc",
            "just some search words",
            "https://open.spotify.com/track/",
            "https://open.spotify.com/track/../../users",
            "https://open.spotify.com/artist/abc123",
        ] {
            assert_eq!(SpotifyRef::parse(input), None, "input: {input}");
        }
    }
}
