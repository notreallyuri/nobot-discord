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
                body.contains("<path") || body.contains("<circle"),
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
