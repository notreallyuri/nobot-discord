use dashmap::DashMap;
use std::sync::Arc;

pub type Modes = Arc<DashMap<i64, Mode>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, poise::ChoiceParameter)]
pub enum Mode {
    #[name = "off"]
    #[default]
    Off,
    #[name = "track"]
    Track,
    #[name = "queue"]
    Queue,
}

impl Mode {
    pub fn describe(self) -> &'static str {
        match self {
            Self::Off => "Repeat is off — the queue plays through once.",
            Self::Track => {
                "Repeating the current track. The rest of the queue stays put and \
                            resumes when you turn this off."
            }
            Self::Queue => "Repeating the whole queue — finished tracks go back on the end.",
        }
    }
}

pub fn get(modes: &Modes, guild_id: i64) -> Mode {
    modes.get(&guild_id).map(|mode| *mode).unwrap_or_default()
}

pub fn set(modes: &Modes, guild_id: i64, mode: Mode) {
    if mode == Mode::Off {
        modes.remove(&guild_id);
    } else {
        modes.insert(guild_id, mode);
    }
}

pub fn shuffle<T>(queue: &mut std::collections::VecDeque<T>) -> usize {
    use rand::seq::SliceRandom;

    if queue.len() <= 2 {
        return 0;
    }

    queue.make_contiguous()[1..].shuffle(&mut rand::rng());
    queue.len() - 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn off_is_the_default_and_stores_nothing() {
        let modes: Modes = Arc::new(DashMap::new());

        assert_eq!(get(&modes, 1), Mode::Off);

        set(&modes, 1, Mode::Track);
        assert_eq!(get(&modes, 1), Mode::Track);

        set(&modes, 1, Mode::Off);
        assert_eq!(get(&modes, 1), Mode::Off);
        assert!(modes.is_empty(), "Off should not keep an entry around");
    }

    #[test]
    fn guilds_do_not_share_a_mode() {
        let modes: Modes = Arc::new(DashMap::new());

        set(&modes, 1, Mode::Queue);
        assert_eq!(get(&modes, 2), Mode::Off);
    }

    #[test]
    fn shuffling_never_disturbs_the_playing_track() {
        for _ in 0..50 {
            let mut queue: VecDeque<u32> = (0..12).collect();
            shuffle(&mut queue);
            assert_eq!(queue.front(), Some(&0), "the head must keep playing");
        }
    }

    #[test]
    fn shuffling_keeps_every_track() {
        let mut queue: VecDeque<u32> = (0..12).collect();
        let moved = shuffle(&mut queue);

        assert_eq!(moved, 11);

        let mut seen: Vec<u32> = queue.into_iter().collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn a_queue_too_short_to_shuffle_is_left_alone() {
        for size in 0..=2 {
            let mut queue: VecDeque<u32> = (0..size).collect();
            let before = queue.clone();

            assert_eq!(shuffle(&mut queue), 0);
            assert_eq!(queue, before);
        }
    }

    #[test]
    fn shuffling_actually_reorders() {
        let ordered: VecDeque<u32> = (0..30).collect();
        let changed = (0..20).any(|_| {
            let mut queue = ordered.clone();
            shuffle(&mut queue);
            queue != ordered
        });

        assert!(changed, "20 shuffles of 30 tracks never changed the order");
    }
}
