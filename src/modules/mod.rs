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
            "shop buy",
            "shop list",
            "shuffle",
            "skip",
            "stop",
        ];

        assert_eq!(registered, expected);
    }
}

#[cfg(test)]
mod descriptions {
    use super::*;

    fn walk(command: &crate::Command, path: &str, missing: &mut Vec<String>) {
        let name = if path.is_empty() {
            command.name.clone()
        } else {
            format!("{path} {}", command.name)
        };

        match command.description.as_deref() {
            Some(text) if !text.trim().is_empty() => {}
            _ => missing.push(name.clone()),
        }

        for sub in &command.subcommands {
            walk(sub, &name, missing);
        }
    }

    #[test]
    fn every_command_describes_itself() {
        let mut missing = Vec::new();

        for command in all().iter().flat_map(|module| module.commands()) {
            walk(&command, "", &mut missing);
        }

        assert!(
            missing.is_empty(),
            "these would show a blank description in Discord: {missing:?}"
        );
    }

    #[test]
    fn descriptions_fit_discords_limit() {
        let mut offenders = Vec::new();

        fn check(command: &crate::Command, offenders: &mut Vec<String>) {
            if let Some(text) = command.description.as_deref()
                && text.chars().count() > 100
            {
                offenders.push(format!("{}: {} chars", command.name, text.chars().count()));
            }

            for sub in &command.subcommands {
                check(sub, offenders);
            }
        }

        for command in all().iter().flat_map(|module| module.commands()) {
            check(&command, &mut offenders);
        }

        assert!(
            offenders.is_empty(),
            "over Discord's 100-char cap: {offenders:?}"
        );
    }

    #[test]
    fn every_parameter_describes_itself() {
        let mut missing = Vec::new();

        fn check(command: &crate::Command, path: &str, missing: &mut Vec<String>) {
            let name = if path.is_empty() {
                command.name.clone()
            } else {
                format!("{path} {}", command.name)
            };

            for parameter in &command.parameters {
                match parameter.description.as_deref() {
                    Some(text) if !text.trim().is_empty() => {}
                    _ => missing.push(format!("{name} → {}", parameter.name)),
                }
            }

            for sub in &command.subcommands {
                check(sub, &name, missing);
            }
        }

        for command in all().iter().flat_map(|module| module.commands()) {
            check(&command, "", &mut missing);
        }

        assert!(
            missing.is_empty(),
            "parameters with no description: {missing:?}"
        );
    }
}
