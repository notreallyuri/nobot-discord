use super::accent::Accent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Glow,
    Aurora,
}

impl Effect {
    pub fn defs(self, accent: &Accent) -> String {
        match self {
            Effect::Glow => String::new(),
            Effect::Aurora => format!(
                r##"<radialGradient id="fx-aurora-a" cx="0%" cy="0%" r="88%">
                      <stop offset="0%" stop-color="{light}" stop-opacity="0.34"/>
                      <stop offset="100%" stop-color="{light}" stop-opacity="0"/>
                    </radialGradient>
                    <radialGradient id="fx-aurora-b" cx="100%" cy="100%" r="88%">
                      <stop offset="0%" stop-color="{base}" stop-opacity="0.30"/>
                      <stop offset="100%" stop-color="{base}" stop-opacity="0"/>
                    </radialGradient>"##,
                base = accent.base,
                light = accent.light,
            ),
        }
    }

    pub fn wash(self, width: u32, height: u32) -> String {
        match self {
            Effect::Glow => String::new(),
            Effect::Aurora => format!(
                r##"<rect width="{width}" height="{height}" fill="url(#fx-aurora-a)"/>
                    <rect width="{width}" height="{height}" fill="url(#fx-aurora-b)"/>"##
            ),
        }
    }

    pub fn rim(self, width: u32, height: u32) -> String {
        let passes: &[(f64, f64)] = match self {
            Effect::Glow => &[(14.0, 0.10), (8.0, 0.16), (4.0, 0.30), (1.6, 0.85)],
            Effect::Aurora => &[(6.0, 0.12), (1.4, 0.55)],
        };

        passes
            .iter()
            .map(|(stroke, opacity)| {
                let inset = stroke / 2.0;
                format!(
                    r##"<rect x="{inset:.2}" y="{inset:.2}" width="{w:.2}" height="{h:.2}"
                              rx="{r:.2}" fill="none" stroke="url(#accent)"
                              stroke-width="{stroke}" opacity="{opacity}"/>"##,
                    w = f64::from(width) - stroke,
                    h = f64::from(height) - stroke,
                    r = (26.0 - inset).max(0.0),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const ALL: [Effect; 2] = [Effect::Glow, Effect::Aurora];

    #[test]
    fn every_effect_draws_something() {
        let accent = Accent::default();

        for effect in ALL {
            let drawn = format!(
                "{}{}{}",
                effect.defs(&accent),
                effect.wash(560, 560),
                effect.rim(560, 560)
            );

            assert!(drawn.contains('<'), "{effect:?} draws nothing at all");
            assert!(
                !effect.rim(560, 560).is_empty(),
                "{effect:?} has no rim, so it would be invisible on a busy card"
            );
        }
    }

    #[test]
    fn a_wash_never_refers_to_a_gradient_it_did_not_define() {
        let accent = Accent::default();

        for effect in ALL {
            let defs = effect.defs(&accent);
            let body = format!("{}{}", effect.wash(560, 560), effect.rim(560, 560));

            for id in ["fx-aurora-a", "fx-aurora-b"] {
                if body.contains(&format!("url(#{id})")) {
                    assert!(
                        defs.contains(&format!(r#"id="{id}""#)),
                        "{effect:?} uses {id} without defining it"
                    );
                }
            }
        }
    }

    #[test]
    fn a_rim_stays_inside_the_canvas() {
        assert!(
            Effect::Glow.rim(560, 560).contains(r#"x="7.00""#),
            "the widest pass is not inset by half its stroke"
        );
    }
}
