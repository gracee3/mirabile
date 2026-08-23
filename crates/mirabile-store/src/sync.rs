use mirabile_core::{CanonicalResource, ResourceId, Revision};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SyncHead {
    pub resource_id: ResourceId,
    pub local_revision: Revision,
    pub remote_revision: Option<Revision>,
    pub state: SyncState,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    Clean,
    LocalDirty,
    Syncing,
    Conflict,
    Error { message: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SyncOperation {
    pub operation_id: String,
    pub resource_id: ResourceId,
    pub base_revision: Option<Revision>,
    pub resource: CanonicalResource,
}
