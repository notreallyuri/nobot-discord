use crate::card::{
    accent::{self, Accent},
    effect::Effect,
};

macro_rules! icon {
    ($name:literal) => {
        include_str!(concat!("../../../assets/icons/", $name, ".svg"))
    };
}

pub enum CosmeticType {
    Color {
        base: &'static str,
        light: &'static str,
    },
    CardEffect(Effect),
}

pub struct Cosmetic {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub cosmetic_type: CosmeticType,
    pub price: Option<i64>,
    pub icon: &'static str,
    pub colour: &'static str,
}

impl Cosmetic {
    pub fn slot(&self) -> Slot {
        match self.cosmetic_type {
            CosmeticType::Color { .. } => Slot::Accent,
            CosmeticType::CardEffect(_) => Slot::CardEffect,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Accent,
    CardEffect,
}

impl Slot {
    pub fn label(self) -> &'static str {
        match self {
            Slot::Accent => "accent",
            Slot::CardEffect => "card effect",
        }
    }
}

pub const COSMETICS: &[Cosmetic] = &[
    Cosmetic {
        id: "color_ember",
        name: "Ember",
        description: "Orange falling into gold.",
        cosmetic_type: CosmeticType::Color {
            base: "#f97316",
            light: "#fbbf24",
        },
        price: Some(300),
        icon: icon!("flame"),
        colour: "#f97316",
    },
    Cosmetic {
        id: "color_neon",
        name: "Neon",
        description: "Cyan into violet. Loud on purpose.",
        cosmetic_type: CosmeticType::Color {
            base: "#22d3ee",
            light: "#a78bfa",
        },
        price: Some(500),
        icon: icon!("zap"),
        colour: "#22d3ee",
    },
    Cosmetic {
        id: "color_tidal",
        name: "Tidal",
        description: "Deep water shading into green.",
        cosmetic_type: CosmeticType::Color {
            base: "#0ea5e9",
            light: "#34d399",
        },
        price: Some(700),
        icon: icon!("anchor"),
        colour: "#0ea5e9",
    },
    Cosmetic {
        id: "color_orchid",
        name: "Orchid",
        description: "Magenta cooling to rose.",
        cosmetic_type: CosmeticType::Color {
            base: "#d946ef",
            light: "#fb7185",
        },
        price: Some(900),
        icon: icon!("flower"),
        colour: "#d946ef",
    },
    Cosmetic {
        id: "color_solstice",
        name: "Solstice",
        description: "The longest day of the year, on a card.",
        cosmetic_type: CosmeticType::Color {
            base: "#f43f5e",
            light: "#fcd34d",
        },
        price: Some(1_200),
        icon: icon!("sun"),
        colour: "#f43f5e",
    },
    Cosmetic {
        id: "card_glow",
        name: "Glow",
        description: "Your accent lights the edge of the whole card.",
        cosmetic_type: CosmeticType::CardEffect(Effect::Glow),
        price: Some(1_500),
        icon: icon!("sparkles"),
        colour: "#c084fc",
    },
    Cosmetic {
        id: "card_aurora",
        name: "Aurora",
        description: "Accent light bleeding in from opposite corners.",
        cosmetic_type: CosmeticType::CardEffect(Effect::Aurora),
        price: Some(2_500),
        icon: icon!("moon"),
        colour: "#38bdf8",
    },
];

pub fn find(id: &str) -> Option<&'static Cosmetic> {
    COSMETICS.iter().find(|cosmetic| cosmetic.id == id)
}

pub fn resolve(input: &str) -> Option<&'static Cosmetic> {
    let needle = input.trim();

    COSMETICS.iter().find(|cosmetic| {
        cosmetic.id.eq_ignore_ascii_case(needle) || cosmetic.name.eq_ignore_ascii_case(needle)
    })
}

pub fn purchasable() -> impl Iterator<Item = &'static Cosmetic> {
    let mut shop: Vec<&Cosmetic> = COSMETICS.iter().filter(|c| c.price.is_some()).collect();
    shop.sort_by_key(|c| c.price);
    shop.into_iter()
}

pub fn accent(equipped: Option<&str>, stored: Option<i32>) -> Accent {
    match equipped.and_then(find).map(|c| &c.cosmetic_type) {
        Some(CosmeticType::Color { base, light }) => Accent::pair(
            accent::parse(base).unwrap_or(accent::DEFAULT),
            accent::parse(light).unwrap_or(accent::DEFAULT),
        ),
        _ => Accent::from_stored(stored),
    }
}

pub fn effect(equipped: Option<&str>) -> Option<Effect> {
    match equipped.and_then(find).map(|c| &c.cosmetic_type) {
        Some(CosmeticType::CardEffect(effect)) => Some(*effect),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::emblem::icon_body;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = COSMETICS.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate cosmetic id");
    }

    #[test]
    fn no_name_or_id_is_shared_with_another_catalogue() {
        use crate::modules::leveling::{badges, boosters};

        for cosmetic in COSMETICS {
            for taken in badges::BADGES
                .iter()
                .map(|b| (b.id, b.name))
                .chain(boosters::BOOSTERS.iter().map(|b| (b.id, b.name)))
            {
                assert!(
                    !cosmetic.id.eq_ignore_ascii_case(taken.0),
                    "{} collides with an existing id",
                    cosmetic.id
                );
                assert!(
                    !cosmetic.name.eq_ignore_ascii_case(taken.1),
                    "{} collides with an existing name",
                    cosmetic.name
                );
            }
        }
    }

    #[test]
    fn every_colour_a_cosmetic_names_is_valid_hex() {
        for cosmetic in COSMETICS {
            assert!(
                accent::parse(cosmetic.colour).is_ok(),
                "{} has an invalid shelf colour",
                cosmetic.id
            );

            if let CosmeticType::Color { base, light } = cosmetic.cosmetic_type {
                assert!(accent::parse(base).is_ok(), "{} base", cosmetic.id);
                assert!(accent::parse(light).is_ok(), "{} light", cosmetic.id);
                assert_eq!(
                    cosmetic.colour, base,
                    "{}'s shelf plate does not show the colour it sells",
                    cosmetic.id
                );
            }
        }
    }

    #[test]
    fn no_gradient_needs_lifting_to_stay_legible() {
        for cosmetic in COSMETICS {
            if let CosmeticType::Color { base, light } = cosmetic.cosmetic_type {
                let resolved = Accent::pair(
                    accent::parse(base).expect("valid"),
                    accent::parse(light).expect("valid"),
                );

                assert!(
                    !resolved.adjusted,
                    "{} is too dark and gets lightened away from what it sells",
                    cosmetic.id
                );
            }
        }
    }

    #[test]
    fn every_icon_yields_drawable_content() {
        for cosmetic in COSMETICS {
            let body = icon_body(cosmetic.icon);

            assert!(!body.is_empty(), "{} has an empty icon", cosmetic.id);
            assert!(!body.contains("<svg"), "{} kept its wrapper", cosmetic.id);
        }
    }

    #[test]
    fn the_shop_is_sorted_by_price() {
        let prices: Vec<i64> = purchasable()
            .map(|c| c.price.expect("purchasable"))
            .collect();
        let mut sorted = prices.clone();
        sorted.sort_unstable();

        assert_eq!(prices, sorted);
        assert_eq!(prices.len(), COSMETICS.len(), "a cosmetic is unbuyable");
    }

    #[test]
    fn resolving_accepts_an_id_or_a_name_in_any_case() {
        assert_eq!(resolve("NEON").map(|c| c.id), Some("color_neon"));
        assert_eq!(resolve("  color_glow ").map(|c| c.id), None);
        assert_eq!(resolve("Glow").map(|c| c.id), Some("card_glow"));
        assert!(resolve("nothing").is_none());
    }

    #[test]
    fn a_cosmetic_lands_in_the_slot_its_kind_implies() {
        assert_eq!(find("color_neon").expect("neon").slot(), Slot::Accent);
        assert_eq!(find("card_glow").expect("glow").slot(), Slot::CardEffect);
    }

    #[test]
    fn an_equipped_gradient_beats_the_stored_colour() {
        let stored = Some(accent::Rgb(0x11, 0x22, 0x33).to_i32());

        let worn = accent(Some("color_neon"), stored);
        assert_eq!(worn.base, "#22d3ee");
        assert_eq!(worn.light, "#a78bfa");
    }

    #[test]
    fn taking_the_gradient_off_uncovers_the_colour_underneath() {
        let stored = Some(accent::Rgb(0x22, 0xcc, 0x88).to_i32());

        assert_eq!(accent(None, stored).base, "#22cc88");
    }

    #[test]
    fn an_unknown_id_falls_back_instead_of_breaking_a_card() {
        assert_eq!(
            accent(Some("color_retired"), None).base,
            Accent::default().base
        );
        assert!(effect(Some("card_retired")).is_none());
    }

    #[test]
    fn a_cosmetic_in_the_wrong_slot_is_ignored() {
        assert!(effect(Some("color_neon")).is_none());
        assert_eq!(accent(Some("card_glow"), None).base, Accent::default().base);
    }

    #[test]
    fn every_effect_cosmetic_resolves_to_its_effect() {
        assert_eq!(effect(Some("card_glow")), Some(Effect::Glow));
        assert_eq!(effect(Some("card_aurora")), Some(Effect::Aurora));
        assert_eq!(effect(None), None);
    }
}
