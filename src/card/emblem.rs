use super::accent::{self, Rgb};

pub struct Emblem<'a> {
    pub icon: &'a str,
    pub colour: &'a str,
}

const PLATE: Rgb = Rgb(0x24, 0x2a, 0x3b);

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

pub fn render(emblem: &Emblem<'_>, index: usize, cx: f64, cy: f64, radius: f64) -> String {
    let colour = accent::parse(emblem.colour).unwrap_or(accent::DEFAULT);

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
        body = icon_body(emblem.icon),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: &str = r#"<svg viewBox="0 0 24 24"><path d="M2 2 L20 20"/></svg>"#;

    fn emblem() -> Emblem<'static> {
        Emblem {
            icon: ICON,
            colour: "#fbbf24",
        }
    }

    #[test]
    fn strips_the_wrapper_from_an_icon() {
        assert_eq!(icon_body(ICON), r#"<path d="M2 2 L20 20"/>"#);
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
    fn rendering_centres_the_plate_and_its_icon() {
        let svg = render(&emblem(), 0, 100.0, 50.0, 24.0);

        assert!(
            svg.contains(r#"cx="100.0" cy="50.0" r="24.0""#),
            "got: {svg}"
        );
        assert!(svg.contains("translate(87.28 37.28)"), "got: {svg}");
        assert!(
            svg.contains("#fbbf24"),
            "the ring keeps the emblem's colour"
        );
    }

    #[test]
    fn gradient_ids_do_not_collide() {
        let a = render(&emblem(), 0, 0.0, 0.0, 10.0);
        let b = render(&emblem(), 1, 0.0, 0.0, 10.0);

        assert!(a.contains(r#"id="plate0""#) && a.contains(r#"id="shine0""#));
        assert!(b.contains(r#"id="plate1""#) && b.contains(r#"id="shine1""#));
        assert!(!b.contains(r#"id="plate0""#));
    }

    #[test]
    fn an_unparseable_colour_falls_back_rather_than_panicking() {
        let svg = render(
            &Emblem {
                icon: ICON,
                colour: "not a colour",
            },
            0,
            0.0,
            0.0,
            10.0,
        );

        assert!(svg.contains(&accent::DEFAULT.to_hex()));
    }
}
