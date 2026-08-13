use super::{FONT_FAMILY, accent::Accent, escape, truncate};

pub const WIDTH: u32 = 660;
pub const HEIGHT: u32 = 200;

const AVATAR_R: f64 = 56.0;
const AVATAR_CX: f64 = 108.0;
const AVATAR_CY: f64 = HEIGHT as f64 / 2.0;
const TEXT_X: f64 = AVATAR_CX + AVATAR_R + 34.0;

pub struct Welcome<'a> {
    pub name: &'a str,
    pub server: &'a str,
    pub accent: &'a Accent,
    pub avatar: Option<&'a str>,
    pub member_number: u64,
    pub leaving: bool,
}

pub fn svg(card: &Welcome<'_>) -> String {
    let name = escape(&truncate(card.name, 22));
    let server = escape(&truncate(card.server, 30));
    let initial = escape(
        &card
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string()),
    );

    let eyebrow = if card.leaving { "FAREWELL" } else { "WELCOME" };
    let footer = if card.leaving {
        format!("{server} · {} members left", card.member_number)
    } else {
        format!("{server} · member #{}", card.member_number)
    };

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
            r##"<circle cx="{AVATAR_CX}" cy="{AVATAR_CY}" r="{AVATAR_R}" fill="#2b3145"/>
                <text x="{AVATAR_CX}" y="{AVATAR_CY}" font-family="{FONT_FAMILY}"
                      font-size="48" font-weight="bold" fill="#8b93a8"
                      text-anchor="middle" dominant-baseline="central">{initial}</text>"##
        ),
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
    <clipPath id="card-clip"><rect width="{WIDTH}" height="{HEIGHT}" rx="24"/></clipPath>
    <clipPath id="avatar-clip">
      <circle cx="{AVATAR_CX}" cy="{AVATAR_CY}" r="{AVATAR_R}"/>
    </clipPath>
  </defs>

  <rect width="{WIDTH}" height="{HEIGHT}" rx="24" fill="url(#bg)"/>
  <g clip-path="url(#card-clip)">
    <circle cx="{glow_x}" cy="-40" r="150" fill="{accent_base}" opacity="0.14"/>
    <rect x="0" y="0" width="4" height="{HEIGHT}" fill="url(#accent)"/>
  </g>

  {avatar}
  <circle cx="{AVATAR_CX}" cy="{AVATAR_CY}" r="{ring}" fill="none" stroke="url(#accent)"
          stroke-width="3"/>

  <text x="{TEXT_X}" y="66" font-family="{FONT_FAMILY}" font-size="13" font-weight="bold"
        fill="{accent_light}" letter-spacing="3">{eyebrow}</text>
  <text x="{TEXT_X}" y="112" font-family="{FONT_FAMILY}" font-size="34" font-weight="bold"
        fill="#f4f6fb">{name}</text>
  <text x="{TEXT_X}" y="142" font-family="{FONT_FAMILY}" font-size="15" fill="#9aa4bd">
    {footer}
  </text>
</svg>"##,
        accent_base = card.accent.base,
        accent_light = card.accent.light,
        ring = AVATAR_R + 3.0,
        glow_x = WIDTH as f64 - 60.0,
    )
}

#[cfg(test)]
mod tests {
    use super::super::render;
    use super::*;

    fn card(leaving: bool) -> String {
        let accent = Accent::default();

        svg(&Welcome {
            name: "yuri",
            server: "The Server",
            accent: &accent,
            avatar: None,
            member_number: 42,
            leaving,
        })
    }

    #[test]
    fn both_greetings_rasterise() {
        for leaving in [false, true] {
            let png = render(&card(leaving), WIDTH, HEIGHT).expect("should rasterise");

            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            assert!(png.len() > 2_000, "looks blank ({} bytes)", png.len());
        }
    }

    #[test]
    fn the_wording_matches_the_direction() {
        assert!(card(false).contains("WELCOME"));
        assert!(card(false).contains("member #42"));

        assert!(card(true).contains("FAREWELL"));
        assert!(card(true).contains("42 members left"));
    }

    #[test]
    fn markup_in_a_name_is_escaped() {
        let accent = Accent::default();
        let svg = svg(&Welcome {
            name: "<script>x</script>",
            server: "a & b",
            accent: &accent,
            avatar: None,
            member_number: 1,
            leaving: false,
        });

        assert!(!svg.contains("<script>"));
        assert!(svg.contains("&amp;"));
    }
}
