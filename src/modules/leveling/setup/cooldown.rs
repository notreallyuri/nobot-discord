use crate::data::MemberId;
use dashmap::{DashMap, Entry};
use std::time::{Duration, Instant};

pub fn claim_xp_slot(map: &DashMap<MemberId, Instant>, key: MemberId, cooldown: Duration) -> bool {
    let now = Instant::now();
    let until = now + cooldown;

    match map.entry(key) {
        Entry::Occupied(mut e) if now >= *e.get() => {
            e.insert(until);
            true
        }
        Entry::Occupied(_) => false,
        Entry::Vacant(e) => {
            e.insert(until);
            true
        }
    }
}

pub fn prune(map: &DashMap<MemberId, Instant>) {
    let now = Instant::now();
    map.retain(|_, &mut until| until > now);
}

#[cfg(test)]
mod tests {
    use super::*;

    const COOLDOWN: Duration = Duration::from_secs(30);

    fn member(user_id: i64) -> MemberId {
        MemberId {
            guild_id: 1,
            user_id,
        }
    }

    #[test]
    fn the_first_message_always_earns() {
        let map = DashMap::new();
        assert!(claim_xp_slot(&map, member(1), COOLDOWN));
    }

    #[test]
    fn a_second_message_inside_the_window_does_not() {
        let map = DashMap::new();
        assert!(claim_xp_slot(&map, member(1), COOLDOWN));
        assert!(!claim_xp_slot(&map, member(1), COOLDOWN));
    }

    #[test]
    fn a_zero_cooldown_never_blocks() {
        let map = DashMap::new();

        for _ in 0..5 {
            assert!(claim_xp_slot(&map, member(1), Duration::ZERO));
        }
    }

    #[test]
    fn members_do_not_share_a_cooldown() {
        let map = DashMap::new();
        assert!(claim_xp_slot(&map, member(1), COOLDOWN));
        assert!(claim_xp_slot(&map, member(2), COOLDOWN));
    }

    #[test]
    fn pruning_keeps_anyone_still_on_cooldown() {
        let map = DashMap::new();

        claim_xp_slot(&map, member(1), Duration::from_secs(600));
        claim_xp_slot(&map, member(2), Duration::ZERO);

        prune(&map);

        assert!(map.contains_key(&member(1)), "still cooling down");
        assert!(!map.contains_key(&member(2)), "expired");
    }

    #[test]
    fn pruning_a_long_cooldown_does_not_hand_back_an_early_claim() {
        let map = DashMap::new();
        let long = Duration::from_secs(86_400);

        assert!(claim_xp_slot(&map, member(1), long));
        prune(&map);

        assert!(
            map.contains_key(&member(1)),
            "a day-long cooldown must survive a prune"
        );
        assert!(!claim_xp_slot(&map, member(1), long));
    }
}
