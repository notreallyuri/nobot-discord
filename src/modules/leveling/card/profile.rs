use super::{FONT_FAMILY, accent::Accent, compact, escape, truncate};
use crate::modules::leveling::badges::{self, Badge};

pub const WIDTH: u32 = 760;
pub const HEIGHT: u32 = 380;

const PAD: f64 = 20.0;
const INSET: f64 = 22.0;

const RAIL: Panel = Panel::new(PAD, 20.0, 240.0, 340.0);
const SERVER: Panel = Panel::new(276.0, 20.0, 464.0, 156.0);
const GLOBAL: Panel = Panel::new(276.0, 192.0, 464.0, 68.0);
const STRIP: Panel = Panel::new(276.0, 276.0, 464.0, 84.0);

const AVATAR_R: f64 = 54.0;
const BADGE_R: f64 = 21.0;
const BADGE_STEP: f64 = BADGE_R * 2.0 + 16.0;

const NAME_MAX_SIZE: f64 = 21.0;
const NAME_MIN_SIZE: f64 = 13.0;
const NAME_BUDGET: usize = 28;

const SCRIM: &str = "#0b0d14";
const SCRIM_OPACITY: f64 = 0.65;

const INK_PRIMARY: &str = "#f4f6fb";
const HALO_DARKEN: f64 = 0.70;

fn halo(over_image: bool, colour: &str, size: f64) -> String {
    if !over_image {
        return String::new();
    }

    let shade = super::accent::parse(colour)
        .unwrap_or(super::accent::DEFAULT)
        .mix(super::accent::Rgb(0, 0, 0), HALO_DARKEN);

    format!(
        r##" stroke="{stroke}" stroke-width="{width:.2}"
             stroke-linejoin="round" paint-order="stroke""##,
        stroke = shade.to_hex(),
        width = (size * 0.15).clamp(1.6, 2.8),
    )
}

struct Ink {
    muted: &'static str,
    faint: &'static str,
}

const INK_PLAIN: Ink = Ink {
    muted: "#9aa4bd",
    faint: "#6b7488",
};

const INK_OVER_IMAGE: Ink = Ink {
    muted: "#c8d0e2",
    faint: "#a8b2c8",
};

#[derive(Clone, Copy)]
enum Fill<'a> {
    Plain,
    Frosted(&'a str),
    Solid,
}

fn fit_name(name: &str) -> (String, f64) {
    let room = RAIL.right() - RAIL.left();
    let mut shown = truncate(name, NAME_BUDGET);

    for _ in 0..4 {
        let Some(width) = super::text_width(&shown, NAME_MAX_SIZE) else {
            return (shown, NAME_MIN_SIZE);
        };

        if width <= room {
            return (shown, NAME_MAX_SIZE);
        }

        let scaled = NAME_MAX_SIZE * room / width;
        if scaled >= NAME_MIN_SIZE {
            return (shown, scaled);
        }

        let columns = super::display_columns(&shown);
        let keep = ((columns as f64) * (scaled / NAME_MIN_SIZE)).floor() as usize;
        let next = keep.clamp(4, columns.saturating_sub(1));

        if next >= columns {
            break;
        }

        shown = truncate(&shown, next);
    }

    (shown, NAME_MIN_SIZE)
}

pub struct Standing {
    pub level: i64,
    pub rank: i64,
    pub experience: i64,
    pub progress: (i64, i64),
}

pub struct Profile<'a> {
    pub name: &'a str,
    pub accent: &'a Accent,
    pub avatar: Option<&'a str>,
    pub background: Option<&'a str>,
    pub background_blur: Option<&'a str>,
    pub guild: Standing,
    pub global: Standing,
    pub badges: &'a [&'static Badge],
    pub coins: i64,
    pub currency: &'a str,
}

#[derive(Clone, Copy)]
struct Panel {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Panel {
    const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    fn left(self) -> f64 {
        self.x + INSET
    }

    fn right(self) -> f64 {
        self.x + self.w - INSET
    }

    fn centre(self) -> f64 {
        self.x + self.w / 2.0
    }

    fn render(self, index: usize, fill: Fill<'_>) -> String {
        let Self { x, y, w, h } = self;

        let body = match fill {
            Fill::Frosted(uri) => format!(
                r##"<clipPath id="panel{index}">
                      <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"/>
                    </clipPath>
                    <g clip-path="url(#panel{index})">
                      <image x="0" y="0" width="{WIDTH}" height="{HEIGHT}" href="{uri}"
                             preserveAspectRatio="xMidYMid slice"/>
                      <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"
                            fill="{SCRIM}" opacity="{SCRIM_OPACITY}"/>
                    </g>"##
            ),
            Fill::Solid => format!(
                r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"
                          fill="{SCRIM}" opacity="{SCRIM_OPACITY}"/>"##
            ),
            Fill::Plain => format!(
                r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"
                          fill="#ffffff" opacity="0.045"/>"##
            ),
        };

        format!(
            r##"{body}
                <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18" fill="none"
                      stroke="#ffffff" stroke-opacity="0.10" stroke-width="1"/>"##
        )
    }
}

fn section(panel: Panel, offset: f64, text: &str, accent: &Accent, over_image: bool) -> String {
    format!(
        r##"<text x="{x}" y="{y}" font-family="{FONT_FAMILY}" font-size="11"
                  font-weight="bold" fill="{c}" letter-spacing="2"{halo}>{text}</text>"##,
        x = panel.left(),
        y = panel.y + offset,
        c = accent.light,
        halo = halo(over_image, &accent.light, 11.0),
    )
}

pub fn svg(card: &Profile<'_>) -> String {
    let fill = match (card.background, card.background_blur) {
        (None, _) => Fill::Plain,
        (Some(_), Some(uri)) => Fill::Frosted(uri),
        (Some(_), None) => Fill::Solid,
    };

    let over_image = card.background.is_some();
    let ink = if over_image {
        &INK_OVER_IMAGE
    } else {
        &INK_PLAIN
    };

    let (shown, name_size) = fit_name(card.name);
    let name = escape(&shown);
    let initial = escape(
        &card
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string()),
    );

    let avatar_cx = RAIL.centre();
    let avatar_cy = RAIL.y + 84.0;

    let avatar = match card.avatar {
        Some(uri) => format!(
            r##"<image x="{x}" y="{y}" width="{size}" height="{size}"
                       clip-path="url(#avatar-clip)" href="{uri}"
                       preserveAspectRatio="xMidYMid slice"/>"##,
            x = avatar_cx - AVATAR_R,
            y = avatar_cy - AVATAR_R,
            size = AVATAR_R * 2.0,
        ),
        None => format!(
            r##"<circle cx="{avatar_cx}" cy="{avatar_cy}" r="{AVATAR_R}" fill="#2b3145"/>
                <text x="{avatar_cx}" y="{avatar_cy}" font-family="{FONT_FAMILY}"
                      font-size="46" font-weight="bold" fill="#8b93a8"
                      text-anchor="middle" dominant-baseline="central">{initial}</text>"##
        ),
    };

    let backdrop = match card.background {
        Some(uri) => format!(
            r##"<image x="0" y="0" width="{WIDTH}" height="{HEIGHT}" href="{uri}"
                       preserveAspectRatio="xMidYMid slice"/>
                <rect width="{WIDTH}" height="{HEIGHT}" fill="#0b0d14" opacity="0.42"/>"##
        ),
        None => format!(
            r##"<circle cx="{cx}" cy="-40" r="190" fill="{accent}" opacity="0.13"/>"##,
            cx = WIDTH as f64 - 60.0,
            accent = card.accent.base
        ),
    };

    let (earned, needed) = card.guild.progress;
    let fraction = if needed > 0 {
        (earned as f64 / needed as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let track = SERVER.right() - SERVER.left();
    let filled = (track * fraction).max(if fraction > 0.0 { 14.0 } else { 0.0 });

    let strip = if card.badges.is_empty() {
        format!(
            r##"<text x="{x}" y="{y}" font-family="{FONT_FAMILY}" font-size="14"
                      fill="{faint}"{halo}>No badges yet — see /shop</text>"##,
            x = STRIP.left(),
            y = STRIP.y + 60.0,
            faint = ink.faint,
            halo = halo(over_image, ink.faint, 14.0),
        )
    } else {
        card.badges
            .iter()
            .enumerate()
            .map(|(i, badge)| {
                let cx = STRIP.left() + BADGE_R + (i as f64) * BADGE_STEP;
                badges::render(badge, i, cx, STRIP.y + STRIP.h - BADGE_R - 12.0, BADGE_R)
            })
            .collect::<Vec<_>>()
            .join("")
    };

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg"
                 xmlns:xlink="http://www.w3.org/1999/xlink"
                 width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#12141d"/>
      <stop offset="100%" stop-color="#1d2130"/>
    </linearGradient>
    <linearGradient id="accent" x1="0" y1="0" x2="1" y2="0">
      <stop offset="0%" stop-color="{accent_light}"/>
      <stop offset="100%" stop-color="{accent_base}"/>
    </linearGradient>
    <clipPath id="card-clip"><rect width="{WIDTH}" height="{HEIGHT}" rx="26"/></clipPath>
    <clipPath id="avatar-clip">
      <circle cx="{avatar_cx}" cy="{avatar_cy}" r="{AVATAR_R}"/>
    </clipPath>
  </defs>

  <rect width="{WIDTH}" height="{HEIGHT}" rx="26" fill="url(#bg)"/>
  <g clip-path="url(#card-clip)">{backdrop}</g>

  {rail_panel}
  {avatar}
  <circle cx="{avatar_cx}" cy="{avatar_cy}" r="{ring}" fill="none" stroke="url(#accent)"
          stroke-width="3"/>
  <text x="{avatar_cx}" y="{name_y}" font-family="{FONT_FAMILY}" font-size="{name_size:.1}"
        font-weight="bold" fill="{INK_PRIMARY}" text-anchor="middle"{halo_name}>{name}</text>
  <text x="{avatar_cx}" y="{sub_y}" font-family="{FONT_FAMILY}" font-size="13"
        fill="{muted}" text-anchor="middle"{halo_muted_sm}>Level {guild_level} · Rank #{guild_rank}</text>
  <text x="{avatar_cx}" y="{coins_y}" font-family="{FONT_FAMILY}" font-size="17"
        font-weight="bold" fill="{accent_light}" text-anchor="middle"{halo_accent_md}>{coins} {currency}</text>

  {server_panel}
  {server_label}
  <text x="{s_left}" y="{s_lvl_y}" font-family="{FONT_FAMILY}" font-size="26"
        font-weight="bold" fill="{INK_PRIMARY}"{halo_primary_lg}>Level {guild_level}</text>
  <text x="{s_right}" y="{s_lvl_y}" font-family="{FONT_FAMILY}" font-size="18" fill="{muted}"
        text-anchor="end"{halo_muted_md}>Rank #{guild_rank}</text>
  <rect x="{s_left}" y="{bar_y}" width="{track}" height="14" rx="7" fill="#2b3145"/>
  <rect x="{s_left}" y="{bar_y}" width="{filled:.1}" height="14" rx="7" fill="url(#accent)"/>
  <text x="{s_left}" y="{xp_y}" font-family="{FONT_FAMILY}" font-size="13" fill="{muted}"{halo_muted_sm}>
    {earned} / {needed} XP to level {next_level}
  </text>
  <text x="{s_right}" y="{xp_y}" font-family="{FONT_FAMILY}" font-size="13" fill="{muted}"
        text-anchor="end"{halo_muted_sm}>{guild_xp} XP</text>

  {global_panel}
  {global_label}
  <text x="{g_left}" y="{g_y}" font-family="{FONT_FAMILY}" font-size="18" font-weight="bold"
        fill="{INK_PRIMARY}"{halo_primary_md}>Level {global_level}</text>
  <text x="{g_mid}" y="{g_y}" font-family="{FONT_FAMILY}" font-size="14" fill="{muted}"{halo_muted_14}>
    Rank #{global_rank}
  </text>
  <text x="{g_right}" y="{g_y}" font-family="{FONT_FAMILY}" font-size="14" fill="{muted}"
        text-anchor="end"{halo_muted_14}>{global_xp} XP</text>

  {strip_panel}
  {strip_label}
  {strip}
</svg>"##,
        accent_base = card.accent.base,
        accent_light = card.accent.light,
        muted = ink.muted,
        halo_name = halo(over_image, INK_PRIMARY, name_size),
        halo_primary_lg = halo(over_image, INK_PRIMARY, 26.0),
        halo_primary_md = halo(over_image, INK_PRIMARY, 18.0),
        halo_muted_sm = halo(over_image, ink.muted, 13.0),
        halo_muted_md = halo(over_image, ink.muted, 18.0),
        halo_muted_14 = halo(over_image, ink.muted, 14.0),
        halo_accent_md = halo(over_image, &card.accent.light, 17.0),
        ring = AVATAR_R + 2.0,
        rail_panel = RAIL.render(0, fill),
        server_panel = SERVER.render(1, fill),
        global_panel = GLOBAL.render(2, fill),
        strip_panel = STRIP.render(3, fill),
        server_label = section(SERVER, 30.0, "THIS SERVER", card.accent, over_image),
        global_label = section(GLOBAL, 26.0, "GLOBAL", card.accent, over_image),
        strip_label = section(STRIP, 26.0, "BADGES", card.accent, over_image),
        name_y = RAIL.y + 176.0,
        sub_y = RAIL.y + 200.0,
        coins_y = RAIL.y + 300.0,
        s_left = SERVER.left(),
        s_right = SERVER.right(),
        g_left = GLOBAL.left(),
        g_right = GLOBAL.right(),
        g_mid = GLOBAL.left() + (GLOBAL.right() - GLOBAL.left()) * 0.45,
        s_lvl_y = SERVER.y + 92.0,
        bar_y = SERVER.y + 112.0,
        xp_y = SERVER.y + 146.0,
        g_y = GLOBAL.y + 46.0,
        guild_level = card.guild.level,
        guild_rank = card.guild.rank,
        guild_xp = compact(card.guild.experience),
        global_level = card.global.level,
        global_rank = card.global.rank,
        global_xp = compact(card.global.experience),
        next_level = card.guild.level + 1,
        coins = compact(card.coins),
        currency = escape(card.currency),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card_for(name: &str) -> String {
        let accent = Accent::default();
        svg(&Profile {
            name,
            accent: &accent,
            avatar: None,
            background: None,
            background_blur: None,
            guild: Standing {
                level: 7,
                rank: 3,
                experience: 4_900,
                progress: (940, 1_500),
            },
            global: Standing {
                level: 12,
                rank: 148,
                experience: 14_400,
                progress: (1_900, 2_500),
            },
            badges: &[],
            coins: 1_240,
            currency: "coins",
        })
    }

    fn name_ink(name: &str) -> Option<(u32, u32)> {
        let pixmap = super::super::rasterise(&card_for(name), WIDTH, HEIGHT).expect("rasterise");

        let band = (RAIL.y + 160.0) as u32..(RAIL.y + 184.0) as u32;
        let mut bounds: Option<(u32, u32)> = None;

        for y in band {
            for x in 0..WIDTH {
                let pixel = pixmap.pixel(x, y).expect("in bounds");
                if pixel.red() > 200 && pixel.green() > 200 && pixel.blue() > 200 {
                    bounds = Some(match bounds {
                        Some((lo, hi)) => (lo.min(x), hi.max(x)),
                        None => (x, x),
                    });
                }
            }
        }

        bounds
    }

    fn white_background() -> super::super::background::Prepared {
        let source = image::RgbImage::from_pixel(1600, 900, image::Rgb([255, 255, 255]));
        let mut encoded = Vec::new();
        image::DynamicImage::ImageRgb8(source)
            .write_to(
                &mut std::io::Cursor::new(&mut encoded),
                image::ImageFormat::Png,
            )
            .expect("encode source");

        super::super::background::prepare_bytes(&encoded).expect("normalise")
    }

    fn panel_ground(pixmap: &resvg::tiny_skia::Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let pixel = pixmap.pixel(x, y).expect("in bounds");
        (pixel.red(), pixel.green(), pixel.blue())
    }

    fn card_over_white(with_blur: bool) -> resvg::tiny_skia::Pixmap {
        let prepared = white_background();
        let sharp = super::super::background::data_uri(&prepared.sharp);
        let blurred = super::super::background::data_uri(&prepared.blurred);
        let accent = Accent::default();

        let svg = svg(&Profile {
            name: "yuri",
            accent: &accent,
            avatar: None,
            background: Some(&sharp),
            background_blur: with_blur.then_some(blurred.as_str()),
            guild: Standing {
                level: 7,
                rank: 3,
                experience: 4_900,
                progress: (940, 1_500),
            },
            global: Standing {
                level: 12,
                rank: 148,
                experience: 14_400,
                progress: (1_900, 2_500),
            },
            badges: &[],
            coins: 1_240,
            currency: "coins",
        });

        super::super::rasterise(&svg, WIDTH, HEIGHT).expect("rasterise")
    }

    fn local_contrast(
        pixmap: &resvg::tiny_skia::Pixmap,
        xs: std::ops::Range<u32>,
        ys: std::ops::Range<u32>,
    ) -> f64 {
        let mut lightest = f64::MIN;
        let mut darkest = f64::MAX;

        for y in ys {
            for x in xs.clone() {
                let pixel = pixmap.pixel(x, y).expect("in bounds");
                let luminance =
                    super::super::accent::Rgb(pixel.red(), pixel.green(), pixel.blue()).luminance();

                lightest = lightest.max(luminance);
                darkest = darkest.min(luminance);
            }
        }

        (lightest + 0.05) / (darkest + 0.05)
    }

    #[test]
    fn text_stays_legible_over_a_white_background() {
        for with_blur in [true, false] {
            let pixmap = card_over_white(with_blur);

            for (label, xs, ys) in [
                ("rail sub-line", 60..220, 210..224),
                ("server xp line", 298..480, 156..172),
                ("global rank", 298..600, 228..244),
                ("badges notice", 298..520, 326..342),
            ] {
                let ratio = local_contrast(&pixmap, xs, ys);
                assert!(
                    ratio >= 4.5,
                    "the {label} only reaches {ratio:.2}:1 against what surrounds it \
                     (blur present: {with_blur})"
                );
            }
        }
    }

    #[test]
    fn the_scrim_still_lets_the_background_through() {
        let pixmap = card_over_white(true);

        for (label, x, y) in [("server", 500, 100), ("badges", 650, 310)] {
            let (r, g, b) = panel_ground(&pixmap, x, y);
            let brightness = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;

            assert!(
                brightness > 90,
                "the {label} panel is {brightness}, too opaque to show the background"
            );
            assert!(
                brightness < 190,
                "the {label} panel is {brightness}, barely scrimmed at all"
            );
        }
    }

    #[test]
    fn a_missing_blur_falls_back_to_a_darker_panel_never_a_brighter_one() {
        let with = card_over_white(true);
        let without = card_over_white(false);

        for (x, y) in [(500, 100), (650, 310)] {
            let (fallback, _, _) = panel_ground(&without, x, y);
            let (frosted, _, _) = panel_ground(&with, x, y);

            assert!(
                fallback <= frosted,
                "panel at ({x}, {y}) is {fallback} without a blur but only {frosted} with one — \
                 the fallback must never be the brighter of the two"
            );
        }
    }

    #[test]
    fn short_names_keep_the_largest_size_untruncated() {
        for name in ["yuri", "", "abcdefghij"] {
            let (shown, size) = fit_name(name);
            assert_eq!(shown, name);
            assert_eq!(size, NAME_MAX_SIZE, "{name:?}");
        }
    }

    #[test]
    fn wider_names_shrink_before_they_truncate() {
        let (shown, size) = fit_name("a_very_long_username_here");
        assert_eq!(
            shown, "a_very_long_username_here",
            "should not have truncated"
        );
        assert!(
            (NAME_MIN_SIZE..NAME_MAX_SIZE).contains(&size),
            "expected a shrunk-but-legible size, got {size}"
        );
    }

    #[test]
    fn truncation_is_the_last_resort() {
        let (shown, size) = fit_name(&"M".repeat(40));

        assert!(shown.ends_with('…'), "got {shown:?}");
        assert!(
            (NAME_MIN_SIZE..NAME_MIN_SIZE + 2.0).contains(&size),
            "should have bottomed out near the floor, got {size}"
        );
    }

    #[test]
    #[ignore = "diagnostic: render the rail against awkward names"]
    fn name_widths() {
        let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");

        for (i, name) in [
            "yuri",
            "a_very_long_username_here",
            "MMMMMMMMMMMMMMMMMMMMMMMMMMMM",
            "日本語のユーザー名前です",
        ]
        .iter()
        .enumerate()
        {
            let (shown, size) = fit_name(name);
            println!("{name:?} -> {shown:?} at {size:.1}px");

            let png = super::super::render(&card_for(name), WIDTH, HEIGHT).expect("render");
            std::fs::write(format!("{dir}/name-{i}.png"), png).expect("write");
        }
    }

    #[test]
    fn fitting_a_name_stays_cheap() {
        let start = std::time::Instant::now();
        for _ in 0..20 {
            fit_name("a_very_long_username_here");
        }
        let each = start.elapsed() / 20;

        assert!(
            each < std::time::Duration::from_millis(8),
            "fit_name took {each:?} per call"
        );
        println!("fit_name: {each:?} per call");
    }

    #[test]
    fn a_full_length_name_still_fits_inside_the_rail() {
        let names = [
            "yuri",
            "a_very_long_username_here",
            "MMMMMMMMMMMMMMMMMMMMMMMMMMMM",
            "日本語のユーザー名前です",
            "mixed日本語name_that_runs_on",
        ];

        for name in names {
            let (lo, hi) = name_ink(name).unwrap_or_else(|| panic!("{name:?} drew nothing"));

            assert!(
                f64::from(lo) >= RAIL.left() && f64::from(hi) <= RAIL.right(),
                "{name:?} drew from {lo} to {hi}, outside the rail's {}..{}",
                RAIL.left(),
                RAIL.right()
            );
        }
    }

    #[test]
    fn the_budget_bounds_even_pathological_names() {
        assert!(super::super::display_columns(&fit_name(&"あ".repeat(60)).0) <= NAME_BUDGET);
    }
}
