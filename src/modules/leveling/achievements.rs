pub const COINS_PER_TICK: i64 = 10;

pub struct Achievement {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub level: i64,
    pub coins: i64,
    pub badge: Option<&'static str>,
}

pub const ACHIEVEMENTS: &[Achievement] = &[
    Achievement {
        id: "newcomer",
        name: "Newcomer",
        description: "Reach level 1.",
        level: 1,
        coins: 100,
        badge: None,
    },
    Achievement {
        id: "regular",
        name: "Regular",
        description: "Reach level 5.",
        level: 5,
        coins: 250,
        badge: None,
    },
    Achievement {
        id: "veteran",
        name: "Veteran",
        description: "Reach level 10.",
        level: 10,
        coins: 500,
        badge: Some("veteran"),
    },
    Achievement {
        id: "elder",
        name: "Elder",
        description: "Reach level 25.",
        level: 25,
        coins: 1_500,
        badge: Some("elder"),
    },
    Achievement {
        id: "legend",
        name: "Legend",
        description: "Reach level 50.",
        level: 50,
        coins: 5_000,
        badge: Some("legend"),
    },
];

pub fn find(id: &str) -> Option<&'static Achievement> {
    ACHIEVEMENTS.iter().find(|a| a.id == id)
}

pub fn activity_coins(experience: i64) -> i64 {
    let ticks = experience.max(0) / super::setup::xp::XP_PER_MESSAGE;
    ticks * COINS_PER_TICK
}

pub fn earned_at(level: i64) -> impl Iterator<Item = &'static Achievement> {
    ACHIEVEMENTS.iter().filter(move |a| a.level <= level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::leveling::badges;

    #[test]
    fn ids_are_unique_and_sorted_by_level() {
        let mut ids: Vec<_> = ACHIEVEMENTS.iter().map(|a| a.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate achievement id");

        let levels: Vec<i64> = ACHIEVEMENTS.iter().map(|a| a.level).collect();
        let mut sorted = levels.clone();
        sorted.sort_unstable();
        assert_eq!(levels, sorted, "achievements should read in level order");
    }

    #[test]
    fn every_granted_badge_exists_and_is_not_purchasable() {
        for achievement in ACHIEVEMENTS {
            let Some(id) = achievement.badge else {
                continue;
            };

            let badge = badges::find(id)
                .unwrap_or_else(|| panic!("{} grants unknown badge {id}", achievement.id));

            assert!(
                badge.price.is_none(),
                "{id} is granted by an achievement but also purchasable"
            );
        }
    }

    #[test]
    fn earned_at_is_cumulative() {
        assert_eq!(earned_at(0).count(), 0);
        assert_eq!(earned_at(1).count(), 1);
        assert_eq!(earned_at(9).count(), 2);
        assert_eq!(earned_at(10).count(), 3);
        assert_eq!(earned_at(100).count(), ACHIEVEMENTS.len());
    }

    #[test]
    fn rewards_grow_with_level() {
        let coins: Vec<i64> = ACHIEVEMENTS.iter().map(|a| a.coins).collect();
        let mut sorted = coins.clone();
        sorted.sort_unstable();
        assert_eq!(coins, sorted, "later milestones should pay more");
    }
}
