use crate::error::AppError;
use base64::{Engine, engine::general_purpose::STANDARD};
use poise::serenity_prelude as serenity;
use resvg::{
    tiny_skia,
    usvg::{self, fontdb},
};
use std::sync::{Arc, OnceLock};

pub mod accent;
pub mod background;
pub mod levelup;
pub mod profile;
pub mod welcome;

const FIGTREE: &[u8] = include_bytes!("../../../../assets/fonts/Figtree-Regular.ttf");
const FIGTREE_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/Figtree-Bold.ttf");
const NOTO_SANS: &[u8] = include_bytes!("../../../../assets/fonts/NotoSans-Regular.ttf");
const NOTO_SANS_BOLD: &[u8] = include_bytes!("../../../../assets/fonts/NotoSans-Bold.ttf");
const NOTO_CJK: &[u8] = include_bytes!("../../../../assets/fonts/NotoSansCJK-Regular.ttc");

pub const FONT_FAMILY: &str = "Figtree";
const AVATAR_PIXELS: u32 = 256;

fn fonts() -> Arc<fontdb::Database> {
    static FONTS: OnceLock<Arc<fontdb::Database>> = OnceLock::new();

    Arc::clone(FONTS.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_font_data(FIGTREE.to_vec());
        db.load_font_data(FIGTREE_BOLD.to_vec());
        db.load_font_data(NOTO_SANS.to_vec());
        db.load_font_data(NOTO_SANS_BOLD.to_vec());
        db.load_font_data(NOTO_CJK.to_vec());
        db.set_sans_serif_family(FONT_FAMILY);

        Arc::new(db)
    }))
}

pub const SUPERSAMPLE: u32 = 2;

fn rasterise(
    svg: &str,
    width: u32,
    height: u32,
    scale: u32,
) -> Result<tiny_skia::Pixmap, AppError> {
    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };

    let tree = usvg::Tree::from_str(svg, &options)
        .map_err(|e| AppError::Message(format!("Couldn't build the card: {e}")))?;

    let mut pixmap = tiny_skia::Pixmap::new(width * scale, height * scale)
        .ok_or_else(|| AppError::Message("Card dimensions are invalid.".into()))?;

    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale as f32, scale as f32),
        &mut pixmap.as_mut(),
    );

    Ok(pixmap)
}

fn straight_rgba(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    let mut rgba = vec![0u8; pixmap.data().len()];

    for (out, pixel) in rgba.chunks_exact_mut(4).zip(pixmap.pixels()) {
        let colour = pixel.demultiply();
        out.copy_from_slice(&[colour.red(), colour.green(), colour.blue(), colour.alpha()]);
    }

    rgba
}

pub fn render(svg: &str, width: u32, height: u32, scale: u32) -> Result<Vec<u8>, AppError> {
    let pixmap = rasterise(svg, width, height, scale)?;

    let rgba = straight_rgba(&pixmap);

    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new_with_quality(
        &mut out,
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::Adaptive,
    );

    image::ImageEncoder::write_image(
        encoder,
        &rgba,
        pixmap.width(),
        pixmap.height(),
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|e| AppError::Message(format!("Couldn't encode the card: {e}")))?;

    Ok(out)
}

pub async fn render_async(
    svg: String,
    width: u32,
    height: u32,
    scale: u32,
) -> Result<Vec<u8>, AppError> {
    tokio::task::spawn_blocking(move || render(&svg, width, height, scale))
        .await
        .map_err(|_| AppError::Message("Card rendering panicked.".into()))?
}

pub async fn avatar_data_uri(http: &reqwest::Client, user: &serenity::User) -> Option<String> {
    let url = user.face().replace(".webp", ".png").replace(".gif", ".png");
    let url = match url.split_once('?') {
        Some((base, _)) => format!("{base}?size={AVATAR_PIXELS}"),
        None => format!("{url}?size={AVATAR_PIXELS}"),
    };

    let bytes = async {
        let response = http.get(&url).send().await.ok()?;
        if !response.status().is_success() {
            tracing::warn!(status = %response.status(), %url, "avatar fetch rejected");
            return None;
        }
        response.bytes().await.ok()
    }
    .await?;

    Some(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }

    out
}

fn display_width(c: char) -> usize {
    let wide = matches!(u32::from(c),
        0x1100..=0x115F
            | 0x2E80..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x2_0000..=0x3_FFFD
    );

    1 + usize::from(wide)
}

pub fn display_columns(text: &str) -> usize {
    text.chars().map(display_width).sum()
}

pub fn text_width(text: &str, size: f64) -> Option<f64> {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="8000" height="200">
              <text x="0" y="100" font-family="{FONT_FAMILY}" font-size="{size}"
                    font-weight="bold">{}</text>
            </svg>"##,
        escape(text)
    );

    let options = usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };

    usvg::Tree::from_str(&svg, &options)
        .ok()
        .map(|tree| f64::from(tree.root().abs_bounding_box().width()))
}

pub fn truncate(name: &str, budget: usize) -> String {
    let total = display_columns(name);
    if total <= budget {
        return name.to_string();
    }

    let mut used = 0;
    let mut kept = String::new();
    for c in name.chars() {
        let width = display_width(c);
        if used + width > budget.saturating_sub(1) {
            break;
        }
        used += width;
        kept.push(c);
    }

    format!("{kept}…")
}

pub fn compact(value: i64) -> String {
    match value {
        v if v >= 1_000_000 => format!("{:.1}M", v as f64 / 1_000_000.0),
        v if v >= 10_000 => format!("{:.1}k", v as f64 / 1_000.0),
        v => v.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_markup_in_names() {
        assert_eq!(
            escape("<script>alert('x')</script>"),
            "&lt;script&gt;alert(&apos;x&apos;)&lt;/script&gt;"
        );
        assert_eq!(escape("a & b"), "a &amp; b");
    }

    #[test]
    fn strips_control_characters() {
        assert_eq!(escape("na\u{0}me\u{7}"), "name");
    }

    #[test]
    fn truncates_by_display_width() {
        assert_eq!(truncate("short", 10), "short");

        assert_eq!(display_width('日'), 2);
        assert_eq!(display_width('a'), 1);
        assert_eq!(truncate("日本語のユーザー名", 10), "日本語の…");

        assert_eq!(truncate("abcdefghijklmno", 10), "abcdefghi…");
    }

    #[test]
    fn truncation_never_exceeds_its_budget() {
        let names = [
            "yuri",
            "MMMMMMMMMMMMMMMMMMMM",
            "日本語のユーザー名前です",
            "mixed日本語name",
            "",
        ];

        for name in names {
            for budget in [4, 10, 16, 40] {
                let width: usize = truncate(name, budget).chars().map(display_width).sum();
                assert!(
                    width <= budget,
                    "{name:?} at budget {budget} produced {width} columns"
                );
            }
        }
    }

    #[test]
    fn compacts_large_numbers() {
        assert_eq!(compact(999), "999");
        assert_eq!(compact(9_999), "9999");
        assert_eq!(compact(12_400), "12.4k");
        assert_eq!(compact(2_500_000), "2.5M");
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;

    #[test]
    fn both_cards_rasterise() {
        let accent = accent::Accent::default();
        let level_up = levelup::svg(&levelup::LevelUp {
            name: "yuri",
            accent: &accent,
            avatar: None,
            from: 6,
            to: 7,
        });

        let custom = accent::Accent::new(accent::Rgb(0x22, 0xcc, 0x88));
        let profile = profile::svg(&profile::Profile {
            name: "日本語のユーザー",
            handle: "nihongo_user",
            accent: &custom,
            avatar: None,
            background: None,
            background_blur: None,
            guild: profile::Standing {
                level: 7,
                rank: 3,
                experience: 4_900,
                progress: (940, 1_500),
            },
            global: profile::Standing {
                level: 12,
                rank: 148,
                experience: 14_400,
                progress: (1_900, 2_500),
            },
            badges: &[
                crate::modules::leveling::badges::find("veteran").expect("veteran"),
                crate::modules::leveling::badges::find("melody").expect("melody"),
                crate::modules::leveling::badges::find("peak").expect("peak"),
                crate::modules::leveling::badges::find("heart").expect("heart"),
            ],
            coins: 1_240,
            currency: "coins",
        });

        let cards = [
            ("levelup", level_up, levelup::WIDTH, levelup::HEIGHT),
            ("profile", profile, profile::WIDTH, profile::HEIGHT),
        ];

        for (name, svg, width, height) in cards {
            let png = render(&svg, width, height, 1).expect("card should rasterise");

            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{name} is not a PNG");
            assert!(
                png.len() > 2_000,
                "{name} looks blank ({} bytes)",
                png.len()
            );

            if let Ok(dir) = std::env::var("CARD_DUMP") {
                std::fs::write(format!("{dir}/{name}.png"), &png).expect("dump");
            }

            let pixmap = rasterise(&svg, width, height, 1).expect("card should rasterise");
            for (label, x, y) in [
                ("top-left", 0, 0),
                ("top-right", width - 1, 0),
                ("bottom-left", 0, height - 1),
                ("bottom-right", width - 1, height - 1),
            ] {
                let alpha = pixmap.pixel(x, y).expect("in bounds").alpha();
                assert_eq!(alpha, 0, "{name} {label} corner is not transparent");
            }
        }
    }
}

#[cfg(test)]
mod background_render_tests {
    use super::*;

    #[test]
    fn profile_card_renders_a_background() {
        let source = image::RgbImage::from_pixel(1600, 400, image::Rgb([255, 255, 255]));
        let mut encoded = Vec::new();
        image::DynamicImage::ImageRgb8(source)
            .write_to(
                &mut std::io::Cursor::new(&mut encoded),
                image::ImageFormat::Png,
            )
            .expect("encode source");

        let stored = background::prepare_bytes(&encoded).expect("should normalise");
        let uri = background::data_uri(&stored.sharp);

        let svg = profile::svg(&profile::Profile {
            name: "yuri",
            handle: "yuri",
            accent: &accent::Accent::default(),
            avatar: None,
            background: Some(&uri),
            background_blur: Some(&uri),
            guild: profile::Standing {
                level: 7,
                rank: 3,
                experience: 4_900,
                progress: (940, 1_500),
            },
            global: profile::Standing {
                level: 12,
                rank: 148,
                experience: 14_400,
                progress: (1_900, 2_500),
            },
            badges: &[],
            coins: 0,
            currency: "coins",
        });

        let pixmap =
            rasterise(&svg, profile::WIDTH, profile::HEIGHT, 1).expect("card should rasterise");

        for (label, x, y) in [
            ("identity header", 300, 45),
            ("stats panel", 320, 230),
            ("stats footer", 300, 500),
            ("badges rail", 500, 240),
        ] {
            let pixel = pixmap.pixel(x, y).expect("in bounds");
            assert!(
                pixel.red() < 190 && pixel.green() < 190 && pixel.blue() < 190,
                "the {label} is not scrimmed at all: {pixel:?}"
            );
        }

        assert_eq!(pixmap.pixel(0, 0).expect("in bounds").alpha(), 0);
        assert_eq!(
            pixmap
                .pixel(profile::WIDTH - 1, profile::HEIGHT - 1)
                .expect("in bounds")
                .alpha(),
            0
        );

        if let Ok(dir) = std::env::var("CARD_DUMP") {
            let png = render(&svg, profile::WIDTH, profile::HEIGHT, 1).expect("encode");
            std::fs::write(format!("{dir}/profile-bg.png"), png).expect("dump");
        }
    }
}

#[cfg(test)]
mod badge_closeup {
    use super::*;
    use crate::modules::leveling::badges;

    #[test]
    #[ignore = "diagnostic: render badges large for a visual check"]
    fn large_badges() {
        let ids = ["veteran", "elder", "legend", "melody", "sun", "bloom"];
        let plates: String = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let badge = badges::find(id).expect("known badge");
                badges::render(badge, i, 70.0 + (i as f64) * 130.0, 80.0, 55.0)
            })
            .collect();

        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="820" height="160"
                     viewBox="0 0 820 160">
              <rect width="820" height="160" fill="#1b1f2b"/>{plates}</svg>"##
        );

        let png = render(&svg, 820, 160, 1).expect("render");
        let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");
        std::fs::write(format!("{dir}/badges-large.png"), png).expect("write");
    }
}

#[cfg(test)]
mod levelup_widths {
    use super::*;

    #[test]
    #[ignore = "diagnostic: check the small card against awkward names"]
    fn name_widths() {
        let accent = accent::Accent::default();
        let names = [
            "yuri",
            "MMMMMMMMMMMMMMMMMM",
            "a_very_long_username_here",
            "日本語のユーザー名前",
        ];

        for (i, name) in names.iter().enumerate() {
            let svg = levelup::svg(&levelup::LevelUp {
                name,
                accent: &accent,
                avatar: None,
                from: if i == 3 { 99 } else { 9 },
                to: if i == 3 { 100 } else { 10 },
            });
            let png = render(&svg, levelup::WIDTH, levelup::HEIGHT, 1).expect("render");
            let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");
            std::fs::write(format!("{dir}/levelup-{i}.png"), png).expect("write");
        }
    }
}

#[cfg(test)]
mod blur_probe {
    use super::*;

    #[test]
    #[ignore = "diagnostic"]
    fn gaussian_blur_is_supported() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"
                           viewBox="0 0 200 100">
          <defs>
            <filter id="b" x="-50%" y="-50%" width="200%" height="200%">
              <feGaussianBlur stdDeviation="6"/>
            </filter>
          </defs>
          <rect width="200" height="100" fill="#101018"/>
          <rect x="10" y="20" width="60" height="60" fill="#ffffff"/>
          <rect x="120" y="20" width="60" height="60" fill="#ffffff" filter="url(#b)"/>
        </svg>"##;

        let pixmap = rasterise(svg, 200, 100, 1).expect("render");

        let sharp_outside = pixmap.pixel(6, 50).expect("in bounds").red();
        let blurred_outside = pixmap.pixel(116, 50).expect("in bounds").red();

        println!("sharp rect, 4px outside edge:   red={sharp_outside}");
        println!("blurred rect, 4px outside edge: red={blurred_outside}");

        assert!(sharp_outside < 40, "unfiltered rect should not bleed");
        assert!(
            blurred_outside > 40,
            "feGaussianBlur appears unsupported: no bleed outside the rect"
        );
    }
}

#[cfg(test)]
mod layout_prototype {
    use super::*;
    use crate::modules::leveling::badges;

    struct Spec {
        name: &'static str,
        width: f64,
        height: f64,
        pad: f64,
        hero_header: bool,
        avatar_r: f64,
        name_size: f64,
        header: (f64, f64),
        server: (f64, f64),
        global: (f64, f64),
        badges: (f64, f64),
        badge_r: f64,
    }

    const VARIANTS: &[Spec] = &[
        Spec {
            name: "a-hero",
            width: 440.0,
            height: 572.0,
            pad: 20.0,
            hero_header: true,
            avatar_r: 44.0,
            name_size: 24.0,
            header: (20.0, 150.0),
            server: (186.0, 140.0),
            global: (342.0, 90.0),
            badges: (448.0, 104.0),
            badge_r: 22.0,
        },
        Spec {
            name: "b-refined",
            width: 440.0,
            height: 512.0,
            pad: 20.0,
            hero_header: false,
            avatar_r: 36.0,
            name_size: 22.0,
            header: (20.0, 104.0),
            server: (140.0, 136.0),
            global: (292.0, 84.0),
            badges: (392.0, 100.0),
            badge_r: 21.0,
        },
        Spec {
            name: "c-compact",
            width: 420.0,
            height: 420.0,
            pad: 16.0,
            hero_header: false,
            avatar_r: 30.0,
            name_size: 19.0,
            header: (16.0, 88.0),
            server: (116.0, 112.0),
            global: (240.0, 68.0),
            badges: (320.0, 84.0),
            badge_r: 18.0,
        },
    ];

    fn panel(s: &Spec, (y, h): (f64, f64), i: usize) -> String {
        let (x, w) = (s.pad, s.width - s.pad * 2.0);
        format!(
            r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"
                      fill="#ffffff" opacity="0.045"/>
                <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18" fill="none"
                      stroke="#ffffff" stroke-opacity="0.10" stroke-width="1"/>
                <!-- panel {i} -->"##
        )
    }

    fn label(s: &Spec, y: f64, text: &str, accent: &accent::Accent, family: &str) -> String {
        format!(
            r##"<text x="{x}" y="{y}" font-family="{F}" font-size="11" font-weight="bold"
                      fill="{c}" letter-spacing="2">{text}</text>"##,
            x = s.pad + 20.0,
            F = family,
            c = accent.light
        )
    }

    fn build(s: &Spec, accent: &accent::Accent) -> String {
        build_with(s, accent, FONT_FAMILY)
    }

    fn build_with(s: &Spec, accent: &accent::Accent, family: &str) -> String {
        let (l, r) = (s.pad + 20.0, s.width - s.pad - 20.0);
        let (hy, hh) = s.header;

        let header = if s.hero_header {
            let cx = s.width / 2.0;
            let cy = hy + s.avatar_r + 22.0;
            format!(
                r##"<circle cx="{cx}" cy="{cy}" r="{ar}" fill="#2b3145"/>
                    <text x="{cx}" y="{cy}" font-family="{F}" font-size="{init}" font-weight="bold"
                          fill="#8b93a8" text-anchor="middle" dominant-baseline="central">Y</text>
                    <circle cx="{cx}" cy="{cy}" r="{ring}" fill="none" stroke="url(#accent)" stroke-width="3"/>
                    <text x="{cx}" y="{ny}" font-family="{F}" font-size="{ns}" font-weight="bold"
                          fill="#f4f6fb" text-anchor="middle">yuri</text>"##,
                ar = s.avatar_r,
                ring = s.avatar_r + 2.0,
                init = s.avatar_r * 0.86,
                ny = hy + hh - 18.0,
                ns = s.name_size,
                F = family,
            )
        } else {
            let cx = l + s.avatar_r;
            let cy = hy + hh / 2.0;
            let tx = cx + s.avatar_r + 18.0;
            format!(
                r##"<circle cx="{cx}" cy="{cy}" r="{ar}" fill="#2b3145"/>
                    <text x="{cx}" y="{cy}" font-family="{F}" font-size="{init}" font-weight="bold"
                          fill="#8b93a8" text-anchor="middle" dominant-baseline="central">Y</text>
                    <circle cx="{cx}" cy="{cy}" r="{ring}" fill="none" stroke="url(#accent)" stroke-width="3"/>
                    <text x="{tx}" y="{ny}" font-family="{F}" font-size="{ns}" font-weight="bold"
                          fill="#f4f6fb">yuri</text>
                    <text x="{tx}" y="{sy}" font-family="{F}" font-size="{ss}" fill="#9aa4bd">
                      Level 7 · Rank #3
                    </text>"##,
                ar = s.avatar_r,
                ring = s.avatar_r + 2.0,
                init = s.avatar_r * 0.86,
                ny = cy - 4.0,
                sy = cy + 18.0,
                ns = s.name_size,
                ss = s.name_size * 0.62,
                F = family,
            )
        };

        let (sy, sh) = s.server;
        let bar_y = sy + sh - 58.0;
        let (gy, gh) = s.global;
        let (by, bh) = s.badges;
        let strip: String = ["veteran", "elder", "legend", "melody", "peak", "heart"]
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let badge = badges::find(id).expect("badge");
                let step = s.badge_r * 2.0 + 16.0;
                badges::render(
                    badge,
                    i,
                    l + s.badge_r + (i as f64) * step,
                    by + bh - s.badge_r - 14.0,
                    s.badge_r,
                )
            })
            .collect();

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#12141d"/><stop offset="100%" stop-color="#1d2130"/>
    </linearGradient>
    <linearGradient id="accent" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0%" stop-color="{al}"/><stop offset="100%" stop-color="{ab}"/>
    </linearGradient>
    <clipPath id="card"><rect width="{w}" height="{h}" rx="26"/></clipPath>
  </defs>
  <rect width="{w}" height="{h}" rx="26" fill="url(#bg)"/>
  <g clip-path="url(#card)"><circle cx="{gx}" cy="-30" r="150" fill="{ab}" opacity="0.14"/></g>

  {p0}{header}
  {p1}{server_label}
  <text x="{l}" y="{lvl_y}" font-family="{F}" font-size="{lvl}" font-weight="bold" fill="#f4f6fb">Level 7</text>
  <text x="{r}" y="{lvl_y}" font-family="{F}" font-size="{sub}" fill="#9aa4bd" text-anchor="end">Rank #3</text>
  <rect x="{l}" y="{bar_y}" width="{barw}" height="14" rx="7" fill="#2b3145"/>
  <rect x="{l}" y="{bar_y}" width="{fill}" height="14" rx="7" fill="url(#accent)"/>
  <text x="{l}" y="{xp_y}" font-family="{F}" font-size="13" fill="#9aa4bd">940 / 1500 XP to level 8</text>
  <text x="{r}" y="{xp_y}" font-family="{F}" font-size="13" fill="#9aa4bd" text-anchor="end">4900 XP</text>

  {p2}{global_label}
  <text x="{l}" y="{g_y}" font-family="{F}" font-size="{glvl}" font-weight="bold" fill="#f4f6fb">Level 12</text>
  <text x="{gmid}" y="{g_y}" font-family="{F}" font-size="{sub}" fill="#9aa4bd">Rank #148</text>
  <text x="{r}" y="{g_y}" font-family="{F}" font-size="{sub}" fill="#9aa4bd" text-anchor="end">14.4k XP</text>

  {p3}{badges_label}
  <text x="{r}" y="{b_y}" font-family="{F}" font-size="14" font-weight="bold" fill="{al}"
        text-anchor="end">1,240 coins</text>
  {strip}
</svg>"##,
            w = s.width,
            h = s.height,
            F = family,
            ab = accent.base,
            al = accent.light,
            gx = s.width - 40.0,
            p0 = panel(s, s.header, 0),
            p1 = panel(s, s.server, 1),
            p2 = panel(s, s.global, 2),
            p3 = panel(s, s.badges, 3),
            server_label = label(s, sy + 26.0, "THIS SERVER", accent, family),
            global_label = label(s, gy + 26.0, "GLOBAL", accent, family),
            badges_label = label(s, by + 26.0, "BADGES", accent, family),
            lvl_y = sy + 62.0,
            lvl = s.name_size * 1.2,
            sub = s.name_size * 0.82,
            barw = r - l,
            fill = (r - l) * 0.627,
            xp_y = bar_y + 36.0,
            g_y = gy + gh - 22.0,
            glvl = s.name_size,
            gmid = l + (r - l) * 0.45,
            b_y = by + 26.0,
        )
    }

    fn rasterise_with(svg: &str, w: u32, h: u32, family: &str, files: &[&str]) -> Vec<u8> {
        let mut db = fontdb::Database::new();
        for path in files {
            let data = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            db.load_font_data(data);
        }

        db.load_font_data(NOTO_SANS.to_vec());
        db.load_font_data(NOTO_CJK.to_vec());
        db.set_sans_serif_family(family);

        let options = usvg::Options {
            fontdb: Arc::new(db),
            ..Default::default()
        };
        let tree = usvg::Tree::from_str(svg, &options).expect("parse");
        let mut pixmap = tiny_skia::Pixmap::new(w, h).expect("pixmap");
        resvg::render(
            &tree,
            tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        pixmap.encode_png().expect("png")
    }

    #[test]
    #[ignore = "prototype: compare candidate fonts"]
    fn compare_fonts() {
        let accent = accent::Accent::default();
        let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");
        let spec = &VARIANTS[1];

        let candidates: [(&str, Vec<&str>); 5] = [
            ("Noto Sans", vec![]),
            (
                "Figtree",
                vec![
                    "/tmp/fonttest/Figtree_400Regular.ttf",
                    "/tmp/fonttest/Figtree_700Bold.ttf",
                ],
            ),
            (
                "Inter",
                vec![
                    "/tmp/fonttest/Inter_400Regular.ttf",
                    "/tmp/fonttest/Inter_700Bold.ttf",
                ],
            ),
            (
                "Manrope",
                vec![
                    "/tmp/fonttest/Manrope_400Regular.ttf",
                    "/tmp/fonttest/Manrope_700Bold.ttf",
                ],
            ),
            (
                "Outfit",
                vec![
                    "/tmp/fonttest/Outfit_400Regular.ttf",
                    "/tmp/fonttest/Outfit_700Bold.ttf",
                ],
            ),
        ];

        for (family, files) in &candidates {
            let svg = build_with(spec, &accent, family);
            let png = rasterise_with(&svg, spec.width as u32, spec.height as u32, family, files);
            let slug = family.to_lowercase().replace(' ', "-");
            std::fs::write(format!("{dir}/font-{slug}.png"), png).expect("write");
            println!("rendered {family}");
        }
    }

    #[test]
    #[ignore = "prototype: compare layout proportions"]
    fn compare_variants() {
        let accent = accent::Accent::default();
        let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");

        for spec in VARIANTS {
            let png = render(
                &build(spec, &accent),
                spec.width as u32,
                spec.height as u32,
                1,
            )
            .expect("render");
            std::fs::write(format!("{dir}/var-{}.png", spec.name), png).expect("write");
            println!("{}: {}x{}", spec.name, spec.width, spec.height);
        }
    }
}

#[cfg(test)]
mod timing {
    use super::*;
    use std::time::Instant;

    fn ms(label: &str, start: Instant) {
        println!(
            "{label:<38} {:>8.1} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }

    #[test]
    #[ignore = "diagnostic: where does a profile render spend its time"]
    fn profile_render_breakdown() {
        let source = image::RgbImage::from_fn(2000, 1200, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        });
        let mut encoded = Vec::new();
        image::DynamicImage::ImageRgb8(source)
            .write_to(
                &mut std::io::Cursor::new(&mut encoded),
                image::ImageFormat::Png,
            )
            .expect("encode source");

        let start = Instant::now();
        let prepared = background::prepare_bytes(&encoded).expect("normalise");
        ms("background::prepare_bytes (upload)", start);

        let start = Instant::now();
        let refit = background::fit_to_card(prepared.sharp.clone());
        ms("fit_to_card (already correct size)", start);

        let stale = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            760,
            380,
            image::Rgb([120, 90, 200]),
        ));
        let mut stale_bytes = Vec::new();
        stale
            .write_to(
                &mut std::io::Cursor::new(&mut stale_bytes),
                image::ImageFormat::Jpeg,
            )
            .expect("encode stale");

        let start = Instant::now();
        let _ = background::fit_to_card(stale_bytes.clone());
        ms("fit_to_card (stale size, resizes)", start);

        let start = Instant::now();
        let sharp = background::data_uri(&refit);
        let blurred = background::data_uri(&prepared.blurred);
        ms("data_uri x2 (base64)", start);

        let accent = accent::Accent::default();
        let build = || {
            profile::svg(&profile::Profile {
                name: "yuri",
                handle: "yuri",
                accent: &accent,
                avatar: None,
                background: Some(&sharp),
                background_blur: Some(&blurred),
                guild: profile::Standing {
                    level: 7,
                    rank: 3,
                    experience: 4_900,
                    progress: (940, 1_500),
                },
                global: profile::Standing {
                    level: 12,
                    rank: 148,
                    experience: 14_400,
                    progress: (1_900, 2_500),
                },
                badges: &[],
                coins: 1_240,
                currency: "coins",
            })
        };

        let start = Instant::now();
        let svg = build();
        ms("profile::svg (includes fit_name)", start);
        println!("{:<38} {:>8} KB", "svg payload", svg.len() / 1024);

        for scale in [1, SUPERSAMPLE] {
            let start = Instant::now();
            let pixmap = rasterise(&svg, profile::WIDTH, profile::HEIGHT, scale).expect("render");
            ms(&format!("rasterise @{scale}x"), start);

            let start = Instant::now();
            let baseline = pixmap.encode_png().expect("png");
            ms(&format!("  tiny_skia encode_png @{scale}x"), start);
            println!(
                "{:<38} {:>8} KB",
                format!("  tiny_skia png @{scale}x"),
                baseline.len() / 1024
            );

            let start = Instant::now();
            let png = render(&svg, profile::WIDTH, profile::HEIGHT, scale).expect("render");
            ms(&format!("  render() end to end @{scale}x"), start);
            println!(
                "{:<38} {:>8} KB",
                format!("  render() png @{scale}x"),
                png.len() / 1024
            );
        }
    }
}
