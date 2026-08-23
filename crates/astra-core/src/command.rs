use serde::{Deserialize, Serialize};

use crate::{CanonicalResource, ChartDefinition, InstanceId, ResourceId, Revision, ViewInstanceId};

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
    SetActiveChart {
        workspace: ResourceId,
        instance_id: Option<InstanceId>,
    },
    SetActiveView {
        workspace: ResourceId,
        view: Option<ViewInstanceId>,
    },
    SetWorkspaceAspectSet {
        workspace: ResourceId,
        aspect_set: ResourceId,
    },
}
