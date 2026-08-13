use super::store::{Choice, Menu};
use poise::serenity_prelude as serenity;

pub const PREFIX: &str = "rolemenu:";

pub fn custom_id(menu_id: i64) -> String {
    format!("{PREFIX}{menu_id}")
}

pub fn menu_id_from(custom_id: &str) -> Option<i64> {
    custom_id.strip_prefix(PREFIX)?.parse().ok()
}

pub fn embed(menu: &Menu, choices: &[Choice]) -> serenity::CreateEmbed {
    let listing = if choices.is_empty() {
        "No roles yet — an admin can add some with `/rolemenu add`.".to_string()
    } else {
        choices
            .iter()
            .map(|choice| match &choice.description {
                Some(note) => format!("<@&{}> — {note}", choice.role_id),
                None => format!("<@&{}>", choice.role_id),
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut embed = serenity::CreateEmbed::new()
        .title(&menu.title)
        .field("Roles", listing, false);

    if let Some(description) = &menu.description {
        embed = embed.description(description);
    }

    if !choices.is_empty() {
        embed = embed.footer(serenity::CreateEmbedFooter::new(
            "Pick from the menu below. Deselecting removes the role.",
        ));
    }

    embed
}

pub fn components(menu: &Menu, choices: &[Choice]) -> Vec<serenity::CreateActionRow> {
    if choices.is_empty() {
        return Vec::new();
    }

    let options: Vec<serenity::CreateSelectMenuOption> = choices
        .iter()
        .map(|choice| {
            let mut option =
                serenity::CreateSelectMenuOption::new(&choice.label, choice.role_id.to_string());

            if let Some(description) = &choice.description {
                option = option.description(description);
            }

            option
        })
        .collect();

    let highest = choices.len().min(super::store::MAX_OPTIONS) as u8;

    let select = serenity::CreateSelectMenu::new(
        custom_id(menu.id),
        serenity::CreateSelectMenuKind::String { options },
    )
    .placeholder("Choose your roles")
    .min_values(0)
    .max_values(
        u8::try_from(menu.max_choices)
            .unwrap_or(highest)
            .min(highest),
    );

    vec![serenity::CreateActionRow::SelectMenu(select)]
}

pub struct Applied {
    pub added: Vec<serenity::RoleId>,
    pub removed: Vec<serenity::RoleId>,
    pub blocked: Vec<serenity::RoleId>,
}

impl Applied {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();

        let list = |roles: &[serenity::RoleId]| {
            roles
                .iter()
                .map(|role| format!("<@&{role}>"))
                .collect::<Vec<_>>()
                .join(", ")
        };

        if !self.added.is_empty() {
            parts.push(format!("Added {}", list(&self.added)));
        }

        if !self.removed.is_empty() {
            parts.push(format!("Removed {}", list(&self.removed)));
        }

        if !self.blocked.is_empty() {
            parts.push(format!(
                "Couldn't change {} — my highest role isn't above it",
                list(&self.blocked)
            ));
        }

        if parts.is_empty() {
            "Nothing changed.".to_string()
        } else {
            parts.join("\n")
        }
    }
}

pub fn plan(
    offered: &[serenity::RoleId],
    chosen: &[serenity::RoleId],
    held: &[serenity::RoleId],
) -> (Vec<serenity::RoleId>, Vec<serenity::RoleId>) {
    let add = chosen
        .iter()
        .filter(|role| offered.contains(role) && !held.contains(role))
        .copied()
        .collect();

    let remove = offered
        .iter()
        .filter(|role| !chosen.contains(role) && held.contains(role))
        .copied()
        .collect();

    (add, remove)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn role(id: u64) -> serenity::RoleId {
        serenity::RoleId::new(id)
    }

    #[test]
    fn custom_ids_round_trip() {
        assert_eq!(menu_id_from(&custom_id(42)), Some(42));
        assert_eq!(menu_id_from(&custom_id(i64::MAX)), Some(i64::MAX));
    }

    #[test]
    fn unrelated_components_are_ignored() {
        for id in [
            "",
            "rolemenu:",
            "rolemenu:abc",
            "other:12",
            "12",
            "rolemenu",
        ] {
            assert_eq!(menu_id_from(id), None, "should have ignored {id:?}");
        }
    }

    #[test]
    fn picking_adds_only_what_is_missing() {
        let offered = [role(1), role(2), role(3)];
        let (add, remove) = plan(&offered, &[role(1), role(2)], &[role(2)]);

        assert_eq!(add, [role(1)], "2 is already held");
        assert!(remove.is_empty());
    }

    #[test]
    fn deselecting_removes_the_role() {
        let offered = [role(1), role(2)];
        let (add, remove) = plan(&offered, &[role(1)], &[role(1), role(2)]);

        assert!(add.is_empty());
        assert_eq!(remove, [role(2)]);
    }

    #[test]
    fn selecting_nothing_clears_the_menus_roles() {
        let offered = [role(1), role(2)];
        let (add, remove) = plan(&offered, &[], &[role(1), role(2)]);

        assert!(add.is_empty());
        assert_eq!(remove, [role(1), role(2)]);
    }

    #[test]
    fn roles_outside_the_menu_are_never_touched() {
        let offered = [role(1)];
        let held = [role(1), role(99)];

        let (add, remove) = plan(&offered, &[], &held);
        assert!(add.is_empty());
        assert_eq!(remove, [role(1)], "99 is not this menu's business");

        let (add, remove) = plan(&offered, &[role(1), role(99)], &held);
        assert!(
            add.is_empty() && remove.is_empty(),
            "a role not offered here must not be granted"
        );
    }

    #[test]
    fn a_summary_reads_sensibly_when_nothing_happened() {
        let applied = Applied {
            added: Vec::new(),
            removed: Vec::new(),
            blocked: Vec::new(),
        };

        assert_eq!(applied.summary(), "Nothing changed.");
    }
}
