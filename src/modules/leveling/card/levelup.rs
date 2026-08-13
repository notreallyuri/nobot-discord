use super::{FONT_FAMILY, accent::Accent, escape, truncate};

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 48;

const NAME_BUDGET: usize = 18;

pub struct LevelUp<'a> {
    pub name: &'a str,
    pub accent: &'a Accent,
    pub avatar: Option<&'a str>,
    pub from: i64,
    pub to: i64,
}

pub fn svg(card: &LevelUp<'_>) -> String {
    let name = escape(&truncate(card.name, NAME_BUDGET));
    let initial = escape(
        &card
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".to_string()),
    );

    let level_size = match card.to.abs() {
        0..=99 => 16,
        100..=999 => 13,
        _ => 10,
    };

    let avatar = match card.avatar {
        Some(uri) => format!(
            r##"<image x="9" y="7" width="34" height="34"
                       clip-path="url(#avatar-clip)" href="{uri}"
                       preserveAspectRatio="xMidYMid slice"/>"##
        ),
        None => format!(
            r##"<circle cx="26" cy="24" r="17" fill="#2b3145"/>
                <text x="26" y="24" font-family="{FONT_FAMILY}" font-size="15"
                      font-weight="bold" fill="#8b93a8" text-anchor="middle"
                      dominant-baseline="central">{initial}</text>"##
        ),
    };

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg"
                 xmlns:xlink="http://www.w3.org/1999/xlink"
                 width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#171a24"/>
      <stop offset="100%" stop-color="#232839"/>
    </linearGradient>
    <linearGradient id="accent" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="{accent_light}"/>
      <stop offset="100%" stop-color="{accent_base}"/>
    </linearGradient>
    <radialGradient id="badge" cx="34%" cy="26%" r="78%">
      <stop offset="0%" stop-color="{accent_base}" stop-opacity="0.45"/>
      <stop offset="100%" stop-color="#1b2030" stop-opacity="1"/>
    </radialGradient>
    <clipPath id="avatar-clip">
      <circle cx="26" cy="24" r="17"/>
    </clipPath>
    <clipPath id="card-clip">
      <rect width="{WIDTH}" height="{HEIGHT}" rx="10"/>
    </clipPath>
  </defs>

  <rect width="{WIDTH}" height="{HEIGHT}" rx="10" fill="url(#bg)"/>
  <g clip-path="url(#card-clip)">
    <!-- Clipped, or these fill in the rounded corners. -->
    <circle cx="344" cy="2" r="46" fill="{accent_base}" opacity="0.10"/>
    <rect x="0" y="0" width="3" height="{HEIGHT}" fill="url(#accent)"/>
  </g>

  {avatar}
  <circle cx="26" cy="24" r="18" fill="none" stroke="url(#accent)" stroke-width="2"/>

  <text x="54" y="13" font-family="{FONT_FAMILY}" font-size="8" font-weight="bold"
        fill="{accent_light}" letter-spacing="1.6">LEVEL UP</text>
  <text x="54" y="29" font-family="{FONT_FAMILY}" font-size="14" font-weight="bold"
        fill="#f4f6fb">{name}</text>
  <text x="54" y="41" font-family="{FONT_FAMILY}" font-size="10" fill="#9aa4bd">
    Level {from} &#8594; {to}
  </text>

  <circle cx="352" cy="24" r="18" fill="url(#badge)"/>
  <circle cx="352" cy="24" r="18" fill="none" stroke="url(#accent)" stroke-width="2"/>
  <text x="352" y="24" font-family="{FONT_FAMILY}" font-size="{level_size}" font-weight="bold"
        fill="#f4f6fb" text-anchor="middle" dominant-baseline="central">{to}</text>
</svg>"##,
        from = card.from,
        to = card.to,
        accent_base = card.accent.base,
        accent_light = card.accent.light,
    )
}
