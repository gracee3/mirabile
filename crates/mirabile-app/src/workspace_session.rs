use std::collections::BTreeMap;

use mirabile_core::{
    ChartSlotId, InstanceId, ResourceEnvelope, ResourceId, Revision, ViewInstanceId, ViewOverrides,
    WorkspaceDocument,
};

use crate::ChartDraft;

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceSessionDraftChart {
    pub instance_id: InstanceId,
    pub draft: ChartDraft,
}

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
    /// Working title; canonical metadata lives on the saved `ResourceEnvelope`.
    pub working_title: String,
    pub document: WorkspaceDocument,
    pub active_chart: Option<InstanceId>,
    pub selected_charts: Vec<InstanceId>,
    pub active_view: Option<ViewInstanceId>,
    pub draft_charts: Vec<WorkspaceSessionDraftChart>,
    /// Effective chart-slot assignments that point only at unsaved session drafts.
    ///
    /// These overlays never enter the durable `WorkspaceDocument`. Saving a draft promotes its
    /// assignments after the same instance becomes a saved workspace chart.
    pub draft_chart_assignments: BTreeMap<ViewInstanceId, BTreeMap<ChartSlotId, InstanceId>>,
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
            working_title: document.title.clone(),
            active_chart: document
                .payload
                .chart_instances
                .first()
                .map(|chart| chart.instance_id),
            selected_charts: Vec::new(),
            active_view: document.payload.views.first().map(|view| view.id),
            document: document.payload.clone(),
            draft_charts: Vec::new(),
            draft_chart_assignments: BTreeMap::new(),
            temporary_view_overrides: BTreeMap::new(),
            document_dirty: false,
        }
    }

    pub fn unsaved(document: WorkspaceDocument) -> Self {
        let active_chart = document
            .chart_instances
            .first()
            .map(|chart| chart.instance_id);
        let active_view = document.views.first().map(|view| view.id);
        Self {
            backing: WorkspaceDocumentBacking::Unsaved,
            working_title: "Untitled Workspace".into(),
            document,
            active_chart,
            selected_charts: Vec::new(),
            active_view,
            draft_charts: Vec::new(),
            draft_chart_assignments: BTreeMap::new(),
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
            || self
                .draft_charts
                .iter()
                .any(|chart| chart.instance_id == instance_id)
    }

    pub fn contains_saved_chart(&self, instance_id: InstanceId) -> bool {
        self.document
            .chart_instances
            .iter()
            .any(|chart| chart.instance_id == instance_id)
    }

    pub fn contains_draft_chart(&self, instance_id: InstanceId) -> bool {
        self.draft_charts
            .iter()
            .any(|chart| chart.instance_id == instance_id)
    }

    pub fn effective_chart_assignment(
        &self,
        view_id: ViewInstanceId,
        slot: &ChartSlotId,
    ) -> Option<InstanceId> {
        self.draft_chart_assignments
            .get(&view_id)
            .and_then(|assignments| assignments.get(slot))
            .copied()
            .or_else(|| {
                self.document
                    .views
                    .iter()
                    .find(|view| view.id == view_id)
                    .and_then(|view| view.charts.get(slot))
                    .copied()
            })
    }

    pub fn effective_chart_assignments(
        &self,
        view_id: ViewInstanceId,
    ) -> BTreeMap<ChartSlotId, InstanceId> {
        let mut assignments = self
            .document
            .views
            .iter()
            .find(|view| view.id == view_id)
            .map(|view| view.charts.clone())
            .unwrap_or_default();
        if let Some(overrides) = self.draft_chart_assignments.get(&view_id) {
            assignments.extend(overrides.clone());
        }
        assignments
    }

    pub(crate) fn remove_draft_assignments(&mut self, instance_id: InstanceId) -> bool {
        let mut removed = false;
        self.draft_chart_assignments.retain(|_, assignments| {
            let before = assignments.len();
            assignments.retain(|_, chart| *chart != instance_id);
            removed |= assignments.len() != before;
            !assignments.is_empty()
        });
        removed
    }

    pub(crate) fn promote_draft_assignments(&mut self, instance_id: InstanceId) {
        let promoted = self
            .draft_chart_assignments
            .iter()
            .flat_map(|(view_id, assignments)| {
                assignments.iter().filter_map(move |(slot, chart)| {
                    (*chart == instance_id).then_some((*view_id, slot.clone()))
                })
            })
            .collect::<Vec<_>>();
        for (view_id, slot) in promoted {
            if let Some(view) = self
                .document
                .views
                .iter_mut()
                .find(|view| view.id == view_id)
            {
                view.charts.insert(slot, instance_id);
            }
        }
        self.remove_draft_assignments(instance_id);
    }
}
