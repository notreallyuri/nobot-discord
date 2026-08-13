use crate::module::Module;

pub mod greetings;
pub mod leveling;
pub mod roles;
pub mod voice;

pub fn all() -> Vec<Box<dyn Module>> {
    vec![
        Box::new(leveling::LevelingModule),
        Box::new(voice::VoiceModule),
        Box::new(roles::RolesModule),
        Box::new(greetings::GreetingsModule),
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
            "autorole add",
            "autorole list",
            "autorole remove",
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
            "config dj",
            "config economy",
            "config farewell",
            "config reset",
            "config show",
            "config voice",
            "config welcome",
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
            "repeat",
            "resume",
            "rolemenu add",
            "rolemenu create",
            "rolemenu delete",
            "rolemenu list",
            "rolemenu remove",
            "shop",
            "shuffle",
            "skip",
            "stop",
        ];

        assert_eq!(registered, expected);
    }
}
