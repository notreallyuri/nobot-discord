use crate::error::AppError;
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

const RELEASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download";
const SUMS: &str = "SHA2-256SUMS";
const MAX_BYTES: usize = 128 * 1024 * 1024;

static PROGRAM: OnceLock<String> = OnceLock::new();

pub fn program() -> &'static str {
    PROGRAM.get().map_or("yt-dlp", String::as_str)
}

pub async fn resolve(http: &reqwest::Client) {
    match locate(http).await {
        Ok(found) => {
            let _ = PROGRAM.set(found);
        }
        Err(e) => {
            tracing::error!(?e, "could not provide yt-dlp, playback will not work");
        }
    }
}

async fn locate(http: &reqwest::Client) -> Result<String, AppError> {
    if let Some(configured) = std::env::var_os("YTDLP_PATH") {
        let path = PathBuf::from(configured);
        if version(&path.to_string_lossy()).await.is_none() {
            return Err(AppError::Message(format!(
                "YTDLP_PATH is set to {}, but that does not run.",
                path.display()
            )));
        }

        tracing::info!(path = %path.display(), "using yt-dlp from YTDLP_PATH");
        return Ok(path.to_string_lossy().into_owned());
    }

    if let Some(found) = version("yt-dlp").await {
        tracing::info!(version = %found, "using yt-dlp from PATH");
        return Ok("yt-dlp".to_string());
    }

    let path = cache_dir()?.join("yt-dlp");

    if let Some(found) = version(&path.to_string_lossy()).await {
        tracing::info!(version = %found, path = %path.display(), "using cached yt-dlp");
        return Ok(path.to_string_lossy().into_owned());
    }

    install(http, &path).await?;
    Ok(path.to_string_lossy().into_owned())
}

async fn install(http: &reqwest::Client, dest: &Path) -> Result<(), AppError> {
    let asset = asset()?;
    tracing::info!(asset, dest = %dest.display(), "yt-dlp not found, downloading");

    let expected = published_digest(http, asset).await?;
    let bytes = fetch(http, &format!("{RELEASE}/{asset}")).await?;
    let actual = hex(Sha256::digest(&bytes));

    if actual != expected {
        return Err(AppError::Message(format!(
            "yt-dlp download failed its checksum (expected {expected}, got {actual})."
        )));
    }

    write_executable(dest, &bytes).await?;

    let Some(found) = version(&dest.to_string_lossy()).await else {
        return Err(AppError::Message(format!(
            "downloaded yt-dlp to {} but it would not run.",
            dest.display()
        )));
    };

    tracing::info!(version = %found, path = %dest.display(), "yt-dlp installed");
    Ok(())
}

async fn version(program: &str) -> Option<String> {
    let output = tokio::process::Command::new(program)
        .arg("--version")
        .output()
        .await
        .ok()?;

    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|found| !found.is_empty())
}

fn asset() -> Result<&'static str, AppError> {
    let musl = cfg!(target_env = "musl");

    match (std::env::consts::OS, std::env::consts::ARCH, musl) {
        ("linux", "x86_64", false) => Ok("yt-dlp_linux"),
        ("linux", "x86_64", true) => Ok("yt-dlp_musllinux"),
        ("linux", "aarch64", false) => Ok("yt-dlp_linux_aarch64"),
        ("linux", "aarch64", true) => Ok("yt-dlp_musllinux_aarch64"),
        ("macos", _, _) => Ok("yt-dlp_macos"),
        (os, arch, _) => Err(AppError::Message(format!(
            "no prebuilt yt-dlp for {os}/{arch}: install it yourself and set YTDLP_PATH."
        ))),
    }
}

async fn published_digest(http: &reqwest::Client, asset: &str) -> Result<String, AppError> {
    let sums = fetch(http, &format!("{RELEASE}/{SUMS}")).await?;
    let sums = String::from_utf8_lossy(&sums);

    digest_for(&sums, asset)
        .ok_or_else(|| AppError::Message(format!("{SUMS} has no entry for {asset}.")))
}

fn digest_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines()
        .filter_map(|line| line.split_once("  "))
        .find(|(_, name)| name.trim() == asset)
        .map(|(digest, _)| digest.trim().to_ascii_lowercase())
        .filter(|digest| digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()))
}

async fn fetch(http: &reqwest::Client, url: &str) -> Result<Vec<u8>, AppError> {
    let mut response = http
        .get(url)
        .send()
        .await
        .map_err(|e| AppError::Message(format!("couldn't reach {url}: {e}")))?
        .error_for_status()
        .map_err(|e| AppError::Message(format!("couldn't download {url}: {e}")))?;

    let mut bytes = Vec::new();

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| AppError::Message(format!("{url} stopped downloading: {e}")))?
    {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err(AppError::Message(format!("{url} is bigger than expected.")));
        }
        bytes.extend_from_slice(&chunk);
    }

    Ok(bytes)
}

async fn write_executable(dest: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = dest
        .parent()
        .ok_or_else(|| AppError::Message(format!("{} has no parent directory.", dest.display())))?;

    let failed = |e: std::io::Error| AppError::Message(format!("couldn't install yt-dlp: {e}"));

    tokio::fs::create_dir_all(parent).await.map_err(failed)?;

    let staged = dest.with_extension("partial");
    tokio::fs::write(&staged, bytes).await.map_err(failed)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .await
            .map_err(failed)?;
    }

    tokio::fs::rename(&staged, dest).await.map_err(failed)
}

fn cache_dir() -> Result<PathBuf, AppError> {
    for (key, suffix) in [
        ("YTDLP_DIR", ""),
        ("XDG_CACHE_HOME", "dis-ru"),
        ("HOME", ".cache/dis-ru"),
    ] {
        if let Some(base) = std::env::var_os(key).filter(|value| !value.is_empty()) {
            return Ok(PathBuf::from(base).join(suffix));
        }
    }

    Err(AppError::Message(
        "nowhere to cache yt-dlp: set YTDLP_DIR or YTDLP_PATH.".into(),
    ))
}

fn hex(digest: impl AsRef<[u8]>) -> String {
    use std::fmt::Write;

    digest.as_ref().iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_SUMS: &str = "\
6bbb3d314cde4febe36e5fa1d55462e29c974f63444e707871834f6d8cc210ae  yt-dlp_linux
d7d2d09e900b5ae11821b5784b18cf064984a2bd88b1ca5c798d744bcbe3658b  yt-dlp_linux.zip
b6ce97646773070d7a7ffd6bbbdcaecb47c48483909c54c915bf08a7a9b5e0b1  yt-dlp_linux_aarch64
";

    #[test]
    fn picks_the_exact_asset_not_a_prefix() {
        assert_eq!(
            digest_for(REAL_SUMS, "yt-dlp_linux").as_deref(),
            Some("6bbb3d314cde4febe36e5fa1d55462e29c974f63444e707871834f6d8cc210ae")
        );
        assert_eq!(
            digest_for(REAL_SUMS, "yt-dlp_linux_aarch64").as_deref(),
            Some("b6ce97646773070d7a7ffd6bbbdcaecb47c48483909c54c915bf08a7a9b5e0b1")
        );
    }

    #[test]
    fn missing_or_malformed_entries_yield_nothing() {
        assert!(digest_for(REAL_SUMS, "yt-dlp_macos").is_none());
        assert!(digest_for("", "yt-dlp_linux").is_none());
        assert!(digest_for("garbage", "yt-dlp_linux").is_none());
        assert!(digest_for("short  yt-dlp_linux", "yt-dlp_linux").is_none());
        assert!(digest_for(&format!("{}  yt-dlp_linux", "z".repeat(64)), "yt-dlp_linux").is_none());
    }

    #[test]
    fn hexes_a_known_digest() {
        assert_eq!(
            hex(Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn every_supported_target_names_an_asset() {
        assert!(
            asset().is_ok(),
            "{}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }

    #[tokio::test]
    #[ignore = "downloads yt-dlp from GitHub"]
    async fn downloads_verifies_and_runs_a_real_yt_dlp() {
        let dir = std::env::temp_dir().join("dis-ru-ytdlp-test");
        let _ = std::fs::remove_dir_all(&dir);
        let dest = dir.join("yt-dlp");

        install(&reqwest::Client::new(), &dest)
            .await
            .expect("should download, checksum and install");

        let found = version(&dest.to_string_lossy())
            .await
            .expect("the installed binary should report a version");

        println!("installed yt-dlp {found} at {}", dest.display());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn an_explicit_directory_wins_over_the_rest() {
        unsafe { std::env::set_var("YTDLP_DIR", "/tmp/dis-ru-test") };
        assert_eq!(
            cache_dir().expect("set above"),
            PathBuf::from("/tmp/dis-ru-test")
        );
        unsafe { std::env::remove_var("YTDLP_DIR") };
    }
}
