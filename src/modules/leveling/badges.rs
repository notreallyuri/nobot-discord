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
        id: "evergreen",
        name: "Evergreen",
        icon: icon!("leaf"),
        colour: "#4ade80",
        price: Some(900),
        description: "Still here, season after season.",
    },
    Badge {
        id: "command",
        name: "Operator",
        icon: icon!("command"),
        colour: "#60a5fa",
        price: Some(1_000),
        description: "Runs the place.",
    },
    Badge {
        id: "crown",
        name: "Sovereign",
        icon: icon!("crown"),
        colour: "#eab308",
        price: Some(1_200),
        description: "Rules with a light touch.",
    },
    Badge {
        id: "quill",
        name: "Quill",
        icon: icon!("feather"),
        colour: "#a5b4fc",
        price: Some(1_500),
        description: "Says it better than the rest of us.",
    },
    Badge {
        id: "flame",
        name: "Wildfire",
        icon: icon!("flame"),
        colour: "#f97316",
        price: Some(1_800),
        description: "Impossible to ignore.",
    },
    Badge {
        id: "anchor",
        name: "Anchor",
        icon: icon!("anchor"),
        colour: "#0ea5e9",
        price: Some(2_000),
        description: "What the server steadies itself on.",
    },
    Badge {
        id: "eclipse",
        name: "Eclipse",
        icon: icon!("moon"),
        colour: "#22d3ee",
        price: Some(2_500),
        description: "Turns up rarely, and everyone stops to look.",
    },
    Badge {
        id: "frost",
        name: "Frostbite",
        icon: icon!("snowflake"),
        colour: "#7dd3fc",
        price: Some(3_000),
        description: "Unbothered, and a little cold about it.",
    },
    Badge {
        id: "radiance",
        name: "Radiance",
        icon: icon!("sparkles"),
        colour: "#c084fc",
        price: Some(3_500),
        description: "Hard to be near without noticing.",
    },
    Badge {
        id: "duelist",
        name: "Duelist",
        icon: icon!("swords"),
        colour: "#f87171",
        price: Some(4_200),
        description: "Has never once left an argument early.",
    },
    Badge {
        id: "ascent",
        name: "Ascent",
        icon: icon!("rocket"),
        colour: "#cbd5e1",
        price: Some(5_000),
        description: "Only ever pointed one way.",
    },
    Badge {
        id: "paragon",
        name: "Paragon",
        icon: icon!("trophy"),
        colour: "#f59e0b",
        price: Some(7_500),
        description: "There is no tier above this one.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{emblem::icon_body, profile};

    #[test]
    fn the_card_has_a_slot_for_every_badge_a_user_can_equip() {
        assert_eq!(MAX_EQUIPPED, profile::CAPACITY);
    }

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
            assert!(!body.contains("<svg"), "{} kept its wrapper", badge.id);
            assert!(
                [
                    "<path",
                    "<circle",
                    "<line",
                    "<polyline",
                    "<polygon",
                    "<rect",
                    "<ellipse"
                ]
                .iter()
                .any(|shape| body.contains(shape)),
                "{} has no drawable elements",
                badge.id
            );
        }
    }

    #[test]
    fn colours_are_valid_hex() {
        for badge in BADGES {
            assert!(
                crate::card::accent::parse(badge.colour).is_ok(),
                "{} has an invalid colour",
                badge.id
            );
        }
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

#[cfg(test)]
mod preview {
    use super::*;
    use crate::card::{self, emblem::Emblem, profile};

    #[test]
    #[ignore = "diagnostic: render the shop's badges onto a card"]
    fn shop_badges_on_a_card() {
        let dir = std::env::var("CARD_DUMP").expect("set CARD_DUMP");
        let accent = card::accent::Accent::default();

        let shown: Vec<&'static Badge> = match std::env::var("BADGE_IDS") {
            Ok(ids) => ids
                .split(',')
                .filter_map(|id| find(id.trim()))
                .take(profile::CAPACITY)
                .collect(),
            Err(_) => {
                let mut dearest: Vec<&'static Badge> = purchasable().collect();
                dearest.reverse();
                dearest.truncate(profile::CAPACITY);
                dearest
            }
        };

        for badge in &shown {
            println!(
                "{:>10}  {:>5} coins  {}",
                badge.name,
                badge.price.unwrap_or(0),
                badge.id
            );
        }

        let emblems: Vec<Emblem<'_>> = shown
            .iter()
            .map(|badge| Emblem {
                icon: badge.icon,
                colour: badge.colour,
            })
            .collect();

        let svg = profile::svg(&profile::Profile {
            name: "yuri",
            handle: "yuri",
            accent: &accent,
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
            badges: &emblems,
            coins: 17_700,
            currency: "coins",
        });

        let png =
            card::render(&svg, profile::WIDTH, profile::HEIGHT, card::SUPERSAMPLE).expect("render");
        std::fs::write(format!("{dir}/shop-badges.png"), png).expect("write");
    }
}
