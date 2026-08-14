use std::time::Duration;

pub const XP_COOLDOWN: Duration = Duration::from_secs(30);
pub const XP_PER_MESSAGE: i64 = 75;

pub fn level_for_xp(xp: i64) -> i64 {
    ((xp.max(0) as f64 / 100.0).sqrt()).floor() as i64
}

pub fn xp_for_level(level: i64) -> i64 {
    let level = level.max(0);
    100 * level * level
}

pub fn level_progress(xp: i64) -> (i64, i64) {
    let level = level_for_xp(xp);
    let start = xp_for_level(level);
    let end = xp_for_level(level + 1);

    (xp.max(0) - start, end - start)
}

pub fn leveled_up(before: i64, after: i64) -> Option<i64> {
    let new_level = level_for_xp(after);
    (level_for_xp(before) < new_level).then_some(new_level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_is_monotonic() {
        let mut last = 0;
        for xp in (0..100_000).step_by(137) {
            let level = level_for_xp(xp);
            assert!(level >= last, "level dropped at {xp} xp");
            last = level;
        }
    }

    #[test]
    fn xp_for_level_inverts_level_for_xp() {
        for level in 0..64 {
            let threshold = xp_for_level(level);
            assert_eq!(level_for_xp(threshold), level, "at level {level}");
            if level > 0 {
                assert_eq!(
                    level_for_xp(threshold - 1),
                    level - 1,
                    "one xp short of {level}"
                );
            }
        }
    }

    #[test]
    fn progress_spans_the_whole_level() {
        for level in 0..32 {
            let start = xp_for_level(level);
            let (earned, needed) = level_progress(start);
            assert_eq!(earned, 0, "level {level} should start empty");

            let (earned, needed_again) = level_progress(xp_for_level(level + 1) - 1);
            assert_eq!(needed, needed_again);
            assert_eq!(earned, needed - 1, "level {level} should end full");
        }
    }

    #[test]
    fn detects_only_real_crossings() {
        assert_eq!(leveled_up(99, 100), Some(1));
        assert_eq!(leveled_up(100, 150), None);
        assert_eq!(leveled_up(0, 99), None);
        assert_eq!(leveled_up(0, 400), Some(2));
    }

    #[test]
    fn negative_xp_is_clamped() {
        assert_eq!(level_for_xp(-500), 0);
        assert_eq!(xp_for_level(-3), 0);
        let (earned, needed) = level_progress(-10);
        assert_eq!(earned, 0);
        assert!(needed > 0);
    }
}
