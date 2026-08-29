use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{
    AppReadModel, ApplicationStatus, ControlAddress, ControlDescriptor, InstanceId,
    PendingOperationReadModel, ProjectionVersion, ResourceId, Scene, ViewComputationState,
    ViewInstanceId,
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
    pub resource_editor: crate::ResourceEditorReadModel,
    pub semantic_output: AutomationSemanticOutput,
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
            title: model.workspace.title.clone(),
            resource_id: model.workspace.document_id,
            revision: model.workspace.document_revision.map(crate::Revision::get),
            dirty: model.workspace.document_dirty,
            active_chart: model.workspace.active_chart,
            active_view: model.workspace.active_view,
            temporary_display_override: model.workspace.has_temporary_display_override,
            switch_reasons: model
                .workspace
                .switch_decision
                .as_ref()
                .map(|decision| decision.reasons.clone())
                .unwrap_or_default(),
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
                scene_manifest: view.scene.as_ref().map(AutomationSceneManifest::capture),
                points: view
                    .display
                    .points
                    .iter()
                    .map(|point| AutomationPointVisibility {
                        point_id: point.point_id.as_str().into(),
                        visible: point.visible,
                        durable_visible: point.durable_visible,
                        temporary_visible: point.temporary_visible,
                    })
                    .collect(),
                slots: view
                    .slots
                    .iter()
                    .map(|slot| {
                        let (source, definition_id, promotion) = match slot.source {
                            crate::SlotAssignmentSource::Unassigned => ("unassigned", None, None),
                            crate::SlotAssignmentSource::Saved { definition_id, .. } => {
                                ("saved", Some(definition_id), None)
                            }
                            crate::SlotAssignmentSource::Draft { .. } => {
                                ("draft", None, Some("requires_chart_save"))
                            }
                        };
                        AutomationSlotAssignment {
                            slot: slot.slot.as_str().into(),
                            chart: slot.chart,
                            durable_chart: slot.durable_chart,
                            draft_chart: slot.draft_chart,
                            source: source.into(),
                            definition_id,
                            promotion: promotion.map(str::to_owned),
                        }
                    })
                    .collect(),
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
            resource_editor: model.resource_editor.clone(),
            semantic_output: AutomationSemanticOutput::capture(&model.semantic_output),
            controls,
            coordinator,
            recent_trace,
        }
    }
}

const AUTOMATION_SEMANTIC_ROW_LIMIT: usize = 64;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct AutomationSemanticOutput {
    pub points: Vec<AutomationSemanticPoint>,
    pub houses: Vec<AutomationSemanticHouse>,
    pub angles: Vec<AutomationSemanticAngle>,
    pub aspects: Vec<AutomationSemanticAspect>,
    pub provenance: Vec<AutomationProvenanceEntry>,
    pub unavailable_reason: Option<String>,
}

impl AutomationSemanticOutput {
    fn capture(value: &crate::SemanticOutputReadModel) -> Self {
        Self {
            points: value
                .points
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|point| AutomationSemanticPoint {
                    point_id: point.point_id.to_string(),
                    longitude_degrees: point.longitude_degrees,
                    retrograde: point.retrograde,
                    derived: point.derived,
                })
                .collect(),
            houses: value
                .houses
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|house| AutomationSemanticHouse {
                    number: house.number,
                    cusp_degrees: house.cusp_degrees,
                })
                .collect(),
            angles: value
                .angles
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|angle| AutomationSemanticAngle {
                    name: angle.name.clone(),
                    longitude_degrees: angle.longitude_degrees,
                })
                .collect(),
            aspects: value
                .aspects
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|aspect| AutomationSemanticAspect {
                    lhs: aspect.lhs.to_string(),
                    rhs: aspect.rhs.to_string(),
                    aspect: aspect.aspect.to_string(),
                    orb_degrees: aspect.orb_degrees,
                    applying: aspect.applying,
                })
                .collect(),
            provenance: value
                .provenance
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|entry| AutomationProvenanceEntry {
                    responsibility: entry.responsibility.clone(),
                    implementation: entry.implementation.clone(),
                    detail: entry.detail.clone(),
                })
                .collect(),
            unavailable_reason: value.unavailable_reason.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AutomationSemanticPoint {
    pub point_id: String,
    pub longitude_degrees: f64,
    pub retrograde: bool,
    pub derived: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AutomationSemanticHouse {
    pub number: usize,
    pub cusp_degrees: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AutomationSemanticAngle {
    pub name: String,
    pub longitude_degrees: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AutomationSemanticAspect {
    pub lhs: String,
    pub rhs: String,
    pub aspect: String,
    pub orb_degrees: f64,
    pub applying: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationProvenanceEntry {
    pub responsibility: String,
    pub implementation: String,
    pub detail: String,
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
    pub title: String,
    pub resource_id: Option<ResourceId>,
    pub revision: Option<u64>,
    pub dirty: bool,
    pub active_chart: Option<InstanceId>,
    pub active_view: Option<ViewInstanceId>,
    pub temporary_display_override: bool,
    pub switch_reasons: Vec<String>,
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
    #[serde(default)]
    pub scene_manifest: Option<AutomationSceneManifest>,
    pub points: Vec<AutomationPointVisibility>,
    pub slots: Vec<AutomationSlotAssignment>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationSceneManifest {
    pub zodiac_count: usize,
    pub house_count: usize,
    pub angle_count: usize,
    pub point_count: usize,
    pub aspect_count: usize,
    pub zodiac_ids: Vec<String>,
    pub houses: Vec<AutomationSceneHouse>,
    pub angles: Vec<AutomationSceneAngle>,
    pub points: Vec<AutomationScenePoint>,
    pub aspects: Vec<AutomationSceneAspect>,
}

impl AutomationSceneManifest {
    fn capture(scene: &Scene) -> Self {
        Self {
            zodiac_count: scene.zodiac.len(),
            house_count: scene.houses.len(),
            angle_count: scene.angles.len(),
            point_count: scene.points.len(),
            aspect_count: scene.aspects.len(),
            zodiac_ids: scene
                .zodiac
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|sign| sign.id.clone())
                .collect(),
            houses: scene
                .houses
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|house| AutomationSceneHouse {
                    number: house.number,
                    cusp_visible: house.show_cusp,
                    number_visible: house.show_number,
                })
                .collect(),
            angles: scene
                .angles
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|angle| AutomationSceneAngle {
                    id: angle.id.clone(),
                    derived_opposite: angle.derived_opposite,
                })
                .collect(),
            points: scene
                .points
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|point| AutomationScenePoint {
                    point_id: point.point.to_string(),
                    label: point.display_label.clone(),
                    formatted_position: point.formatted_position.clone(),
                    retrograde: point.retrograde,
                    retrograde_marker: point.retrograde && point.show_retrograde,
                    displaced: point.leader.is_some(),
                    glyph_fallback: point.glyph_fallback,
                })
                .collect(),
            aspects: scene
                .aspects
                .iter()
                .take(AUTOMATION_SEMANTIC_ROW_LIMIT)
                .map(|aspect| AutomationSceneAspect {
                    aspect_id: aspect.aspect_id.clone(),
                    lhs: aspect.lhs.to_string(),
                    rhs: aspect.rhs.to_string(),
                    classification: format!("{:?}", aspect.classification).to_lowercase(),
                    chord: aspect.draw_chord,
                    applying: aspect.applying,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationSceneHouse {
    pub number: usize,
    pub cusp_visible: bool,
    pub number_visible: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationSceneAngle {
    pub id: String,
    pub derived_opposite: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct AutomationScenePoint {
    pub point_id: String,
    pub label: String,
    pub formatted_position: String,
    pub retrograde: bool,
    pub retrograde_marker: bool,
    pub displaced: bool,
    pub glyph_fallback: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationSceneAspect {
    pub aspect_id: String,
    pub lhs: String,
    pub rhs: String,
    pub classification: String,
    pub chord: bool,
    pub applying: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationPointVisibility {
    pub point_id: String,
    pub visible: bool,
    pub durable_visible: bool,
    pub temporary_visible: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AutomationSlotAssignment {
    pub slot: String,
    pub chart: Option<InstanceId>,
    pub durable_chart: Option<InstanceId>,
    pub draft_chart: Option<InstanceId>,
    pub source: String,
    pub definition_id: Option<ResourceId>,
    pub promotion: Option<String>,
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

    #[test]
    fn scene_manifest_is_bounded_semantic_and_excludes_geometry() {
        let scene = Scene {
            width: 520.0,
            height: 520.0,
            lines: vec![crate::Line {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                stroke: crate::StrokeRole::Accent,
                width: 1.0,
            }],
            zodiac: vec![crate::ZodiacDivision {
                index: 0,
                id: "aries".into(),
                name: "Aries".into(),
                glyph: "♈".into(),
                longitude_degrees: 0.0,
                screen_angle_degrees: 270.0,
                line: crate::LineGeometry {
                    x1: 10.0,
                    y1: 11.0,
                    x2: 12.0,
                    y2: 13.0,
                },
                label_x: 14.0,
                label_y: 15.0,
                show_boundary: true,
                show_label: true,
            }],
            ..Scene::default()
        };
        let manifest = AutomationSceneManifest::capture(&scene);
        assert_eq!(manifest.zodiac_count, 1);
        assert_eq!(manifest.zodiac_ids, ["aries"]);
        let json = serde_json::to_value(manifest).expect("manifest serializes");
        let object = json.as_object().expect("manifest object");
        for forbidden in ["width", "height", "lines", "line", "x", "y", "x1", "y1"] {
            assert!(
                !object.contains_key(forbidden),
                "geometry field {forbidden}"
            );
        }
        assert!(!json.to_string().contains("520.0"));
        assert!(!json.to_string().contains("\"x1\""));
    }
}
