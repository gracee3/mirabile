use serde::{Deserialize, Serialize};

use crate::{
    CanonicalResource, ChartDefinition, ChartSlotId, InstanceId, ResourceId, Revision,
    ViewInstanceId,
};

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
    OpenSavedChart {
        workspace: ResourceId,
        definition: ResourceId,
        instance_id: InstanceId,
    },
    OpenEphemeralChart {
        workspace: ResourceId,
        definition: Box<ChartDefinition>,
        instance_id: InstanceId,
    },
    CloseChart {
        workspace: ResourceId,
        instance_id: InstanceId,
    },
    SetActiveChart {
        workspace: ResourceId,
        instance_id: Option<InstanceId>,
    },
    SetChartSelection {
        workspace: ResourceId,
        instance_id: InstanceId,
        selected: bool,
    },
    SetActiveView {
        workspace: ResourceId,
        view: Option<ViewInstanceId>,
    },
    AssignChartSlot {
        workspace: ResourceId,
        view: ViewInstanceId,
        slot: ChartSlotId,
        chart: Option<InstanceId>,
    },
    SetWorkspaceAspectSet {
        workspace: ResourceId,
        aspect_set: ResourceId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_workspace_mutations_have_typed_portable_commands() {
        let workspace = ResourceId::new();
        let instance_id = InstanceId::new();
        let commands = vec![
            Command::CloseChart {
                workspace,
                instance_id,
            },
            Command::SetChartSelection {
                workspace,
                instance_id,
                selected: true,
            },
            Command::AssignChartSlot {
                workspace,
                view: ViewInstanceId::new(),
                slot: ChartSlotId::new("radix").expect("slot ID is valid"),
                chart: Some(instance_id),
            },
        ];

        let json = serde_json::to_string(&commands).expect("commands serialize");
        let decoded: Vec<Command> = serde_json::from_str(&json).expect("commands deserialize");
        assert_eq!(decoded, commands);
    }
}
