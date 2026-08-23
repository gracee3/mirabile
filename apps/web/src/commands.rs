#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandId {
    SaveDraft,
    CancelDraft,
    FocusChartRail,
    RefreshView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandMetadata {
    pub id: CommandId,
    pub label: &'static str,
    pub shortcut: &'static str,
    pub group: &'static str,
    pub keywords: &'static [&'static str],
}

const COMMANDS: [CommandMetadata; 4] = [
    CommandMetadata {
        id: CommandId::SaveDraft,
        label: "Save",
        shortcut: "Ctrl/⌘ S",
        group: "Resource editing",
        keywords: &["commit", "revision", "draft"],
    },
    CommandMetadata {
        id: CommandId::CancelDraft,
        label: "Cancel",
        shortcut: "Esc",
        group: "Resource editing",
        keywords: &["revert", "discard", "draft"],
    },
    CommandMetadata {
        id: CommandId::FocusChartRail,
        label: "Focus charts",
        shortcut: "Alt 1",
        group: "Navigation",
        keywords: &["workspace", "rail", "charts"],
    },
    CommandMetadata {
        id: CommandId::RefreshView,
        label: "Refresh",
        shortcut: "Ctrl/⌘ R",
        group: "View",
        keywords: &["recalculate", "scene", "retry"],
    },
];

pub fn metadata(id: CommandId) -> &'static CommandMetadata {
    COMMANDS
        .iter()
        .find(|metadata| metadata.id == id)
        .expect("every command ID has metadata")
}

pub fn command_for_key(
    key: &str,
    primary_modifier: bool,
    alt_modifier: bool,
    typing: bool,
) -> Option<CommandId> {
    if primary_modifier && key.eq_ignore_ascii_case("s") {
        return Some(CommandId::SaveDraft);
    }
    if typing {
        return None;
    }
    if key == "Escape" {
        return Some(CommandId::CancelDraft);
    }
    if alt_modifier && key == "1" {
        return Some(CommandId::FocusChartRail);
    }
    if primary_modifier && key.eq_ignore_ascii_case("r") {
        return Some(CommandId::RefreshView);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_preserve_normal_form_entry() {
        assert_eq!(
            command_for_key("s", true, false, true),
            Some(CommandId::SaveDraft)
        );
        assert_eq!(command_for_key("Escape", false, false, true), None);
        assert_eq!(command_for_key("1", false, true, true), None);
        assert_eq!(
            command_for_key("1", false, true, false),
            Some(CommandId::FocusChartRail)
        );
        assert_eq!(
            command_for_key("r", true, false, false),
            Some(CommandId::RefreshView)
        );
    }

    #[test]
    fn command_metadata_keeps_presentation_out_of_application_contract() {
        let save = metadata(CommandId::SaveDraft);
        assert_eq!(save.group, "Resource editing");
        assert!(save.keywords.contains(&"revision"));
    }
}
