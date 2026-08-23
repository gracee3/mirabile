use std::collections::BTreeMap;

use mirabile_core::{
    InstanceId, ResourceEnvelope, ResourceId, Revision, ViewInstanceId, ViewOverrides,
    WorkspaceDocument,
};

/// Whether a session's durable projection has a canonical saved document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkspaceDocumentBacking {
    Saved {
        document_id: ResourceId,
        revision: Revision,
    },
    Unsaved,
}

/// Application-owned working state for one workspace.
///
/// `document` is the session's durable working projection. Interaction state and temporary
/// overrides are deliberately adjacent to, but not serialized into, that canonical payload.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceSession {
    pub backing: WorkspaceDocumentBacking,
    pub document: WorkspaceDocument,
    pub active_chart: Option<InstanceId>,
    pub selected_charts: Vec<InstanceId>,
    pub active_view: Option<ViewInstanceId>,
    pub temporary_view_overrides: BTreeMap<ViewInstanceId, ViewOverrides>,
    pub document_dirty: bool,
}

impl WorkspaceSession {
    pub fn from_saved(document: &ResourceEnvelope<WorkspaceDocument>) -> Self {
        Self {
            backing: WorkspaceDocumentBacking::Saved {
                document_id: document.id,
                revision: document.revision,
            },
            active_chart: document
                .payload
                .chart_instances
                .first()
                .map(|chart| chart.instance_id),
            selected_charts: Vec::new(),
            active_view: document.payload.views.first().map(|view| view.id),
            document: document.payload.clone(),
            temporary_view_overrides: BTreeMap::new(),
            document_dirty: false,
        }
    }

    pub fn mark_document_dirty(&mut self) {
        self.document_dirty = true;
    }

    pub fn mark_saved(&mut self, document_id: ResourceId, revision: Revision) {
        self.backing = WorkspaceDocumentBacking::Saved {
            document_id,
            revision,
        };
        self.document_dirty = false;
    }

    pub fn contains_chart(&self, instance_id: InstanceId) -> bool {
        self.document
            .chart_instances
            .iter()
            .any(|chart| chart.instance_id == instance_id)
    }
}
