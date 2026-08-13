use crate::module::Module;

pub mod leveling;
pub mod voice;

pub fn all() -> Vec<Box<dyn Module>> {
    vec![
        Box::new(leveling::LevelingModule),
        Box::new(voice::VoiceModule),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_is_registered() {
        let mut registered: Vec<String> = all()
            .iter()
            .flat_map(|module| module.commands())
            .flat_map(|command| {
                let parent = command.name.clone();
                let subcommands: Vec<String> = command
                    .subcommands
                    .iter()
                    .map(|sub| format!("{parent} {}", sub.name))
                    .collect();

                if subcommands.is_empty() {
                    vec![parent]
                } else {
                    subcommands
                }
            })
            .collect();
        registered.sort();

        let expected = [
            "achievements",
            "background clear",
            "background set",
            "badges equip",
            "badges list",
            "badges unequip",
            "buy",
            "clear",
            "color clear",
            "color set",
            "config currency",
            "config economy",
            "config reset",
            "config show",
            "config xp",
            "leaderboard",
            "leave",
            "lyrics",
            "move",
            "nowplaying",
            "pause",
            "play",
            "profile",
            "queue",
            "remove",
            "resume",
            "shop",
            "skip",
            "stop",
        ];

        assert_eq!(registered, expected);
    }
}
