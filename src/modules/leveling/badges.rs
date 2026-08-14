use crate::modules::leveling::card::accent::{self, Rgb};

pub const MAX_EQUIPPED: usize = 8;

pub struct Badge {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub colour: &'static str,
    pub price: Option<i64>,
    pub description: &'static str,
}

macro_rules! icon {
    ($name:literal) => {
        include_str!(concat!("../../../assets/icons/", $name, ".svg"))
    };
}

pub const BADGES: &[Badge] = &[
    Badge {
        id: "veteran",
        name: "Veteran",
        icon: icon!("star"),
        colour: "#fbbf24",
        price: None,
        description: "Reach level 10.",
    },
    Badge {
        id: "elder",
        name: "Elder",
        icon: icon!("gem"),
        colour: "#38bdf8",
        price: None,
        description: "Reach level 25.",
    },
    Badge {
        id: "legend",
        name: "Legend",
        icon: icon!("infinity"),
        colour: "#f472b6",
        price: None,
        description: "Reach level 50.",
    },
    // Bought from the shop.
    Badge {
        id: "dot",
        name: "Full Stop",
        icon: icon!("circle-dot"),
        colour: "#94a3b8",
        price: Some(150),
        description: "Understated.",
    },
    Badge {
        id: "melody",
        name: "Melody",
        icon: icon!("music"),
        colour: "#a78bfa",
        price: Some(300),
        description: "For the ones who keep the queue full.",
    },
    Badge {
        id: "peak",
        name: "Peak",
        icon: icon!("mountain"),
        colour: "#34d399",
        price: Some(400),
        description: "Always climbing.",
    },
    Badge {
        id: "heart",
        name: "Heart",
        icon: icon!("heart"),
        colour: "#fb7185",
        price: Some(500),
        description: "Beloved by the server.",
    },
    Badge {
        id: "sun",
        name: "Daybreak",
        icon: icon!("sun"),
        colour: "#fcd34d",
        price: Some(600),
        description: "First one online, every time.",
    },
    Badge {
        id: "bloom",
        name: "Bloom",
        icon: icon!("flower"),
        colour: "#f9a8d4",
        price: Some(750),
        description: "Grew on everyone.",
    },
    Badge {
        id: "command",
        name: "Operator",
        icon: icon!("command"),
        colour: "#60a5fa",
        price: Some(1_000),
        description: "Runs the place.",
    },
];

pub fn find(id: &str) -> Option<&'static Badge> {
    BADGES.iter().find(|badge| badge.id == id)
}

pub fn resolve(input: &str) -> Option<&'static Badge> {
    let needle = input.trim();

    BADGES.iter().find(|badge| {
        badge.id.eq_ignore_ascii_case(needle) || badge.name.eq_ignore_ascii_case(needle)
    })
}

pub fn purchasable() -> impl Iterator<Item = &'static Badge> {
    let mut shop: Vec<&Badge> = BADGES.iter().filter(|b| b.price.is_some()).collect();
    shop.sort_by_key(|b| b.price);
    shop.into_iter()
}

pub fn icon_body(svg: &str) -> &str {
    let opening_end = svg
        .find("<svg")
        .and_then(|start| svg[start..].find('>').map(|offset| start + offset + 1));
    let closing = svg.rfind("</svg>");

    match (opening_end, closing) {
        (Some(start), Some(end)) if start <= end => svg[start..end].trim(),
        _ => "",
    }
}

const PLATE: Rgb = Rgb(0x24, 0x2a, 0x3b);

pub fn render(badge: &Badge, index: usize, cx: f64, cy: f64, radius: f64) -> String {
    let colour = accent::parse(badge.colour).unwrap_or(accent::DEFAULT);

    let lit = colour.mix(PLATE, 0.62);
    let shadow = PLATE.mix(Rgb(0, 0, 0), 0.45);
    let icon = colour.lighten(0.10);

    let size = radius * 1.06;
    let scale = size / 24.0;
    let (x, y) = (cx - size / 2.0, cy - size / 2.0);

    format!(
        r##"<radialGradient id="plate{index}" cx="34%" cy="26%" r="76%">
              <stop offset="0%" stop-color="{lit}"/>
              <stop offset="100%" stop-color="{shadow}"/>
            </radialGradient>
            <radialGradient id="shine{index}" cx="32%" cy="20%" r="52%">
              <stop offset="0%" stop-color="#ffffff" stop-opacity="0.26"/>
              <stop offset="100%" stop-color="#ffffff" stop-opacity="0"/>
            </radialGradient>
            <circle cx="{cx:.1}" cy="{cy:.1}" r="{radius:.1}" fill="url(#plate{index})"/>
            <circle cx="{cx:.1}" cy="{cy:.1}" r="{radius:.1}" fill="url(#shine{index})"/>
            <circle cx="{cx:.1}" cy="{cy:.1}" r="{radius:.1}" fill="none"
                    stroke="{ring}" stroke-width="1.5" opacity="0.55"/>
            <g transform="translate({x:.2} {y:.2}) scale({scale:.4})" fill="none"
               stroke="{icon}" stroke-width="2" stroke-linecap="round"
               stroke-linejoin="round">{body}</g>"##,
        lit = lit.to_hex(),
        shadow = shadow.to_hex(),
        ring = colour.to_hex(),
        icon = icon.to_hex(),
        body = icon_body(badge.icon),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = BADGES.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate badge id");
    }

    #[test]
    fn every_icon_yields_drawable_content() {
        for badge in BADGES {
            let body = icon_body(badge.icon);

            assert!(!body.is_empty(), "{} has an empty icon", badge.id);
            // A nested <svg> would carry its own sizing and break placement.
            assert!(!body.contains("<svg"), "{} kept its wrapper", badge.id);
            assert!(
                body.contains("<path") || body.contains("<circle"),
                "{} has no drawable elements",
                badge.id
            );
        }
    }

    #[test]
    fn icon_body_survives_malformed_input() {
        assert_eq!(icon_body(""), "");
        assert_eq!(icon_body("<svg>"), "");
        assert_eq!(icon_body("not svg at all"), "");
        assert_eq!(
            icon_body("<svg foo><path d=\"M0 0\"/></svg>"),
            "<path d=\"M0 0\"/>"
        );
    }

    #[test]
    fn colours_are_valid_hex() {
        for badge in BADGES {
            assert!(
                crate::modules::leveling::card::accent::parse(badge.colour).is_ok(),
                "{} has an invalid colour",
                badge.id
            );
        }
    }

    #[test]
    fn rendering_centres_the_plate_and_its_icon() {
        let badge = find("veteran").expect("veteran exists");
        let svg = render(badge, 0, 100.0, 50.0, 24.0);

        assert!(
            svg.contains(r#"cx="100.0" cy="50.0" r="24.0""#),
            "got: {svg}"
        );
        assert!(svg.contains("translate(87.28 37.28)"), "got: {svg}");
        assert!(
            svg.contains(badge.colour),
            "the ring keeps the badge's colour"
        );
    }

    #[test]
    fn gradient_ids_do_not_collide_between_badges() {
        let a = render(find("veteran").expect("veteran"), 0, 0.0, 0.0, 10.0);
        let b = render(find("melody").expect("melody"), 1, 0.0, 0.0, 10.0);

        assert!(a.contains(r#"id="plate0""#) && a.contains(r#"id="shine0""#));
        assert!(b.contains(r#"id="plate1""#) && b.contains(r#"id="shine1""#));
        assert!(!b.contains(r#"id="plate0""#));
    }

    #[test]
    fn shop_is_sorted_and_excludes_earned_badges() {
        let prices: Vec<i64> = purchasable()
            .map(|b| b.price.expect("purchasable"))
            .collect();
        let mut sorted = prices.clone();
        sorted.sort_unstable();
        assert_eq!(prices, sorted);

        assert!(!purchasable().any(|b| b.id == "veteran"));
    }
}
