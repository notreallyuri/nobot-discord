use crate::error::AppError;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::ImageReader;
use poise::serenity_prelude as serenity;
use std::{io::Cursor, sync::OnceLock};

use super::profile;

const ALLOWED_HOSTS: [&str; 2] = ["cdn.discordapp.com", "media.discordapp.net"];
const MAX_UPLOAD_BYTES: u64 = 8 * 1024 * 1024;
const MAX_STREAM_BYTES: usize = 8 * 1024 * 1024;
const MAX_DIMENSION: u32 = 6_000;
const MAX_DECODED_BYTES: u64 = 256 * 1024 * 1024;

const JPEG_QUALITY: u8 = 82;

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("background http client")
    })
}

pub struct Prepared {
    pub sharp: Vec<u8>,
    pub blurred: Vec<u8>,
}

pub async fn prepare(attachment: &serenity::Attachment) -> Result<Prepared, AppError> {
    check_metadata(attachment)?;
    let bytes = download(&attachment.url).await?;

    tokio::task::spawn_blocking(move || prepare_bytes(&bytes))
        .await
        .map_err(|_| AppError::Message("Processing that image panicked.".into()))?
}

fn check_metadata(attachment: &serenity::Attachment) -> Result<(), AppError> {
    let declared = attachment.content_type.as_deref().unwrap_or_default();
    if !declared.starts_with("image/") {
        return Err(AppError::Message(
            "That attachment isn't an image.".to_string(),
        ));
    }

    if u64::from(attachment.size) > MAX_UPLOAD_BYTES {
        return Err(AppError::Message(format!(
            "That image is too large — the limit is {} MB.",
            MAX_UPLOAD_BYTES / 1024 / 1024
        )));
    }

    if !is_allowed_host(&attachment.url) {
        tracing::warn!(url = %attachment.url, "attachment from unexpected host");
        return Err(AppError::Message(
            "That attachment didn't come from Discord.".to_string(),
        ));
    }

    Ok(())
}

pub fn is_allowed_host(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url) else {
        return false;
    };

    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return false;
    }

    parsed
        .host_str()
        .is_some_and(|host| ALLOWED_HOSTS.contains(&host.to_ascii_lowercase().as_str()))
}

async fn download(url: &str) -> Result<Vec<u8>, AppError> {
    let response = client()
        .get(url)
        .send()
        .await
        .map_err(|_| AppError::Message("Couldn't download that image.".into()))?;

    if !response.status().is_success() {
        return Err(AppError::Message(
            "Couldn't download that image from Discord.".into(),
        ));
    }

    if let Some(len) = response.content_length()
        && len > MAX_UPLOAD_BYTES
    {
        return Err(AppError::Message("That image is too large.".into()));
    }

    let mut response = response;
    let mut bytes: Vec<u8> = Vec::new();

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::Message("That image failed to download.".into()))?
    {
        if bytes.len() + chunk.len() > MAX_STREAM_BYTES {
            return Err(AppError::Message("That image is too large.".into()));
        }
        bytes.extend_from_slice(&chunk);
    }

    if bytes.is_empty() {
        return Err(AppError::Message("That image was empty.".into()));
    }

    Ok(bytes)
}

pub fn prepare_bytes(bytes: &[u8]) -> Result<Prepared, AppError> {
    let card = normalise(bytes)?;

    Ok(Prepared {
        blurred: blur(&card)?,
        sharp: encode(&card)?,
    })
}

const BLUR_SCALE: u32 = 4;

fn blur(card: &image::DynamicImage) -> Result<Vec<u8>, AppError> {
    const DIVISOR: u32 = 14;

    let small = card.resize_exact(
        (card.width() / DIVISOR).max(1),
        (card.height() / DIVISOR).max(1),
        image::imageops::FilterType::Triangle,
    );

    encode(&small.resize_exact(
        (card.width() / BLUR_SCALE).max(1),
        (card.height() / BLUR_SCALE).max(1),
        image::imageops::FilterType::Triangle,
    ))
}

fn encode(card: &image::DynamicImage) -> Result<Vec<u8>, AppError> {
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY)
        .encode_image(&card.to_rgb8())
        .map_err(|_| AppError::Message("Couldn't re-encode that image.".into()))?;

    Ok(out)
}

fn normalise(bytes: &[u8]) -> Result<image::DynamicImage, AppError> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| AppError::Message("Couldn't read that image.".into()))?;

    if reader.format().is_none() {
        return Err(AppError::Message(
            "That file isn't an image format I understand.".into(),
        ));
    }

    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_BYTES);
    reader.limits(limits);

    let decoded = reader.decode().map_err(|e| {
        tracing::debug!(?e, "background decode rejected");
        AppError::Message("That image couldn't be decoded — try a PNG or JPEG.".into())
    })?;

    let resized = decoded.resize_to_fill(
        profile::PIXEL_WIDTH,
        profile::PIXEL_HEIGHT,
        image::imageops::FilterType::Lanczos3,
    );

    Ok(resized)
}

pub fn data_uri(image: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", STANDARD.encode(image))
}

fn resize_to(stored: Vec<u8>, width: u32, height: u32) -> (Vec<u8>, bool) {
    if let Ok(reader) = ImageReader::new(Cursor::new(&stored)).with_guessed_format()
        && let Ok(dimensions) = reader.into_dimensions()
        && dimensions == (width, height)
    {
        return (stored, false);
    }

    let Ok(decoded) = image::load_from_memory(&stored) else {
        return (stored, false);
    };

    let resized = decoded.resize_to_fill(width, height, image::imageops::FilterType::Lanczos3);

    match encode(&resized) {
        Ok(bytes) => (bytes, true),
        Err(_) => (stored, false),
    }
}

fn derive_blur(sharp: &[u8]) -> Option<Vec<u8>> {
    let card = image::load_from_memory(sharp).ok()?;
    blur(&card).ok()
}

pub struct CardImages {
    pub sharp: Option<String>,
    pub blurred: Option<String>,
    pub restore: Option<Prepared>,
}

pub async fn uris_for_card(sharp: Option<Vec<u8>>, blurred: Option<Vec<u8>>) -> CardImages {
    if sharp.is_none() && blurred.is_none() {
        return CardImages {
            sharp: None,
            blurred: None,
            restore: None,
        };
    }

    tokio::task::spawn_blocking(move || {
        let (sharp, sharp_changed) = match sharp {
            Some(bytes) => {
                let (bytes, changed) =
                    resize_to(bytes, profile::PIXEL_WIDTH, profile::PIXEL_HEIGHT);
                (Some(bytes), changed)
            }
            None => (None, false),
        };

        let (blurred, blur_changed) = match blurred {
            Some(bytes) => {
                let (bytes, changed) = resize_to(
                    bytes,
                    profile::PIXEL_WIDTH / BLUR_SCALE,
                    profile::PIXEL_HEIGHT / BLUR_SCALE,
                );
                (Some(bytes), changed)
            }
            None => (sharp.as_deref().and_then(derive_blur), true),
        };

        let restore = match (&sharp, &blurred) {
            (Some(sharp), Some(blurred)) if sharp_changed || blur_changed => Some(Prepared {
                sharp: sharp.clone(),
                blurred: blurred.clone(),
            }),
            _ => None,
        };

        CardImages {
            sharp: sharp.map(|bytes| data_uri(&bytes)),
            blurred: blurred.map(|bytes| data_uri(&bytes)),
            restore,
        }
    })
    .await
    .unwrap_or(CardImages {
        sharp: None,
        blurred: None,
        restore: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_discord_cdn_hosts() {
        assert!(is_allowed_host(
            "https://cdn.discordapp.com/attachments/1/2/pic.png"
        ));
        assert!(is_allowed_host(
            "https://media.discordapp.net/attachments/1/2/pic.png?width=100"
        ));
    }

    #[test]
    fn rejects_everything_else() {
        for url in [
            "http://cdn.discordapp.com/a.png",
            "https://169.254.169.254/latest/meta-data/",
            "https://localhost/a.png",
            "https://evil.com/a.png",
            "https://cdn.discordapp.com.evil.com/a.png",
            "https://notcdn.discordapp.com/a.png",
            "https://cdn.discordapp.com@evil.com/a.png",
            "https://user:pass@cdn.discordapp.com/a.png",
            "https://cdn.discordapp.com:8080/a.png",
            "cdn.discordapp.com/a.png",
            "file:///etc/passwd",
            "",
        ] {
            assert!(!is_allowed_host(url), "should have rejected: {url}");
        }
    }

    #[test]
    fn host_matching_ignores_case() {
        assert!(is_allowed_host(
            "https://CDN.DiscordApp.com/attachments/1/2/a.png"
        ));
    }

    #[test]
    fn rejects_non_image_attachments() {
        let mut attachment = attachment();
        attachment.content_type = Some("application/zip".into());
        assert!(check_metadata(&attachment).is_err());

        attachment.content_type = None;
        assert!(check_metadata(&attachment).is_err());
    }

    #[test]
    fn rejects_oversized_attachments() {
        let mut attachment = attachment();
        attachment.size = (MAX_UPLOAD_BYTES + 1) as u32;
        assert!(check_metadata(&attachment).is_err());
    }

    #[test]
    fn accepts_a_plausible_upload() {
        assert!(check_metadata(&attachment()).is_ok());
    }

    #[test]
    fn rejects_bytes_that_are_not_an_image() {
        assert!(prepare_bytes(b"definitely not an image").is_err());
    }

    fn detail(bytes: &[u8]) -> f64 {
        let image = image::load_from_memory(bytes)
            .expect("valid jpeg")
            .to_luma8();
        let mut total = 0.0;
        let mut samples = 0.0;

        for y in 0..image.height() {
            for x in 1..image.width() {
                let left = f64::from(image.get_pixel(x - 1, y).0[0]);
                let here = f64::from(image.get_pixel(x, y).0[0]);
                total += (left - here).abs();
                samples += 1.0;
            }
        }

        total / samples
    }

    fn checkerboard() -> Vec<u8> {
        let source = image::RgbImage::from_fn(200, 900, |x, y| {
            let on = (x / 3 + y / 3) % 2 == 0;
            image::Rgb([if on { 235 } else { 20 }, (y % 256) as u8, 128])
        });

        let mut encoded = Vec::new();
        image::DynamicImage::ImageRgb8(source)
            .write_to(&mut Cursor::new(&mut encoded), image::ImageFormat::Png)
            .expect("encode source");

        encoded
    }

    #[test]
    fn normalises_to_card_dimensions() {
        let out = prepare_bytes(&checkerboard()).expect("should normalise");

        let sharp = image::load_from_memory(&out.sharp).expect("valid jpeg");
        assert_eq!(sharp.width(), profile::PIXEL_WIDTH, "sharp width");
        assert_eq!(sharp.height(), profile::PIXEL_HEIGHT, "sharp height");

        let blurred = image::load_from_memory(&out.blurred).expect("valid jpeg");
        assert_eq!(
            blurred.width(),
            profile::PIXEL_WIDTH / BLUR_SCALE,
            "blurred width"
        );
        assert_eq!(
            blurred.height(),
            profile::PIXEL_HEIGHT / BLUR_SCALE,
            "blurred height"
        );

        assert!(
            out.blurred.len() * 4 < out.sharp.len(),
            "the blurred copy should be a fraction of the sharp one: {} vs {} bytes",
            out.blurred.len(),
            out.sharp.len()
        );
    }

    fn jpeg_of(width: u32, height: u32) -> Vec<u8> {
        let source = image::RgbImage::from_fn(width, height, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(source)
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Jpeg)
            .expect("encode");
        out
    }

    fn dimensions(bytes: &[u8]) -> (u32, u32) {
        let decoded = image::load_from_memory(bytes).expect("valid jpeg");
        (decoded.width(), decoded.height())
    }

    #[tokio::test]
    async fn a_background_at_the_current_size_is_not_rewritten() {
        let prepared = prepare_bytes(&checkerboard()).expect("normalise");

        let images = uris_for_card(Some(prepared.sharp), Some(prepared.blurred)).await;

        assert!(images.sharp.is_some() && images.blurred.is_some());
        assert!(
            images.restore.is_none(),
            "a background already in shape should not cost a write on every render"
        );
    }

    #[tokio::test]
    async fn a_background_stored_at_an_older_size_is_handed_back_for_storage() {
        let stale_sharp = jpeg_of(760, 380);
        let stale_blur = jpeg_of(760, 380);

        let images = uris_for_card(Some(stale_sharp), Some(stale_blur)).await;

        let restore = images
            .restore
            .expect("a background from an older card size should be offered back for storage");

        assert_eq!(
            dimensions(&restore.sharp),
            (profile::PIXEL_WIDTH, profile::PIXEL_HEIGHT)
        );
        assert_eq!(
            dimensions(&restore.blurred),
            (
                profile::PIXEL_WIDTH / BLUR_SCALE,
                profile::PIXEL_HEIGHT / BLUR_SCALE
            )
        );

        let settled = uris_for_card(Some(restore.sharp), Some(restore.blurred)).await;
        assert!(
            settled.restore.is_none(),
            "storing the refitted copy should settle it for good"
        );
    }

    #[tokio::test]
    async fn a_missing_blur_is_derived_and_offered_for_storage() {
        let prepared = prepare_bytes(&checkerboard()).expect("normalise");

        let images = uris_for_card(Some(prepared.sharp), None).await;

        assert!(
            images.blurred.is_some(),
            "the blur should have been derived"
        );
        assert!(
            images.restore.is_some(),
            "a derived blur should be stored rather than rebuilt every render"
        );
    }

    #[test]
    fn the_blurred_copy_loses_its_detail() {
        let out = prepare_bytes(&checkerboard()).expect("should normalise");

        let scaled_up = image::load_from_memory(&out.blurred)
            .expect("valid jpeg")
            .resize_exact(
                profile::PIXEL_WIDTH,
                profile::PIXEL_HEIGHT,
                image::imageops::FilterType::Triangle,
            );
        let mut scaled_bytes = Vec::new();
        scaled_up
            .write_to(&mut Cursor::new(&mut scaled_bytes), image::ImageFormat::Png)
            .expect("re-encode");

        let sharp = detail(&out.sharp);
        let blurred = detail(&scaled_bytes);

        assert!(
            blurred < sharp * 0.25,
            "blur barely changed anything: sharp {sharp:.2}, blurred {blurred:.2}"
        );
    }

    fn attachment() -> serenity::Attachment {
        let json = serde_json::json!({
            "id": "1",
            "filename": "pic.png",
            "size": 1024,
            "url": "https://cdn.discordapp.com/attachments/1/2/pic.png",
            "proxy_url": "https://media.discordapp.net/attachments/1/2/pic.png",
            "content_type": "image/png",
        });

        serde_json::from_value(json).expect("attachment fixture")
    }
}
