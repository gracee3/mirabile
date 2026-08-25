use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    AppReadModel, ApplicationStatus, ControlAddress, ControlDescriptor, InstanceId,
    PendingOperationReadModel, ProjectionVersion, ResourceId, ViewComputationState, ViewInstanceId,
};

pub const AUTOMATION_SNAPSHOT_VERSION: u32 = 1;
pub const TRACE_HISTORY_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionSource {
    Agent,
    Human,
    Macro,
    System,
    Test,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PendingTransition {
    pub projection: ProjectionVersion,
    pub pending_operations: Vec<PendingOperationReadModel>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExecutionOutcome {
    Settled,
    Rejected { kind: String, message: String },
    Failed { kind: String, message: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExecutionTraceEntry {
    pub sequence: u64,
    pub source: ActionSource,
    pub origin_control: Option<ControlAddress>,
    pub semantic_intent: String,
    pub accepted_projection: Option<ProjectionVersion>,
    pub settled_projection: ProjectionVersion,
    pub pending_transitions: Vec<PendingTransition>,
    pub outcome: ExecutionOutcome,
}

#[derive(Clone, Debug, Default)]
pub struct TraceHistory {
    entries: VecDeque<ExecutionTraceEntry>,
}

impl TraceHistory {
    pub fn push(&mut self, entry: ExecutionTraceEntry) {
        if self.entries.len() == TRACE_HISTORY_LIMIT {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn entries(&self) -> Vec<ExecutionTraceEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum MacroCoordinatorState {
    #[default]
    Idle,
    Recording,
    Replaying {
        step: usize,
        total: usize,
    },
    Failed {
        step: usize,
        message: String,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CoordinatorReadModel {
    pub running: bool,
    pub queued_actions: usize,
    pub current_source: Option<ActionSource>,
    pub highlighted_control: Option<ControlAddress>,
    pub macro_state: MacroCoordinatorState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AutomationSnapshotV1 {
    pub schema_version: u32,
    pub application: AutomationApplicationSnapshot,
    pub workspace: AutomationWorkspaceSnapshot,
    pub chart: Option<AutomationChartSnapshot>,
    pub view: Option<AutomationViewSnapshot>,
    pub calculation: Option<CalculationDiagnosticsReadModel>,
    pub authoring: crate::AuthoringCapabilitiesReadModel,
    pub chart_editor: Option<crate::ChartEditorReadModel>,
    pub controls: Vec<ControlDescriptor>,
    pub coordinator: CoordinatorReadModel,
    pub recent_trace: Vec<ExecutionTraceEntry>,
}

impl AutomationSnapshotV1 {
    pub fn capture(
        model: &AppReadModel,
        controls: Vec<ControlDescriptor>,
        coordinator: CoordinatorReadModel,
        recent_trace: Vec<ExecutionTraceEntry>,
    ) -> Self {
        let application = AutomationApplicationSnapshot {
            projection: model.version,
            status: match &model.status {
                ApplicationStatus::Initializing => "initializing",
                ApplicationStatus::Ready => "ready",
                ApplicationStatus::Error(_) => "error",
            }
            .into(),
            settled: model.is_settled(),
            pending_operations: model.activity.pending_operations.clone(),
            notice: model.notice.as_ref().map(|notice| notice.message.clone()),
        };
        let workspace = AutomationWorkspaceSnapshot {
            resource_id: model.workspace.document_id,
            revision: model.workspace.document_revision.map(crate::Revision::get),
            dirty: model.workspace.document_dirty,
            active_chart: model.workspace.active_chart,
            active_view: model.workspace.active_view,
            temporary_display_override: model.workspace.has_temporary_display_override,
        };
        let chart = model
            .inspector
            .active_chart
            .as_ref()
            .map(|chart| AutomationChartSnapshot {
                instance_id: chart.instance_id,
                title: chart.title.clone(),
                saved: matches!(chart.persistence, crate::ChartPersistence::Saved { .. }),
            });
        let view = model
            .active_view
            .as_ref()
            .map(|view| AutomationViewSnapshot {
                view_id: view.view_id,
                title: view.title.clone(),
                computation: computation_label(&view.computation).into(),
                last_good_scene_present: view.scene.is_some(),
            });
        Self {
            schema_version: AUTOMATION_SNAPSHOT_VERSION,
            application,
            workspace,
            chart,
            view,
            calculation: model.calculation.clone(),
            authoring: model.authoring.clone(),
            chart_editor: model.chart_editor.clone(),
            controls,
            coordinator,
            recent_trace,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationApplicationSnapshot {
    pub projection: ProjectionVersion,
    pub status: String,
    pub settled: bool,
    pub pending_operations: Vec<PendingOperationReadModel>,
    pub notice: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationWorkspaceSnapshot {
    pub resource_id: Option<ResourceId>,
    pub revision: Option<u64>,
    pub dirty: bool,
    pub active_chart: Option<InstanceId>,
    pub active_view: Option<ViewInstanceId>,
    pub temporary_display_override: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationChartSnapshot {
    pub instance_id: InstanceId,
    pub title: String,
    pub saved: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationViewSnapshot {
    pub view_id: ViewInstanceId,
    pub title: String,
    pub computation: String,
    pub last_good_scene_present: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImplementationIdentityReadModel {
    pub id: String,
    pub version: String,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CalculationDiagnosticsReadModel {
    pub backend: ImplementationIdentityReadModel,
    pub engine: ImplementationIdentityReadModel,
    pub worker_protocol: u32,
    pub active_request_id: Option<u64>,
    pub calc_key: Option<String>,
    pub analysis_key: Option<String>,
    pub computation: Option<String>,
    pub last_good_scene_present: bool,
}

fn computation_label(state: &ViewComputationState) -> &'static str {
    match state {
        ViewComputationState::Loading => "loading",
        ViewComputationState::Fresh => "fresh",
        ViewComputationState::Refreshing => "refreshing",
        ViewComputationState::Failed(_) => "failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_history_is_bounded_to_the_most_recent_entries() {
        let mut history = TraceHistory::default();
        for sequence in 1..=(TRACE_HISTORY_LIMIT as u64 + 3) {
            history.push(ExecutionTraceEntry {
                sequence,
                source: ActionSource::Test,
                origin_control: None,
                semantic_intent: "fixture".into(),
                accepted_projection: None,
                settled_projection: ProjectionVersion::new(sequence),
                pending_transitions: Vec::new(),
                outcome: ExecutionOutcome::Settled,
            });
        }
        let entries = history.entries();
        assert_eq!(entries.len(), TRACE_HISTORY_LIMIT);
        assert_eq!(entries.first().map(|entry| entry.sequence), Some(4));
    }

    #[test]
    fn snapshots_exclude_scene_contents_but_retain_presence() {
        let model = AppReadModel::initializing();
        let snapshot = AutomationSnapshotV1::capture(
            &model,
            Vec::new(),
            CoordinatorReadModel::default(),
            Vec::new(),
        );
        let json = serde_json::to_value(snapshot).expect("snapshot serializes");
        assert_eq!(json["schema_version"], AUTOMATION_SNAPSHOT_VERSION);
        assert!(json.get("scene").is_none());
    }
}
