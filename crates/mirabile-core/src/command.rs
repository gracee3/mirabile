use serde::{Deserialize, Serialize};

use crate::{CanonicalResource, ChartSlotId, InstanceId, ResourceId, Revision, ViewInstanceId};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    CreateResource {
        resource: CanonicalResource,
    },
    SaveResourceDraft {
        expected_revision: Revision,
        resource: CanonicalResource,
    },
    /// Mutates the application-selected workspace session; no canonical workspace identity is
    /// required until an explicit persistence intent.
    OpenSavedChart {
        definition: ResourceId,
        instance_id: InstanceId,
    },
    CloseChart {
        instance_id: InstanceId,
    },
    SetActiveChart {
        instance_id: Option<InstanceId>,
    },
    SetChartSelection {
        instance_id: InstanceId,
        selected: bool,
    },
    SetActiveView {
        view: Option<ViewInstanceId>,
    },
    AssignChartSlot {
        view: ViewInstanceId,
        slot: ChartSlotId,
        chart: Option<InstanceId>,
    },
    SetWorkspaceAspectSet {
        aspect_set: ResourceId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_mutations_have_typed_portable_commands_without_canonical_workspace_identity() {
        let instance_id = InstanceId::new();
        let commands = vec![
            Command::CloseChart { instance_id },
            Command::SetChartSelection {
                instance_id,
                selected: true,
            },
            Command::AssignChartSlot {
                view: ViewInstanceId::new(),
                slot: ChartSlotId::new("radix").expect("slot ID is valid"),
                chart: Some(instance_id),
            },
        ];

        let json = serde_json::to_string(&commands).expect("commands serialize");
        assert!(!json.contains("workspace"));
        let decoded: Vec<Command> = serde_json::from_str(&json).expect("commands deserialize");
        assert_eq!(decoded, commands);
    }
}
