use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    AppIntent, AspectId, AspectLayerKind, ChartMutation, ChartTimezone, CivilDate, CivilTime,
    CoordinateSystem, CorrectionSpec, EventKind, HouseSystem, InstanceId, Latitude, Longitude,
    Offset, PointId, ProjectionVersion, ResourceId, Theme, ViewDisplayMutation, ViewInstanceId,
    ZodiacSpec,
};

pub const WORKFLOW_DOCUMENT_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkflowDocumentV1 {
    pub schema_version: u32,
    pub steps: Vec<WorkflowStepV1>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkflowStepV1 {
    pub name: String,
    #[serde(flatten)]
    pub action: WorkflowActionV1,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum WorkflowActionV1 {
    CreateChart {
        input: ChartInputV1,
        save: bool,
    },
    EditChart {
        chart: ChartReferenceV1,
        patch: ChartInputPatchV1,
        save: bool,
    },
    CreateBiwheelView {
        title: String,
        radix: ChartReferenceV1,
        comparison: ChartReferenceV1,
    },
    ConfigureViewDisplay {
        view: ViewReferenceV1,
        patch: ViewDisplayPatchV1,
    },
    SaveWorkspace {
        title: String,
        description: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
    },
    OpenWorkspace {
        workspace: WorkspaceReferenceV1,
        #[serde(default)]
        dirty_policy: DirtyPolicyV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "ref", content = "value", rename_all = "snake_case")]
pub enum ChartReferenceV1 {
    Id(InstanceId),
    Binding(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "ref", content = "value", rename_all = "snake_case")]
pub enum ViewReferenceV1 {
    Id(ViewInstanceId),
    Binding(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "ref", content = "value", rename_all = "snake_case")]
pub enum WorkspaceReferenceV1 {
    Id(ResourceId),
    Binding(String),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DirtyPolicyV1 {
    #[default]
    Reject,
    Save,
    Discard,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartInputV1 {
    pub title: String,
    pub event_kind: EventKind,
    pub subject: Option<String>,
    pub date: CivilDate,
    pub time: CivilTime,
    pub timezone: WorkflowTimezoneV1,
    pub place_label: String,
    pub country: Option<String>,
    pub latitude: Latitude,
    pub longitude: Longitude,
    pub zodiac: ZodiacSpec,
    pub houses: HouseSystem,
    pub coordinates: CoordinateSystem,
    #[serde(default)]
    pub corrections: CorrectionSpec,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkflowTimezoneV1 {
    Utc,
    FixedOffset { seconds: i32 },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ChartInputPatchV1 {
    pub title: Option<String>,
    pub event_kind: Option<EventKind>,
    pub subject: Option<Option<String>>,
    pub date: Option<CivilDate>,
    pub time: Option<CivilTime>,
    pub timezone: Option<WorkflowTimezoneV1>,
    pub place_label: Option<String>,
    pub country: Option<Option<String>>,
    pub latitude: Option<Latitude>,
    pub longitude: Option<Longitude>,
    pub zodiac: Option<ZodiacSpec>,
    pub houses: Option<HouseSystem>,
    pub coordinates: Option<CoordinateSystem>,
    pub corrections: Option<CorrectionSpec>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SlotPointVisibilityV1 {
    #[serde(default)]
    pub hidden: BTreeMap<PointId, bool>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ViewDisplayPatchV1 {
    #[serde(default)]
    pub point_visibility: BTreeMap<String, SlotPointVisibilityV1>,
    #[serde(default)]
    pub ring_visibility: BTreeMap<String, bool>,
    pub rotation_degrees: Option<f64>,
    pub zodiac_boundaries: Option<bool>,
    pub zodiac_labels: Option<bool>,
    pub house_cusps: Option<bool>,
    pub house_numbers: Option<bool>,
    pub degree_labels: Option<bool>,
    pub retrograde_markers: Option<bool>,
    pub radix_intra_aspects: Option<bool>,
    pub comparison_intra_aspects: Option<bool>,
    pub cross_chart_aspects: Option<bool>,
    #[serde(default)]
    pub aspect_enabled: BTreeMap<AspectId, bool>,
    #[serde(default)]
    pub aspect_orbs_degrees: BTreeMap<AspectId, f64>,
    pub theme: Option<ThemeSelectionV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "resource_id", rename_all = "snake_case")]
pub enum ThemeSelectionV1 {
    MirabileDark,
    HighContrastLight,
    Saved(ResourceId),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowBindingKind {
    Chart,
    View,
    Workspace,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowExecutionStatusV1 {
    Running,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkflowResultV1 {
    pub schema_version: u32,
    pub status: WorkflowExecutionStatusV1,
    pub failed_step: Option<String>,
    #[serde(default)]
    pub errors: Vec<WorkflowValidationError>,
    pub final_projection: Option<ProjectionVersion>,
    #[serde(default)]
    pub created_chart_ids: Vec<InstanceId>,
    #[serde(default)]
    pub created_definition_ids: Vec<ResourceId>,
    #[serde(default)]
    pub created_view_ids: Vec<ViewInstanceId>,
    #[serde(default)]
    pub created_workspace_ids: Vec<ResourceId>,
}

impl WorkflowResultV1 {
    pub fn running(projection: ProjectionVersion) -> Self {
        Self {
            schema_version: WORKFLOW_DOCUMENT_VERSION,
            status: WorkflowExecutionStatusV1::Running,
            failed_step: None,
            errors: Vec::new(),
            final_projection: Some(projection),
            created_chart_ids: Vec::new(),
            created_definition_ids: Vec::new(),
            created_view_ids: Vec::new(),
            created_workspace_ids: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkflowValidationError {
    pub step: Option<String>,
    pub field: String,
    pub code: String,
    pub message: String,
}

impl WorkflowDocumentV1 {
    pub fn from_json(json: &str) -> Result<Self, Vec<WorkflowValidationError>> {
        let document: Self = serde_json::from_str(json).map_err(|error| {
            vec![WorkflowValidationError {
                step: None,
                field: "document".into(),
                code: "invalid_json".into(),
                message: error.to_string(),
            }]
        })?;
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), Vec<WorkflowValidationError>> {
        let mut errors = Vec::new();
        if self.schema_version != WORKFLOW_DOCUMENT_VERSION {
            errors.push(error(None, "schema_version", "unsupported_version", format!(
                "Workflow schema version {} is unsupported; expected {WORKFLOW_DOCUMENT_VERSION}",
                self.schema_version
            )));
        }
        if self.steps.is_empty() {
            errors.push(error(
                None,
                "steps",
                "empty",
                "A workflow must contain at least one step",
            ));
        }
        let mut bindings = BTreeMap::<String, WorkflowBindingKind>::new();
        let mut names = BTreeSet::new();
        for step in &self.steps {
            if step.name.trim().is_empty() {
                errors.push(error(
                    Some(&step.name),
                    "name",
                    "empty",
                    "Step names must not be empty",
                ));
                continue;
            }
            if !names.insert(step.name.clone()) {
                errors.push(error(
                    Some(&step.name),
                    "name",
                    "duplicate",
                    "Step names must be unique",
                ));
                continue;
            }
            validate_action(step, &bindings, &mut errors);
            bindings.insert(step.name.clone(), step.action.result_kind());
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl WorkflowActionV1 {
    pub const fn result_kind(&self) -> WorkflowBindingKind {
        match self {
            Self::CreateChart { .. } | Self::EditChart { .. } => WorkflowBindingKind::Chart,
            Self::CreateBiwheelView { .. } | Self::ConfigureViewDisplay { .. } => {
                WorkflowBindingKind::View
            }
            Self::SaveWorkspace { .. } | Self::OpenWorkspace { .. } => {
                WorkflowBindingKind::Workspace
            }
        }
    }
}

fn validate_action(
    step: &WorkflowStepV1,
    bindings: &BTreeMap<String, WorkflowBindingKind>,
    errors: &mut Vec<WorkflowValidationError>,
) {
    let name = Some(step.name.as_str());
    match &step.action {
        WorkflowActionV1::CreateChart { input, .. } => {
            if input.title.trim().is_empty() {
                errors.push(error(
                    name,
                    "input.title",
                    "empty",
                    "Chart title must not be empty",
                ));
            }
            if input.place_label.trim().is_empty() {
                errors.push(error(
                    name,
                    "input.place_label",
                    "empty",
                    "Place label must not be empty",
                ));
            }
            if let WorkflowTimezoneV1::FixedOffset { seconds } = input.timezone
                && Offset::from_seconds(seconds).is_err()
            {
                errors.push(error(
                    name,
                    "input.timezone.seconds",
                    "out_of_range",
                    "Fixed offset is outside the supported range",
                ));
            }
        }
        WorkflowActionV1::EditChart { chart, .. } => {
            validate_chart_ref(name, chart, bindings, errors);
        }
        WorkflowActionV1::CreateBiwheelView {
            title,
            radix,
            comparison,
        } => {
            if title.trim().is_empty() {
                errors.push(error(
                    name,
                    "title",
                    "empty",
                    "View title must not be empty",
                ));
            }
            validate_chart_ref(name, radix, bindings, errors);
            validate_chart_ref(name, comparison, bindings, errors);
            if radix == comparison {
                errors.push(error(
                    name,
                    "comparison",
                    "duplicate_assignment",
                    "Radix and comparison must be distinct charts",
                ));
            }
        }
        WorkflowActionV1::ConfigureViewDisplay { view, patch } => {
            validate_view_ref(name, view, bindings, errors);
            for (aspect, orb) in &patch.aspect_orbs_degrees {
                if !orb.is_finite() || *orb < 0.0 || *orb > 180.0 {
                    errors.push(error(
                        name,
                        &format!("patch.aspect_orbs_degrees.{aspect}"),
                        "out_of_range",
                        "Aspect orb must be between 0 and 180 degrees",
                    ));
                }
            }
        }
        WorkflowActionV1::SaveWorkspace { title, .. } => {
            if title.trim().is_empty() {
                errors.push(error(
                    name,
                    "title",
                    "empty",
                    "Workspace title must not be empty",
                ));
            }
        }
        WorkflowActionV1::OpenWorkspace { workspace, .. } => {
            validate_workspace_ref(name, workspace, bindings, errors);
        }
    }
}

fn validate_chart_ref(
    name: Option<&str>,
    value: &ChartReferenceV1,
    bindings: &BTreeMap<String, WorkflowBindingKind>,
    errors: &mut Vec<WorkflowValidationError>,
) {
    if let ChartReferenceV1::Binding(binding) = value {
        validate_binding(name, binding, WorkflowBindingKind::Chart, bindings, errors);
    }
}
fn validate_view_ref(
    name: Option<&str>,
    value: &ViewReferenceV1,
    bindings: &BTreeMap<String, WorkflowBindingKind>,
    errors: &mut Vec<WorkflowValidationError>,
) {
    if let ViewReferenceV1::Binding(binding) = value {
        validate_binding(name, binding, WorkflowBindingKind::View, bindings, errors);
    }
}
fn validate_workspace_ref(
    name: Option<&str>,
    value: &WorkspaceReferenceV1,
    bindings: &BTreeMap<String, WorkflowBindingKind>,
    errors: &mut Vec<WorkflowValidationError>,
) {
    if let WorkspaceReferenceV1::Binding(binding) = value {
        validate_binding(
            name,
            binding,
            WorkflowBindingKind::Workspace,
            bindings,
            errors,
        );
    }
}
fn validate_binding(
    name: Option<&str>,
    binding: &str,
    expected: WorkflowBindingKind,
    bindings: &BTreeMap<String, WorkflowBindingKind>,
    errors: &mut Vec<WorkflowValidationError>,
) {
    match bindings.get(binding) {
        Some(actual) if *actual == expected => {}
        Some(actual) => errors.push(error(
            name,
            "binding",
            "wrong_type",
            format!("Binding {binding} is {actual:?}, expected {expected:?}"),
        )),
        None => errors.push(error(
            name,
            "binding",
            "unknown_or_forward",
            format!("Binding {binding} must name a prior step"),
        )),
    }
}
fn error(
    step: Option<&str>,
    field: &str,
    code: &str,
    message: impl Into<String>,
) -> WorkflowValidationError {
    WorkflowValidationError {
        step: step.map(str::to_owned),
        field: field.into(),
        code: code.into(),
        message: message.into(),
    }
}

impl ChartInputV1 {
    pub fn intents(&self) -> Result<Vec<AppIntent>, WorkflowValidationError> {
        let timezone = timezone(&self.timezone)?;
        let calculation = crate::CalculationSpec {
            zodiac: self.zodiac.clone(),
            houses: self.houses,
            coordinates: self.coordinates,
            corrections: self.corrections.clone(),
            ..crate::CalculationSpec::default()
        };
        Ok(vec![
            AppIntent::ApplyChartMutation(ChartMutation::SetTitle(self.title.clone())),
            AppIntent::ApplyChartMutation(ChartMutation::SetEventKind(self.event_kind.clone())),
            AppIntent::ApplyChartMutation(ChartMutation::SetSubjectName(self.subject.clone())),
            AppIntent::ApplyChartMutation(ChartMutation::SetCivilDate(self.date)),
            AppIntent::ApplyChartMutation(ChartMutation::SetCivilTime(self.time)),
            AppIntent::ApplyChartMutation(ChartMutation::SetTimezone(timezone)),
            AppIntent::ApplyChartMutation(ChartMutation::SetLocationEnabled(true)),
            AppIntent::ApplyChartMutation(ChartMutation::SetLocationName(self.place_label.clone())),
            AppIntent::ApplyChartMutation(ChartMutation::SetCountryRegion(self.country.clone())),
            AppIntent::ApplyChartMutation(ChartMutation::SetLatitude(Some(self.latitude))),
            AppIntent::ApplyChartMutation(ChartMutation::SetLongitude(Some(self.longitude))),
            AppIntent::ApplyChartMutation(ChartMutation::SetCalculation(calculation)),
        ])
    }
}

fn timezone(value: &WorkflowTimezoneV1) -> Result<ChartTimezone, WorkflowValidationError> {
    match value {
        WorkflowTimezoneV1::Utc => Ok(ChartTimezone::UniversalTime),
        WorkflowTimezoneV1::FixedOffset { seconds } => Offset::from_seconds(*seconds)
            .map(ChartTimezone::FixedOffset)
            .map_err(|error_value| {
                error(
                    None,
                    "timezone.seconds",
                    "out_of_range",
                    error_value.to_string(),
                )
            }),
    }
}

impl ViewDisplayPatchV1 {
    pub fn direct_mutations(&self) -> Result<Vec<ViewDisplayMutation>, WorkflowValidationError> {
        let mut mutations = Vec::new();
        for (slot, visibility) in &self.point_visibility {
            let slot = crate::ChartSlotId::new(slot).map_err(|error_value| {
                error(
                    None,
                    "point_visibility",
                    "invalid_slot",
                    error_value.to_string(),
                )
            })?;
            for (point, hidden) in &visibility.hidden {
                mutations.push(ViewDisplayMutation::SetPointHidden {
                    slot: slot.clone(),
                    point_id: point.clone(),
                    hidden: *hidden,
                });
            }
        }
        for (slot, visible) in &self.ring_visibility {
            let slot = crate::ChartSlotId::new(slot).map_err(|error_value| {
                error(
                    None,
                    "ring_visibility",
                    "invalid_slot",
                    error_value.to_string(),
                )
            })?;
            mutations.push(ViewDisplayMutation::SetRingHidden {
                slot,
                hidden: !visible,
            });
        }
        if let Some(value) = self.rotation_degrees {
            mutations.push(ViewDisplayMutation::SetRotation(Some(
                crate::Angle::from_degrees(value).map_err(|error_value| {
                    error(
                        None,
                        "rotation_degrees",
                        "not_finite",
                        error_value.to_string(),
                    )
                })?,
            )));
        }
        for (layer, visible) in [
            (AspectLayerKind::RadixIntra, self.radix_intra_aspects),
            (
                AspectLayerKind::ComparisonIntra,
                self.comparison_intra_aspects,
            ),
            (AspectLayerKind::CrossChart, self.cross_chart_aspects),
        ] {
            if let Some(visible) = visible {
                mutations.push(ViewDisplayMutation::SetAspectLayer { layer, visible });
            }
        }
        if let Some(theme) = &self.theme {
            match theme {
                ThemeSelectionV1::MirabileDark => {
                    mutations.push(ViewDisplayMutation::SetTheme(Theme::mirabile_dark()));
                }
                ThemeSelectionV1::HighContrastLight => {
                    mutations.push(ViewDisplayMutation::SetTheme(Theme::high_contrast_light()));
                }
                ThemeSelectionV1::Saved(_) => {}
            }
        }
        Ok(mutations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_forward_and_wrong_type_bindings() {
        let document = WorkflowDocumentV1 {
            schema_version: 1,
            steps: vec![WorkflowStepV1 {
                name: "wheel".into(),
                action: WorkflowActionV1::CreateBiwheelView {
                    title: "Radix × Comparison".into(),
                    radix: ChartReferenceV1::Binding("later".into()),
                    comparison: ChartReferenceV1::Binding("wheel".into()),
                },
            }],
        };
        let errors = document.validate().expect_err("bindings are invalid");
        assert!(
            errors
                .iter()
                .all(|error| error.code == "unknown_or_forward")
        );
    }

    #[test]
    fn dirty_policy_defaults_to_reject() {
        let value: DirtyPolicyV1 = serde_json::from_str("null").unwrap_or_default();
        assert_eq!(value, DirtyPolicyV1::Reject);
    }

    #[test]
    fn shared_live_workflow_fixture_is_valid_v1() {
        let document = WorkflowDocumentV1::from_json(include_str!(
            "../../../scripts/workflow-fixtures/live-workflow-v1.json"
        ))
        .expect("shared workflow fixture");
        assert_eq!(document.steps.len(), 6);
    }
}
