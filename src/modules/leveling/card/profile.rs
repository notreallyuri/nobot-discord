use super::{FONT_FAMILY, accent::Accent, compact, escape, truncate};
use crate::modules::leveling::badges::{self, Badge};

pub const WIDTH: u32 = 560;
pub const HEIGHT: u32 = 560;

pub const PIXEL_WIDTH: u32 = WIDTH * super::SUPERSAMPLE;
pub const PIXEL_HEIGHT: u32 = HEIGHT * super::SUPERSAMPLE;

const PAD: f64 = 28.0;
const INSET: f64 = 22.0;
const RAIL_INSET: f64 = 18.0;

const HEADER_Y: f64 = PAD;
const HEADER_H: f64 = 120.0;
const HEADER: Panel = Panel::new(PAD, HEADER_Y, 504.0, HEADER_H);
const BAR: Panel = Panel::new(PAD, 160.0, 504.0, 42.0);
const STATS: Panel = Panel::new(PAD, 214.0, 327.0, 318.0);
const STRIP: Panel = Panel::new(367.0, 214.0, 165.0, 318.0);

const AVATAR_R: f64 = 48.0;
const RING_WIDTH: f64 = 3.0;
const RING_R: f64 = AVATAR_R + 2.0;
const AVATAR_EDGE: f64 = RING_R + RING_WIDTH / 2.0;
const AVATAR_INSET: f64 = 16.0;
const AVATAR_CX: f64 = PAD + AVATAR_INSET + AVATAR_EDGE;
const AVATAR_CY: f64 = HEADER_Y + HEADER_H / 2.0;

const NAME_X: f64 = AVATAR_CX + AVATAR_EDGE + 18.0;
const NAME_Y: f64 = HEADER_Y + 57.0;
const HANDLE_Y: f64 = HEADER_Y + 83.0;

const _: () = {
    assert!(
        AVATAR_CX - AVATAR_EDGE >= PAD,
        "the avatar ring overhangs the left padding"
    );
    assert!(
        AVATAR_CY - AVATAR_EDGE >= HEADER_Y && AVATAR_CY + AVATAR_EDGE <= HEADER_Y + HEADER_H,
        "the avatar ring overhangs the header"
    );
    assert!(
        NAME_X >= AVATAR_CX + AVATAR_EDGE,
        "the name starts underneath the avatar ring"
    );
};

const BADGE_R: f64 = 24.0;
const BADGE_COLUMNS: usize = 2;
const BADGE_ROWS: usize = 4;
const BADGE_TOP: f64 = 46.0;

const BAR_TEXT: f64 = 15.0;

const NAME_MAX_SIZE: f64 = 27.0;
const NAME_MIN_SIZE: f64 = 15.0;
const NAME_BUDGET: usize = 30;

const HANDLE_MAX_SIZE: f64 = 15.0;
const HANDLE_BUDGET: usize = 34;

const SCRIM: &str = "#0b0d14";
const SCRIM_OPACITY: f64 = 0.72;

const TRACK: &str = "#2b3145";
const RULE: &str = "#ffffff";
const RULE_OPACITY: f64 = 0.10;

const INK_PRIMARY: &str = "#f4f6fb";
fn ink_on(colour: &str) -> &'static str {
    const BRIGHT: f64 = 0.35;

    let luminance = super::accent::parse(colour)
        .unwrap_or(super::accent::DEFAULT)
        .luminance();

    if luminance > BRIGHT {
        "#12141d"
    } else {
        INK_PRIMARY
    }
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
    muted: "#d4dbe9",
    faint: "#bcc6da",
};

#[derive(Clone, Copy)]
enum Fill<'a> {
    Plain,
    Frosted(&'a str),
    Solid,
}

fn fit(text: &str, room: f64, max: f64, min: f64, budget: usize) -> (String, f64) {
    let mut shown = truncate(text, budget);

    for _ in 0..4 {
        let Some(width) = super::text_width(&shown, max) else {
            return (shown, min);
        };

        if width <= room {
            return (shown, max);
        }

        let scaled = max * room / width;
        if scaled >= min {
            return (shown, scaled);
        }

        let columns = super::display_columns(&shown);
        let keep = ((columns as f64) * (scaled / min)).floor() as usize;
        let next = keep.clamp(4, columns.saturating_sub(1));

        if next >= columns {
            break;
        }

        shown = truncate(&shown, next);
    }

    (shown, min)
}

fn fit_name(name: &str) -> (String, f64) {
    fit(
        name,
        HEADER.right() - NAME_X,
        NAME_MAX_SIZE,
        NAME_MIN_SIZE,
        NAME_BUDGET,
    )
}

pub struct Standing {
    pub level: i64,
    pub rank: i64,
    pub experience: i64,
    pub progress: (i64, i64),
}

pub struct Profile<'a> {
    pub name: &'a str,
    pub handle: &'a str,
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

    fn bottom(self) -> f64 {
        self.y + self.h
    }

    fn centre(self) -> f64 {
        self.x + self.w / 2.0
    }

    fn shape(self) -> String {
        let Self { x, y, w, h } = self;
        format!(r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"/>"##)
    }

    fn body(self, fill: Fill<'_>) -> String {
        let Self { x, y, w, h } = self;

        let tint = match fill {
            Fill::Frosted(_) | Fill::Solid => format!(
                r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"
                          fill="{SCRIM}" opacity="{SCRIM_OPACITY}"/>"##
            ),
            Fill::Plain => format!(
                r##"<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18"
                          fill="#ffffff" opacity="0.045"/>"##
            ),
        };

        format!(
            r##"{tint}
                <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="18" fill="none"
                      stroke="{RULE}" stroke-opacity="{RULE_OPACITY}" stroke-width="1"/>"##
        )
    }
}

fn panel_grounds(panels: &[Panel], fill: Fill<'_>) -> String {
    let frosting = match fill {
        Fill::Frosted(uri) => {
            let shapes: String = panels.iter().map(|panel| panel.shape()).collect();
            format!(
                r##"<clipPath id="panels">{shapes}</clipPath>
                    <g clip-path="url(#panels)">
                      <image x="0" y="0" width="{WIDTH}" height="{HEIGHT}" href="{uri}"
                             preserveAspectRatio="xMidYMid slice"/>
                    </g>"##
            )
        }
        Fill::Plain | Fill::Solid => String::new(),
    };

    let bodies: String = panels.iter().map(|panel| panel.body(fill)).collect();
    format!("{frosting}{bodies}")
}

fn section(x: f64, y: f64, text: &str, accent: &Accent) -> String {
    format!(
        r##"<text x="{x}" y="{y}" font-family="{FONT_FAMILY}" font-size="11"
                  font-weight="bold" fill="{c}" letter-spacing="2">{text}</text>"##,
        c = accent.light,
    )
}

fn badge_slots(count: usize) -> impl Iterator<Item = (f64, f64)> {
    let count = count.min(BADGE_COLUMNS * BADGE_ROWS);

    let top = STRIP.y + BADGE_TOP;
    let step_x = (STRIP.w - RAIL_INSET * 2.0 - BADGE_R * 2.0) / (BADGE_COLUMNS - 1) as f64;
    let gap = (STRIP.bottom() - top - BADGE_R * 2.0 * BADGE_ROWS as f64) / (BADGE_ROWS + 1) as f64;

    (0..count).map(move |i| {
        let (column, row) = (i % BADGE_COLUMNS, i / BADGE_COLUMNS);
        (
            STRIP.x + RAIL_INSET + BADGE_R + (column as f64) * step_x,
            top + gap + BADGE_R + (row as f64) * (BADGE_R * 2.0 + gap),
        )
    })
}

fn rule(panel: Panel, y: f64) -> String {
    format!(
        r##"<line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="{RULE}"
                  stroke-opacity="{RULE_OPACITY}" stroke-width="1"/>"##,
        x1 = panel.left(),
        x2 = panel.right(),
    )
}

struct Theme<'a> {
    accent: &'a Accent,
    ink: &'a Ink,
}

fn standing(
    panel: Panel,
    top: f64,
    label: &str,
    value: &Standing,
    size: f64,
    t: &Theme<'_>,
) -> String {
    let (accent, ink) = (t.accent, t.ink);

    format!(
        r##"{label_text}
            <text x="{left}" y="{level_y}" font-family="{FONT_FAMILY}" font-size="{size}"
                  font-weight="bold" fill="{INK_PRIMARY}">Level {level}</text>
            <text x="{right}" y="{level_y}" font-family="{FONT_FAMILY}" font-size="{rank_size:.1}"
                  fill="{muted}" text-anchor="end">Rank #{rank}</text>
            <text x="{left}" y="{xp_y}" font-family="{FONT_FAMILY}" font-size="13"
                  fill="{muted}">{xp} XP total</text>"##,
        label_text = section(panel.left(), top, label, accent),
        left = panel.left(),
        right = panel.right(),
        level_y = top + size + 16.0,
        xp_y = top + size + 42.0,
        rank_size = size * 0.62,
        muted = ink.muted,
        level = value.level,
        rank = value.rank,
        xp = compact(value.experience),
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
    let theme = Theme {
        accent: card.accent,
        ink,
    };

    let (shown, name_size) = fit_name(card.name);
    let name = escape(&shown);

    let (handle_shown, handle_size) = fit(
        card.handle,
        HEADER.right() - NAME_X,
        HANDLE_MAX_SIZE,
        11.0,
        HANDLE_BUDGET,
    );
    let handle = escape(&handle_shown);

    let initial = escape(
        &card
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string()),
    );

    let avatar = match card.avatar {
        Some(uri) => format!(
            r##"<image x="{x}" y="{y}" width="{size}" height="{size}"
                       clip-path="url(#avatar-clip)" href="{uri}"
                       preserveAspectRatio="xMidYMid slice"/>"##,
            x = AVATAR_CX - AVATAR_R,
            y = AVATAR_CY - AVATAR_R,
            size = AVATAR_R * 2.0,
        ),
        None => format!(
            r##"<circle cx="{AVATAR_CX}" cy="{AVATAR_CY}" r="{AVATAR_R}" fill="{TRACK}"/>
                <text x="{AVATAR_CX}" y="{AVATAR_CY}" font-family="{FONT_FAMILY}"
                      font-size="42" font-weight="bold" fill="#8b93a8"
                      text-anchor="middle" dominant-baseline="central">{initial}</text>"##
        ),
    };

    let backdrop = match card.background {
        Some(uri) => format!(
            r##"<image x="0" y="0" width="{WIDTH}" height="{HEIGHT}" href="{uri}"
                       preserveAspectRatio="xMidYMid slice"/>
                <rect width="{WIDTH}" height="{HEIGHT}" fill="{SCRIM}" opacity="0.42"/>"##
        ),
        None => format!(
            r##"<circle cx="{cx}" cy="-60" r="175" fill="{accent}" opacity="0.11"/>"##,
            cx = f64::from(WIDTH) - 60.0,
            accent = card.accent.base
        ),
    };

    let panels = if over_image {
        vec![HEADER, STATS, STRIP]
    } else {
        vec![STATS, STRIP]
    };
    let grounds = panel_grounds(&panels, fill);

    let (earned, needed) = card.guild.progress;
    let fraction = if needed > 0 {
        (earned as f64 / needed as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let filled = (BAR.w * fraction).max(if fraction > 0.0 { BAR.h } else { 0.0 });

    let bar_ink = ink_on(&card.accent.light);
    let end_ink = if fraction > 0.85 {
        ink_on(&card.accent.base)
    } else {
        INK_PRIMARY
    };

    let strip = if card.badges.is_empty() {
        format!(
            r##"<text x="{cx}" y="{y}" font-family="{FONT_FAMILY}" font-size="12"
                      fill="{faint}" text-anchor="middle">
                  <tspan x="{cx}">No badges yet</tspan>
                  <tspan x="{cx}" dy="18">see /shop list</tspan>
                </text>"##,
            cx = STRIP.centre(),
            y = STRIP.y + 150.0,
            faint = ink.faint,
        )
    } else {
        card.badges
            .iter()
            .zip(badge_slots(card.badges.len()))
            .enumerate()
            .map(|(i, (badge, (cx, cy)))| badges::render(badge, i, cx, cy, BADGE_R))
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
      <circle cx="{AVATAR_CX}" cy="{AVATAR_CY}" r="{AVATAR_R}"/>
    </clipPath>
  </defs>

  <rect width="{WIDTH}" height="{HEIGHT}" rx="26" fill="url(#bg)"/>
  <g clip-path="url(#card-clip)">{backdrop}</g>

  {grounds}
  {avatar}
  <circle cx="{AVATAR_CX}" cy="{AVATAR_CY}" r="{RING_R}" fill="none" stroke="url(#accent)"
          stroke-width="{RING_WIDTH}"/>
  <text x="{NAME_X}" y="{NAME_Y}" font-family="{FONT_FAMILY}" font-size="{name_size:.1}"
        font-weight="bold" fill="{INK_PRIMARY}">{name}</text>
  <text x="{NAME_X}" y="{HANDLE_Y}" font-family="{FONT_FAMILY}" font-size="{handle_size:.1}"
        fill="{muted}">@{handle}</text>

  <rect x="{bar_x}" y="{bar_y}" width="{bar_w}" height="{bar_h}" rx="{bar_r}" fill="{TRACK}"/>
  <rect x="{bar_x}" y="{bar_y}" width="{filled:.1}" height="{bar_h}" rx="{bar_r}"
        fill="url(#accent)"/>
  <rect x="{bar_x}" y="{bar_y}" width="{bar_w}" height="{bar_h}" rx="{bar_r}" fill="none"
        stroke="{RULE}" stroke-opacity="{RULE_OPACITY}" stroke-width="1"/>
  <text x="{bar_label_x}" y="{bar_text_y}" font-family="{FONT_FAMILY}" font-size="11"
        font-weight="bold" fill="{bar_ink}" letter-spacing="2" opacity="0.75">XP</text>
  <text x="{bar_value_x}" y="{bar_text_y}" font-family="{FONT_FAMILY}" font-size="{BAR_TEXT}"
        font-weight="bold" fill="{bar_ink}">{earned} / {needed}</text>
  <text x="{bar_end_x}" y="{bar_text_y}" font-family="{FONT_FAMILY}" font-size="13"
        fill="{end_ink}" text-anchor="end">to level {next_level}</text>

  {guild_block}
  {guild_rule}
  {global_block}
  {global_rule}
  <text x="{stats_left}" y="{coins_y}" font-family="{FONT_FAMILY}" font-size="20"
        font-weight="bold" fill="{accent_light}">{coins}</text>
  <text x="{stats_right}" y="{coins_y}" font-family="{FONT_FAMILY}" font-size="13"
        fill="{muted}" text-anchor="end">{currency}</text>

  {strip_label}
  {strip}
</svg>"##,
        accent_base = card.accent.base,
        accent_light = card.accent.light,
        muted = ink.muted,
        bar_x = BAR.x,
        bar_y = BAR.y,
        bar_w = BAR.w,
        bar_h = BAR.h,
        bar_r = BAR.h / 2.0,
        bar_label_x = BAR.x + 18.0,
        bar_value_x = BAR.x + 44.0,
        bar_end_x = BAR.x + BAR.w - 18.0,
        bar_text_y = BAR.y + BAR.h / 2.0 + 5.0,
        stats_left = STATS.left(),
        stats_right = STATS.right(),
        guild_block = standing(
            STATS,
            STATS.y + 30.0,
            "THIS SERVER",
            &card.guild,
            30.0,
            &theme
        ),
        guild_rule = rule(STATS, STATS.y + 128.0),
        global_block = standing(STATS, STATS.y + 168.0, "GLOBAL", &card.global, 22.0, &theme),
        global_rule = rule(STATS, STATS.y + 258.0),
        coins_y = STATS.bottom() - 22.0,
        strip_label = section(STRIP.x + RAIL_INSET, STRIP.y + 30.0, "BADGES", card.accent),
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
            handle: "yuri",
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
        let pixmap = super::super::rasterise(&card_for(name), WIDTH, HEIGHT, 1).expect("rasterise");

        let band = (NAME_Y - 16.0) as u32..(NAME_Y - 2.0) as u32;
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
            handle: "yuri",
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

        super::super::rasterise(&svg, WIDTH, HEIGHT, 1).expect("rasterise")
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
                ("handle line", 150..510, 100..115),
                ("server xp line", 50..333, 274..290),
                ("global rank", 50..333, 400..420),
                ("badges notice", 385..510, 356..372),
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

        for (label, x, y) in [
            ("header", 300, 50),
            ("stats", 320, 480),
            ("badges", 500, 240),
        ] {
            let (r, g, b) = panel_ground(&pixmap, x, y);
            let brightness = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;

            assert!(
                brightness > 70,
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

        for (x, y) in [(300, 50), (320, 480), (500, 240)] {
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
        let wide = "a_very_long_username_goes_here";
        assert_eq!(super::super::display_columns(wide), NAME_BUDGET);

        let (shown, size) = fit_name(wide);
        assert_eq!(shown, wide, "should not have truncated");
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
    #[ignore = "diagnostic: render the header against awkward names"]
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

            let png = super::super::render(&card_for(name), WIDTH, HEIGHT, 1).expect("render");
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
    fn a_full_length_name_still_fits_beside_the_avatar() {
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
                f64::from(lo) >= NAME_X && f64::from(hi) <= HEADER.right(),
                "{name:?} drew from {lo} to {hi}, outside the header's {NAME_X}..{}",
                HEADER.right()
            );
        }
    }

    #[test]
    fn the_budget_bounds_even_pathological_names() {
        assert!(super::super::display_columns(&fit_name(&"あ".repeat(60)).0) <= NAME_BUDGET);
    }

    #[test]
    fn badges_fill_left_to_right_then_downward() {
        let slots: Vec<_> = badge_slots(badges::MAX_EQUIPPED).collect();
        let rows: Vec<_> = slots.chunks(BADGE_COLUMNS).collect();

        for (i, row) in rows.iter().enumerate() {
            assert!(
                row[0].0 < row[1].0,
                "row {i}: badge {} should sit left of badge {}",
                i * BADGE_COLUMNS + 1,
                i * BADGE_COLUMNS + 2
            );
            assert_eq!(row[0].1, row[1].1, "row {i} should share one baseline");
        }

        for (i, pair) in rows.windows(2).enumerate() {
            assert!(
                pair[0][0].1 < pair[1][0].1,
                "row {i} should sit above row {}",
                i + 1
            );
        }
    }

    #[test]
    #[ignore = "diagnostic: render the rail at each badge count, at output scale"]
    fn badge_counts() {
        let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");
        let accent = Accent::default();
        let ids = [
            "veteran", "elder", "legend", "melody", "peak", "heart", "sun", "bloom",
        ];

        for count in [0, 1, 2, 3, 8] {
            let equipped: Vec<&'static Badge> = ids[..count]
                .iter()
                .map(|id| badges::find(id).expect("known badge"))
                .collect();

            let svg = svg(&Profile {
                name: "yuri",
                handle: "yuri",
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
                badges: &equipped,
                coins: 1_240,
                currency: "coins",
            });

            let png = super::super::render(&svg, WIDTH, HEIGHT, super::super::SUPERSAMPLE)
                .expect("render");
            std::fs::write(format!("{dir}/badges-{count}.png"), png).expect("write");
        }
    }

    #[test]
    fn the_badge_grid_stays_inside_its_rail() {
        assert_eq!(
            BADGE_COLUMNS * BADGE_ROWS,
            badges::MAX_EQUIPPED,
            "the grid must have a cell for every badge a user can equip"
        );

        let label_clearance = STRIP.y + BADGE_TOP;
        let full: Vec<_> = badge_slots(badges::MAX_EQUIPPED).collect();

        for count in 1..=badges::MAX_EQUIPPED {
            let slots: Vec<_> = badge_slots(count).collect();
            assert_eq!(slots.len(), count);

            for (i, (cx, cy)) in slots.iter().enumerate() {
                assert!(
                    cx - BADGE_R >= STRIP.x && cx + BADGE_R <= STRIP.x + STRIP.w,
                    "with {count} badges, badge {i} runs off the rail horizontally at {cx}"
                );
                assert!(
                    cy - BADGE_R >= label_clearance && cy + BADGE_R <= STRIP.bottom(),
                    "with {count} badges, badge {i} runs off the rail vertically at {cy}"
                );
            }

            assert_eq!(
                slots[0], full[0],
                "with {count} badges the first slot moved off the top of the grid"
            );
            assert_eq!(
                &slots[..],
                &full[..count],
                "with {count} badges the slots differ from a full grid's first {count}"
            );
        }
    }
}
