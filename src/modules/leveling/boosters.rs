use std::time::Duration;

macro_rules! icon {
    ($name:literal) => {
        include_str!(concat!("../../../assets/icons/", $name, ".svg"))
    };
}

pub const NORMAL_PCT: i64 = 100;
pub const MAX_PCT: i64 = 1_000;

pub struct Booster {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub colour: &'static str,
    pub multiplier_pct: i64,
    pub hours: i64,
    pub price: i64,
    pub description: &'static str,
}

impl Booster {
    pub fn duration(&self) -> Duration {
        Duration::from_secs((self.hours * 3_600) as u64)
    }

    pub fn label(&self) -> String {
        let whole = self.multiplier_pct / 100;
        let rest = self.multiplier_pct % 100;

        if rest == 0 {
            format!("{whole}x XP")
        } else {
            format!("{}.{:02}x XP", whole, rest)
        }
    }
}

pub const BOOSTERS: &[Booster] = &[
    Booster {
        id: "spark",
        icon: icon!("zap"),
        colour: "#facc15",
        name: "Spark",
        multiplier_pct: 150,
        hours: 1,
        price: 250,
        description: "Half again as much XP for an hour.",
    },
    Booster {
        id: "surge",
        icon: icon!("chevrons-up"),
        colour: "#38bdf8",
        name: "Surge",
        multiplier_pct: 200,
        hours: 1,
        price: 500,
        description: "Double XP for an hour.",
    },
    Booster {
        id: "overdrive",
        icon: icon!("gauge"),
        colour: "#fb7185",
        name: "Overdrive",
        multiplier_pct: 300,
        hours: 1,
        price: 1_200,
        description: "Triple XP for an hour. Spend it somewhere busy.",
    },
    Booster {
        id: "momentum",
        icon: icon!("timer"),
        colour: "#a78bfa",
        name: "Momentum",
        multiplier_pct: 200,
        hours: 24,
        price: 3_000,
        description: "Double XP for a full day.",
    },
];

pub fn find(id: &str) -> Option<&'static Booster> {
    BOOSTERS.iter().find(|booster| booster.id == id)
}

pub fn resolve(input: &str) -> Option<&'static Booster> {
    let needle = input.trim();

    BOOSTERS.iter().find(|booster| {
        booster.id.eq_ignore_ascii_case(needle) || booster.name.eq_ignore_ascii_case(needle)
    })
}

pub fn catalogue() -> impl Iterator<Item = &'static Booster> {
    let mut shop: Vec<&Booster> = BOOSTERS.iter().collect();
    shop.sort_by_key(|booster| booster.price);
    shop.into_iter()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = BOOSTERS.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate booster id");
    }

    #[test]
    fn every_booster_actually_boosts_and_fits_the_column() {
        for booster in BOOSTERS {
            assert!(
                booster.multiplier_pct > NORMAL_PCT,
                "{} does not boost anything",
                booster.id
            );
            assert!(
                booster.multiplier_pct <= MAX_PCT,
                "{} exceeds what the check constraint allows",
                booster.id
            );
            assert!(booster.hours > 0, "{} never expires", booster.id);
            assert!(booster.price > 0, "{} is free", booster.id);
        }
    }

    #[test]
    fn a_longer_or_stronger_booster_never_costs_less() {
        for a in BOOSTERS {
            for b in BOOSTERS {
                if a.multiplier_pct >= b.multiplier_pct && a.hours >= b.hours && a.id != b.id {
                    assert!(
                        a.price >= b.price,
                        "{} is at least as good as {} but cheaper",
                        a.id,
                        b.id
                    );
                }
            }
        }
    }

    #[test]
    fn resolving_accepts_an_id_or_a_name_in_any_case() {
        assert_eq!(resolve("SURGE").map(|b| b.id), Some("surge"));
        assert_eq!(resolve("  Overdrive ").map(|b| b.id), Some("overdrive"));
        assert!(resolve("nothing").is_none());
    }

    #[test]
    fn labels_read_as_multipliers() {
        assert_eq!(find("surge").expect("surge").label(), "2x XP");
        assert_eq!(find("spark").expect("spark").label(), "1.50x XP");
    }

    #[test]
    fn the_catalogue_is_sorted_by_price() {
        let prices: Vec<i64> = catalogue().map(|b| b.price).collect();
        let mut sorted = prices.clone();
        sorted.sort_unstable();
        assert_eq!(prices, sorted);
    }
}
