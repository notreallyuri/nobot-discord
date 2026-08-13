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

fn blur(card: &image::DynamicImage) -> Result<Vec<u8>, AppError> {
    const DIVISOR: u32 = 14;

    let small = card.resize_exact(
        (card.width() / DIVISOR).max(1),
        (card.height() / DIVISOR).max(1),
        image::imageops::FilterType::Triangle,
    );

    encode(&small.resize_exact(
        card.width(),
        card.height(),
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
        profile::WIDTH,
        profile::HEIGHT,
        image::imageops::FilterType::Lanczos3,
    );

    Ok(resized)
}

pub fn data_uri(image: &[u8]) -> String {
    format!("data:image/jpeg;base64,{}", STANDARD.encode(image))
}

pub fn fit_to_card(stored: Vec<u8>) -> Vec<u8> {
    let Ok(decoded) = image::load_from_memory(&stored) else {
        return stored;
    };

    if decoded.width() == profile::WIDTH && decoded.height() == profile::HEIGHT {
        return stored;
    }

    let resized = decoded.resize_to_fill(
        profile::WIDTH,
        profile::HEIGHT,
        image::imageops::FilterType::Lanczos3,
    );

    encode(&resized).unwrap_or(stored)
}

fn derive_blur(sharp: &[u8]) -> Option<Vec<u8>> {
    let card = image::load_from_memory(sharp).ok()?;
    blur(&card).ok()
}

pub async fn uris_for_card(
    sharp: Option<Vec<u8>>,
    blurred: Option<Vec<u8>>,
) -> (Option<String>, Option<String>) {
    if sharp.is_none() && blurred.is_none() {
        return (None, None);
    }

    tokio::task::spawn_blocking(move || {
        let sharp = sharp.map(fit_to_card);

        let blurred = match blurred {
            Some(bytes) => Some(fit_to_card(bytes)),
            None => sharp.as_deref().and_then(derive_blur),
        };

        (
            sharp.map(|bytes| data_uri(&bytes)),
            blurred.map(|bytes| data_uri(&bytes)),
        )
    })
    .await
    .unwrap_or((None, None))
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

        for (label, bytes) in [("sharp", &out.sharp), ("blurred", &out.blurred)] {
            let decoded = image::load_from_memory(bytes).expect("valid jpeg");
            assert_eq!(decoded.width(), profile::WIDTH, "{label} width");
            assert_eq!(decoded.height(), profile::HEIGHT, "{label} height");
        }
    }

    #[test]
    fn the_blurred_copy_loses_its_detail() {
        let out = prepare_bytes(&checkerboard()).expect("should normalise");

        let sharp = detail(&out.sharp);
        let blurred = detail(&out.blurred);

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
