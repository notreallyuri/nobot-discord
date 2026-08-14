use super::{FONT_FAMILY, emblem::Emblem, escape, truncate};

const CELL: f64 = 96.0;
const PLATE_R: f64 = 30.0;
const PLATE_Y: f64 = 40.0;
const LABEL_Y: f64 = 88.0;
const HEIGHT: f64 = 104.0;
const LABEL_SIZE: f64 = 12.0;
const LABEL_BUDGET: usize = 13;

const INK: &str = "#c8d0e2";
/// Discord embeds sit on a light ground in light mode, so the strip carries its
/// own dark panel rather than relying on the theme behind it.
const GROUND: &str = "#1a1d27";

pub struct Cell<'a> {
    pub emblem: Emblem<'a>,
    pub label: &'a str,
}

pub fn size(count: usize) -> (u32, u32) {
    ((CELL * count.max(1) as f64) as u32, HEIGHT as u32)
}

pub fn svg(cells: &[Cell<'_>]) -> String {
    let (width, height) = size(cells.len());

    let body: String = cells
        .iter()
        .enumerate()
        .map(|(i, cell)| {
            let cx = CELL * (i as f64) + CELL / 2.0;
            let plate = super::emblem::render(&cell.emblem, i, cx, PLATE_Y, PLATE_R);
            let label = escape(&truncate(cell.label, LABEL_BUDGET));

            format!(
                r##"{plate}
                    <text x="{cx:.1}" y="{LABEL_Y}" font-family="{FONT_FAMILY}"
                          font-size="{LABEL_SIZE}" fill="{INK}" text-anchor="middle">{label}</text>"##
            )
        })
        .collect();

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg"
                 width="{width}" height="{height}" viewBox="0 0 {width} {height}">
             <rect width="{width}" height="{height}" rx="16" fill="{GROUND}"/>
             {body}</svg>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: &str = r#"<svg viewBox="0 0 24 24"><path d="M4 12 L12 4 L20 12 Z"/></svg>"#;

    fn cells(count: usize) -> Vec<Cell<'static>> {
        (0..count)
            .map(|_| Cell {
                emblem: Emblem {
                    icon: ICON,
                    colour: "#fbbf24",
                },
                label: "Sovereign",
            })
            .collect()
    }

    #[test]
    fn a_strip_widens_with_its_cells() {
        assert_eq!(size(1).0, CELL as u32);
        assert_eq!(size(6).0, (CELL * 6.0) as u32);
        assert_eq!(size(6).1, HEIGHT as u32);
    }

    #[test]
    fn every_cell_gets_its_own_gradient_ids() {
        let svg = svg(&cells(3));

        for i in 0..3 {
            assert!(
                svg.contains(&format!(r#"id="plate{i}""#)),
                "missing plate{i}"
            );
        }
    }

    #[test]
    fn a_long_label_is_truncated_rather_than_overrunning_its_cell() {
        let svg = svg(&[Cell {
            emblem: Emblem {
                icon: ICON,
                colour: "#fbbf24",
            },
            label: "An Extremely Long Badge Name",
        }]);

        assert!(svg.contains('…'), "expected the label to be cut short");
    }

    #[test]
    fn a_label_with_markup_is_escaped() {
        let svg = svg(&[Cell {
            emblem: Emblem {
                icon: ICON,
                colour: "#fbbf24",
            },
            label: "<b>x",
        }]);

        assert!(!svg.contains("<b>"), "markup survived into the strip");
    }

    #[test]
    fn a_strip_rasterises() {
        let (width, height) = size(6);
        let png = super::super::render(&svg(&cells(6)), width, height, 1).expect("render");

        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert!(png.len() > 1_000, "strip looks blank ({} bytes)", png.len());
    }

    #[test]
    fn an_empty_strip_still_has_a_valid_size() {
        assert_eq!(size(0).0, CELL as u32);
        let (width, height) = size(0);
        assert!(super::super::render(&svg(&[]), width, height, 1).is_ok());
    }
}

#[cfg(test)]
mod preview {
    use super::*;
    use crate::modules::leveling::{badges, boosters};

    #[test]
    #[ignore = "diagnostic: render a shop shelf"]
    fn shelves() {
        let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");

        let badge_cells: Vec<Cell<'_>> = badges::purchasable()
            .take(6)
            .map(|badge| Cell {
                emblem: Emblem {
                    icon: badge.icon,
                    colour: badge.colour,
                },
                label: badge.name,
            })
            .collect();

        let boost_cells: Vec<Cell<'_>> = boosters::catalogue()
            .map(|booster| Cell {
                emblem: Emblem {
                    icon: booster.icon,
                    colour: booster.colour,
                },
                label: booster.name,
            })
            .collect();

        for (name, cells) in [("badges", badge_cells), ("boosters", boost_cells)] {
            let (width, height) = size(cells.len());
            let png = super::super::render(&svg(&cells), width, height, super::super::SUPERSAMPLE)
                .expect("render");
            std::fs::write(format!("{dir}/shelf-{name}.png"), png).expect("write");
        }
    }
}
