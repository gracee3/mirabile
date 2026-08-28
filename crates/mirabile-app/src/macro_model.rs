use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use crate::{
    AppIntent, AppReadModel, AspectId, AspectSetDraftMutation, ChartMutation, ChartSlotId,
    ChartTimezone, CivilDate, CivilTime, ControlAddress, CoordinateSystem, EventKind, HouseSystem,
    InstanceId, Latitude, Longitude, PointId, ResourceId, ViewInstanceId, WorkspaceSwitchAction,
    ZodiacSpec,
};

pub const MACRO_SCHEMA_VERSION: u32 = 1;
pub const MACRO_STEP_LIMIT: usize = 512;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MacroDocumentV1 {
    pub schema_version: u32,
    pub name: String,
    pub steps: Vec<MacroStepV1>,
}

impl MacroDocumentV1 {
    pub fn new(name: impl Into<String>, steps: Vec<MacroStepV1>) -> Result<Self, MacroError> {
        let document = Self {
            schema_version: MACRO_SCHEMA_VERSION,
            name: name.into(),
            steps,
        };
        document.validate()?;
        Ok(document)
    }

    pub fn from_json(json: &str) -> Result<Self, MacroError> {
        let document: Self = serde_json::from_str(json)
            .map_err(|error| MacroError::InvalidJson(error.to_string()))?;
        document.validate()?;
        Ok(document)
    }

    pub fn validate(&self) -> Result<(), MacroError> {
        if self.schema_version != MACRO_SCHEMA_VERSION {
            return Err(MacroError::UnsupportedVersion(self.schema_version));
        }
        if self.name.trim().is_empty() {
            return Err(MacroError::InvalidName);
        }
        if self.steps.len() > MACRO_STEP_LIMIT {
            return Err(MacroError::TooManySteps(self.steps.len()));
        }
        let mut defined = BTreeMap::<MacroBindingName, usize>::new();
        for (index, step) in self.steps.iter().enumerate() {
            for binding in step.action.referenced_bindings() {
                if !defined.contains_key(binding) {
                    return Err(MacroError::UndefinedBinding {
                        step: index + 1,
                        binding: binding.clone(),
                    });
                }
            }
            if let Some(binding) = &step.bind {
                if !step.action.produces_result() {
                    return Err(MacroError::ActionHasNoResult { step: index + 1 });
                }
                if defined.insert(binding.clone(), index + 1).is_some() {
                    return Err(MacroError::DuplicateBinding(binding.clone()));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MacroStepV1 {
    pub action: SemanticActionV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_control: Option<ControlAddress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<MacroBindingName>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct MacroBindingName(String);

impl MacroBindingName {
    pub fn new(value: impl Into<String>) -> Result<Self, MacroError> {
        let value = value.into();
        let mut characters = value.chars();
        if characters.next() != Some('$')
            || !characters
                .next()
                .is_some_and(|character| character.is_ascii_lowercase())
            || !characters.all(|character| {
                character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
            })
        {
            return Err(MacroError::InvalidBinding(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MacroBindingName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl<'de> Deserialize<'de> for MacroBindingName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MacroInstanceSelector {
    Literal(InstanceId),
    Binding { binding: MacroBindingName },
    Title { title: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MacroResourceSelector {
    Literal(ResourceId),
    Binding { binding: MacroBindingName },
    Title { title: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum MacroViewSelector {
    Literal(ViewInstanceId),
    Binding { binding: MacroBindingName },
    Title { title: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroBoundValue {
    Chart(InstanceId),
    Resource(ResourceId),
    View(ViewInstanceId),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MacroBindings(BTreeMap<MacroBindingName, MacroBoundValue>);

impl MacroBindings {
    pub fn insert(
        &mut self,
        name: MacroBindingName,
        value: MacroBoundValue,
    ) -> Result<(), MacroError> {
        if self.0.insert(name.clone(), value).is_some() {
            return Err(MacroError::DuplicateBinding(name));
        }
        Ok(())
    }

    fn get(&self, name: &MacroBindingName) -> Result<MacroBoundValue, MacroError> {
        self.0
            .get(name)
            .copied()
            .ok_or_else(|| MacroError::MissingBinding(name.clone()))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SemanticActionV1 {
    BeginNewChart,
    BeginSavedChartEdit {
        chart: MacroInstanceSelector,
    },
    NewWorkspace,
    OpenWorkspace {
        workspace: MacroResourceSelector,
    },
    RenameWorkspace {
        title: String,
    },
    DiscardWorkspaceChanges,
    ResolveWorkspaceSwitch {
        resolution: WorkspaceSwitchAction,
    },
    LoadDemoBundle,
    SaveChartEditor,
    CancelChartEditor,
    SetChartTitle {
        title: String,
    },
    SetChartEventKind {
        event_kind: EventKind,
    },
    SetChartSubjectName {
        subject_name: Option<String>,
    },
    SetChartCivilDate {
        date: CivilDate,
    },
    SetChartCivilTime {
        time: CivilTime,
    },
    SetChartTimezone {
        timezone: ChartTimezone,
    },
    SetChartLocationEnabled {
        enabled: bool,
    },
    SetChartLocationName {
        name: String,
    },
    SetChartCountryRegion {
        country_region: Option<String>,
    },
    SetChartLatitude {
        latitude: Option<Latitude>,
    },
    SetChartLongitude {
        longitude: Option<Longitude>,
    },
    SetChartZodiac {
        zodiac: ZodiacSpec,
    },
    SetChartHouses {
        houses: HouseSystem,
    },
    SetChartCoordinates {
        coordinates: CoordinateSystem,
    },
    OpenChart {
        chart: MacroResourceSelector,
    },
    CloseChart {
        chart: MacroInstanceSelector,
    },
    ActivateChart {
        chart: MacroInstanceSelector,
    },
    SetChartSelection {
        chart: MacroInstanceSelector,
        selected: bool,
    },
    SetActiveView {
        view: MacroViewSelector,
    },
    AssignChartSlot {
        view: MacroViewSelector,
        slot: ChartSlotId,
        chart: Option<MacroInstanceSelector>,
    },
    BeginAspectSetEdit {
        aspect_set: MacroResourceSelector,
    },
    BeginNewAspectSet,
    DuplicateAspectSet {
        aspect_set: MacroResourceSelector,
    },
    SetWorkspaceAspectSet {
        aspect_set: MacroResourceSelector,
    },
    UpdateAspectEnabled {
        aspect_id: AspectId,
        enabled: bool,
    },
    UpdateAspectOrb {
        aspect_id: AspectId,
        degrees: f64,
    },
    SetAspectTitle {
        title: String,
    },
    InsertAspect {
        after: Option<AspectId>,
        aspect: mirabile_core::AspectDefinition,
    },
    UpdateAspect {
        aspect_id: AspectId,
        aspect: mirabile_core::AspectDefinition,
    },
    RemoveAspect {
        aspect_id: AspectId,
    },
    MoveAspect {
        aspect_id: AspectId,
        before: Option<AspectId>,
    },
    SaveDraft,
    CancelDraft,
    SaveWorkspace,
    SetTemporaryPointHidden {
        point_id: PointId,
        hidden: bool,
    },
    PromoteTemporaryDisplay,
    RefreshActiveView,
    BeginResourceCreate {
        kind: crate::ResourceDraftKind,
    },
    BeginResourceEdit {
        resource: MacroResourceSelector,
    },
    SetResourceMetadata {
        kind: crate::ResourceDraftKind,
        mutation: crate::ResourceMetadataMutation,
    },
    SaveResourceDraft {
        kind: crate::ResourceDraftKind,
    },
    CancelResourceDraft {
        kind: crate::ResourceDraftKind,
    },
}

impl SemanticActionV1 {
    #[allow(clippy::too_many_lines)]
    pub fn capture(
        intent: &AppIntent,
        model: &AppReadModel,
        bindings: &MacroBindings,
    ) -> Result<Self, MacroError> {
        Ok(match intent {
            AppIntent::BeginNewChart => Self::BeginNewChart,
            AppIntent::BeginSavedChartEdit { instance_id } => Self::BeginSavedChartEdit {
                chart: captured_chart(*instance_id, model, bindings),
            },
            AppIntent::ApplyChartMutation(mutation) => match mutation {
                ChartMutation::SetTitle(title) => Self::SetChartTitle {
                    title: title.clone(),
                },
                ChartMutation::SetEventKind(event_kind) => Self::SetChartEventKind {
                    event_kind: event_kind.clone(),
                },
                ChartMutation::SetSubjectName(subject_name) => Self::SetChartSubjectName {
                    subject_name: subject_name.clone(),
                },
                ChartMutation::SetCivilDate(date) => Self::SetChartCivilDate { date: *date },
                ChartMutation::SetCivilTime(time) => Self::SetChartCivilTime { time: *time },
                ChartMutation::SetTimezone(timezone) => Self::SetChartTimezone {
                    timezone: *timezone,
                },
                ChartMutation::SetLocationEnabled(enabled) => {
                    Self::SetChartLocationEnabled { enabled: *enabled }
                }
                ChartMutation::SetLocationName(name) => {
                    Self::SetChartLocationName { name: name.clone() }
                }
                ChartMutation::SetCountryRegion(country_region) => Self::SetChartCountryRegion {
                    country_region: country_region.clone(),
                },
                ChartMutation::SetLatitude(latitude) => Self::SetChartLatitude {
                    latitude: *latitude,
                },
                ChartMutation::SetLongitude(longitude) => Self::SetChartLongitude {
                    longitude: *longitude,
                },
                ChartMutation::SetZodiac(zodiac) => Self::SetChartZodiac {
                    zodiac: zodiac.clone(),
                },
                ChartMutation::SetHouseSystem(houses) => Self::SetChartHouses { houses: *houses },
                ChartMutation::SetCoordinateSystem(coordinates) => Self::SetChartCoordinates {
                    coordinates: *coordinates,
                },
                ChartMutation::SetRecordDetails(_)
                | ChartMutation::SetCalculation(_)
                | ChartMutation::Notes(_)
                | ChartMutation::LifeEvents(_)
                | ChartMutation::LifeEventNotes { .. } => {
                    return Err(MacroError::UnsupportedIntent(intent.semantic_summary()));
                }
            },
            AppIntent::SaveChartEditor => Self::SaveChartEditor,
            AppIntent::CancelChartEditor => Self::CancelChartEditor,
            AppIntent::OpenChart { definition_id } => Self::OpenChart {
                chart: captured_chart_resource(*definition_id, model, bindings),
            },
            AppIntent::CloseChart { instance_id } => Self::CloseChart {
                chart: captured_chart(*instance_id, model, bindings),
            },
            AppIntent::ActivateChart { instance_id } => Self::ActivateChart {
                chart: captured_chart(*instance_id, model, bindings),
            },
            AppIntent::SetChartSelection {
                instance_id,
                selected,
            } => Self::SetChartSelection {
                chart: captured_chart(*instance_id, model, bindings),
                selected: *selected,
            },
            AppIntent::SetActiveView { view_id } => Self::SetActiveView {
                view: captured_view(*view_id, model, bindings),
            },
            AppIntent::AssignChartSlot {
                view_id,
                slot,
                chart,
            } => Self::AssignChartSlot {
                view: captured_view(*view_id, model, bindings),
                slot: slot.clone(),
                chart: chart.map(|chart| captured_chart(chart, model, bindings)),
            },
            AppIntent::SetWorkspaceAspectSet { resource_id } => Self::SetWorkspaceAspectSet {
                aspect_set: captured_aspect_set(*resource_id, model, bindings),
            },
            AppIntent::NewWorkspace => Self::NewWorkspace,
            AppIntent::OpenWorkspace { resource_id } => Self::OpenWorkspace {
                workspace: captured_workspace(*resource_id, model, bindings),
            },
            AppIntent::RenameWorkspace { title } => Self::RenameWorkspace {
                title: title.clone(),
            },
            AppIntent::DiscardWorkspaceChanges => Self::DiscardWorkspaceChanges,
            AppIntent::ResolveWorkspaceSwitch { action } => Self::ResolveWorkspaceSwitch {
                resolution: *action,
            },
            AppIntent::LoadDemoBundle => Self::LoadDemoBundle,
            AppIntent::SaveWorkspace => Self::SaveWorkspace,
            AppIntent::SetTemporaryPointHidden { point_id, hidden } => {
                Self::SetTemporaryPointHidden {
                    point_id: point_id.clone(),
                    hidden: *hidden,
                }
            }
            AppIntent::PromoteTemporaryDisplay => Self::PromoteTemporaryDisplay,
            AppIntent::BeginAspectSetEdit { resource_id } => Self::BeginAspectSetEdit {
                aspect_set: captured_aspect_set(*resource_id, model, bindings),
            },
            AppIntent::BeginNewAspectSet => Self::BeginNewAspectSet,
            AppIntent::DuplicateAspectSet { resource_id } => Self::DuplicateAspectSet {
                aspect_set: captured_aspect_set(*resource_id, model, bindings),
            },
            AppIntent::UpdateAspectSetDraft(mutation) => match mutation {
                AspectSetDraftMutation::SetTitle(title) => Self::SetAspectTitle {
                    title: title.clone(),
                },
                AspectSetDraftMutation::SetOrb { aspect_id, maximum } => Self::UpdateAspectOrb {
                    aspect_id: aspect_id.clone(),
                    degrees: maximum.degrees(),
                },
                AspectSetDraftMutation::SetEnabled { aspect_id, enabled } => {
                    Self::UpdateAspectEnabled {
                        aspect_id: aspect_id.clone(),
                        enabled: *enabled,
                    }
                }
                AspectSetDraftMutation::Insert { after, aspect } => Self::InsertAspect {
                    after: after.clone(),
                    aspect: aspect.clone(),
                },
                AspectSetDraftMutation::Update { aspect_id, aspect } => Self::UpdateAspect {
                    aspect_id: aspect_id.clone(),
                    aspect: aspect.clone(),
                },
                AspectSetDraftMutation::Remove { aspect_id } => Self::RemoveAspect {
                    aspect_id: aspect_id.clone(),
                },
                AspectSetDraftMutation::Move { aspect_id, before } => Self::MoveAspect {
                    aspect_id: aspect_id.clone(),
                    before: before.clone(),
                },
            },
            AppIntent::SaveDraft => Self::SaveDraft,
            AppIntent::CancelDraft => Self::CancelDraft,
            AppIntent::RefreshActiveView => Self::RefreshActiveView,
            AppIntent::BeginResourceCreate { kind } => Self::BeginResourceCreate { kind: *kind },
            AppIntent::BeginResourceEdit { resource_id } => Self::BeginResourceEdit {
                resource: captured_any_resource(*resource_id, model, bindings),
            },
            AppIntent::ApplyResourceMutation(mutation) => {
                let kind = mutation.kind();
                let metadata = match mutation.as_ref() {
                    crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::Metadata(
                        value,
                    ))
                    | crate::ResourceMutation::ChartDefinition(
                        crate::ChartDefinitionMutation::Metadata(value),
                    )
                    | crate::ResourceMutation::PointSet(crate::PointSetMutation::Metadata(value))
                    | crate::ResourceMutation::AspectSet(crate::AspectSetMutation::Metadata(
                        value,
                    ))
                    | crate::ResourceMutation::AnalysisProfile(
                        crate::AnalysisProfileMutation::Metadata(value),
                    )
                    | crate::ResourceMutation::WheelTemplate(
                        crate::WheelTemplateMutation::Metadata(value),
                    )
                    | crate::ResourceMutation::ViewDocument(
                        crate::ViewDocumentMutation::Metadata(value),
                    )
                    | crate::ResourceMutation::Theme(crate::ThemeMutation::Metadata(value))
                    | crate::ResourceMutation::QueryDefinition(
                        crate::QueryDefinitionMutation::Metadata(value),
                    )
                    | crate::ResourceMutation::WorkspaceDocument(
                        crate::WorkspaceDocumentMutation::Metadata(value),
                    ) => value.clone(),
                    _ => return Err(MacroError::UnsupportedIntent(intent.semantic_summary())),
                };
                Self::SetResourceMetadata {
                    kind,
                    mutation: metadata,
                }
            }
            AppIntent::SaveResourceDraft { kind } => Self::SaveResourceDraft { kind: *kind },
            AppIntent::CancelResourceDraft { kind } => Self::CancelResourceDraft { kind: *kind },
            AppIntent::StartChartDraft { .. }
            | AppIntent::SaveChartDraft { .. }
            | AppIntent::CancelChartDraft { .. }
            | AppIntent::SelectRepositoryResource { .. }
            | AppIntent::BeginDeleteResource { .. }
            | AppIntent::ConfirmDeleteResource { .. }
            | AppIntent::SetWorkspaceBinding { .. }
            | AppIntent::ApplyWorkspaceComposition(_) => {
                return Err(MacroError::UnsupportedIntent(intent.semantic_summary()));
            }
        })
    }

    #[allow(clippy::too_many_lines)]
    pub fn resolve(
        &self,
        model: &AppReadModel,
        bindings: &MacroBindings,
    ) -> Result<AppIntent, MacroError> {
        Ok(match self {
            Self::BeginNewChart => AppIntent::BeginNewChart,
            Self::BeginSavedChartEdit { chart } => AppIntent::BeginSavedChartEdit {
                instance_id: resolve_chart(chart, model, bindings)?,
            },
            Self::NewWorkspace => AppIntent::NewWorkspace,
            Self::OpenWorkspace { workspace } => AppIntent::OpenWorkspace {
                resource_id: resolve_resource(workspace, bindings, |title| {
                    unique_title(
                        title,
                        model
                            .library
                            .workspaces
                            .iter()
                            .map(|item| (item.title.as_str(), item.resource_id)),
                        "workspace",
                    )
                })?,
            },
            Self::RenameWorkspace { title } => AppIntent::RenameWorkspace {
                title: title.clone(),
            },
            Self::DiscardWorkspaceChanges => AppIntent::DiscardWorkspaceChanges,
            Self::ResolveWorkspaceSwitch { resolution } => AppIntent::ResolveWorkspaceSwitch {
                action: *resolution,
            },
            Self::LoadDemoBundle => AppIntent::LoadDemoBundle,
            Self::SaveChartEditor => AppIntent::SaveChartEditor,
            Self::CancelChartEditor => AppIntent::CancelChartEditor,
            Self::SetChartTitle { title } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetTitle(title.clone()))
            }
            Self::SetChartEventKind { event_kind } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetEventKind(event_kind.clone()))
            }
            Self::SetChartSubjectName { subject_name } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetSubjectName(subject_name.clone()))
            }
            Self::SetChartCivilDate { date } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetCivilDate(*date))
            }
            Self::SetChartCivilTime { time } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetCivilTime(*time))
            }
            Self::SetChartTimezone { timezone } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetTimezone(*timezone))
            }
            Self::SetChartLocationEnabled { enabled } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetLocationEnabled(*enabled))
            }
            Self::SetChartLocationName { name } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetLocationName(name.clone()))
            }
            Self::SetChartCountryRegion { country_region } => AppIntent::ApplyChartMutation(
                ChartMutation::SetCountryRegion(country_region.clone()),
            ),
            Self::SetChartLatitude { latitude } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetLatitude(*latitude))
            }
            Self::SetChartLongitude { longitude } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetLongitude(*longitude))
            }
            Self::SetChartZodiac { zodiac } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetZodiac(zodiac.clone()))
            }
            Self::SetChartHouses { houses } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetHouseSystem(*houses))
            }
            Self::SetChartCoordinates { coordinates } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetCoordinateSystem(*coordinates))
            }
            Self::OpenChart { chart } => AppIntent::OpenChart {
                definition_id: resolve_resource(chart, bindings, |title| {
                    unique_title(
                        title,
                        model
                            .library
                            .charts
                            .iter()
                            .map(|item| (item.title.as_str(), item.definition_id)),
                        "saved chart",
                    )
                })?,
            },
            Self::CloseChart { chart } => AppIntent::CloseChart {
                instance_id: resolve_chart(chart, model, bindings)?,
            },
            Self::ActivateChart { chart } => AppIntent::ActivateChart {
                instance_id: resolve_chart(chart, model, bindings)?,
            },
            Self::SetChartSelection { chart, selected } => AppIntent::SetChartSelection {
                instance_id: resolve_chart(chart, model, bindings)?,
                selected: *selected,
            },
            Self::SetActiveView { view } => AppIntent::SetActiveView {
                view_id: resolve_view(view, model, bindings)?,
            },
            Self::AssignChartSlot { view, slot, chart } => AppIntent::AssignChartSlot {
                view_id: resolve_view(view, model, bindings)?,
                slot: slot.clone(),
                chart: chart
                    .as_ref()
                    .map(|chart| resolve_chart(chart, model, bindings))
                    .transpose()?,
            },
            Self::BeginAspectSetEdit { aspect_set } => AppIntent::BeginAspectSetEdit {
                resource_id: resolve_aspect_set(aspect_set, model, bindings)?,
            },
            Self::BeginNewAspectSet => AppIntent::BeginNewAspectSet,
            Self::DuplicateAspectSet { aspect_set } => AppIntent::DuplicateAspectSet {
                resource_id: resolve_aspect_set(aspect_set, model, bindings)?,
            },
            Self::SetWorkspaceAspectSet { aspect_set } => AppIntent::SetWorkspaceAspectSet {
                resource_id: resolve_aspect_set(aspect_set, model, bindings)?,
            },
            Self::UpdateAspectEnabled { aspect_id, enabled } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
                    aspect_id: aspect_id.clone(),
                    enabled: *enabled,
                })
            }
            Self::UpdateAspectOrb { aspect_id, degrees } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetOrb {
                    aspect_id: aspect_id.clone(),
                    maximum: crate::Angle::from_degrees(*degrees)
                        .map_err(|error| MacroError::InvalidValue(error.to_string()))?,
                })
            }
            Self::SetAspectTitle { title } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::SetTitle(title.clone()))
            }
            Self::InsertAspect { after, aspect } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::Insert {
                    after: after.clone(),
                    aspect: aspect.clone(),
                })
            }
            Self::UpdateAspect { aspect_id, aspect } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::Update {
                    aspect_id: aspect_id.clone(),
                    aspect: aspect.clone(),
                })
            }
            Self::RemoveAspect { aspect_id } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::Remove {
                    aspect_id: aspect_id.clone(),
                })
            }
            Self::MoveAspect { aspect_id, before } => {
                AppIntent::UpdateAspectSetDraft(AspectSetDraftMutation::Move {
                    aspect_id: aspect_id.clone(),
                    before: before.clone(),
                })
            }
            Self::SaveDraft => AppIntent::SaveDraft,
            Self::CancelDraft => AppIntent::CancelDraft,
            Self::SaveWorkspace => AppIntent::SaveWorkspace,
            Self::SetTemporaryPointHidden { point_id, hidden } => {
                AppIntent::SetTemporaryPointHidden {
                    point_id: point_id.clone(),
                    hidden: *hidden,
                }
            }
            Self::PromoteTemporaryDisplay => AppIntent::PromoteTemporaryDisplay,
            Self::RefreshActiveView => AppIntent::RefreshActiveView,
            Self::BeginResourceCreate { kind } => AppIntent::BeginResourceCreate { kind: *kind },
            Self::BeginResourceEdit { resource } => AppIntent::BeginResourceEdit {
                resource_id: resolve_any_resource(resource, model, bindings)?,
            },
            Self::SetResourceMetadata { kind, mutation } => AppIntent::ApplyResourceMutation(
                Box::new(metadata_resource_mutation(*kind, mutation.clone())),
            ),
            Self::SaveResourceDraft { kind } => AppIntent::SaveResourceDraft { kind: *kind },
            Self::CancelResourceDraft { kind } => AppIntent::CancelResourceDraft { kind: *kind },
        })
    }

    pub fn produces_result(&self) -> bool {
        matches!(
            self,
            Self::BeginNewChart
                | Self::OpenChart { .. }
                | Self::OpenWorkspace { .. }
                | Self::SaveChartEditor
                | Self::SaveDraft
                | Self::SaveWorkspace
                | Self::SaveResourceDraft { .. }
        )
    }

    pub fn capture_result(&self, model: &AppReadModel) -> Result<MacroBoundValue, MacroError> {
        match self {
            Self::BeginNewChart | Self::OpenChart { .. } | Self::SaveChartEditor => model
                .workspace
                .active_chart
                .map(MacroBoundValue::Chart)
                .ok_or(MacroError::MissingResult("active chart")),
            Self::OpenWorkspace { .. } | Self::SaveWorkspace => model
                .workspace
                .document_id
                .map(MacroBoundValue::Resource)
                .ok_or(MacroError::MissingResult("saved workspace")),
            Self::SaveDraft => model
                .resource_editor
                .aspect_set
                .as_ref()
                .and_then(|draft| draft.resource_id)
                .map(MacroBoundValue::Resource)
                .ok_or(MacroError::MissingResult("saved Aspect Set")),
            Self::SaveResourceDraft { kind } => model
                .resource_editor
                .drafts
                .iter()
                .find(|draft| draft.kind == *kind)
                .and_then(|draft| draft.resource_id)
                .map(MacroBoundValue::Resource)
                .ok_or(MacroError::MissingResult("saved canonical resource")),
            _ => Err(MacroError::MissingResult("action result")),
        }
    }

    fn referenced_bindings(&self) -> Vec<&MacroBindingName> {
        let mut bindings = Vec::new();
        match self {
            Self::BeginSavedChartEdit { chart }
            | Self::CloseChart { chart }
            | Self::ActivateChart { chart }
            | Self::SetChartSelection { chart, .. } => instance_binding(chart, &mut bindings),
            Self::OpenWorkspace { workspace } => resource_binding(workspace, &mut bindings),
            Self::OpenChart { chart } => resource_binding(chart, &mut bindings),
            Self::SetActiveView { view } => view_binding(view, &mut bindings),
            Self::AssignChartSlot { view, chart, .. } => {
                view_binding(view, &mut bindings);
                if let Some(chart) = chart {
                    instance_binding(chart, &mut bindings);
                }
            }
            Self::BeginAspectSetEdit { aspect_set }
            | Self::DuplicateAspectSet { aspect_set }
            | Self::SetWorkspaceAspectSet { aspect_set } => {
                resource_binding(aspect_set, &mut bindings);
            }
            Self::BeginResourceEdit { resource } => resource_binding(resource, &mut bindings),
            _ => {}
        }
        bindings
    }
}

#[derive(Clone, Debug)]
pub struct MacroRecorder {
    name: String,
    steps: Vec<MacroStepV1>,
    bindings: MacroBindings,
    next_chart_binding: usize,
    next_resource_binding: usize,
}

impl MacroRecorder {
    pub fn new(name: impl Into<String>) -> Result<Self, MacroError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(MacroError::InvalidName);
        }
        Ok(Self {
            name,
            steps: Vec::new(),
            bindings: MacroBindings::default(),
            next_chart_binding: 1,
            next_resource_binding: 1,
        })
    }

    pub fn capture(
        &mut self,
        intent: &AppIntent,
        origin_control: Option<ControlAddress>,
        settled_model: &AppReadModel,
    ) -> Result<(), MacroError> {
        if self.steps.len() == MACRO_STEP_LIMIT {
            return Err(MacroError::TooManySteps(self.steps.len() + 1));
        }
        let action = SemanticActionV1::capture(intent, settled_model, &self.bindings)?;
        let bind = if matches!(
            action,
            SemanticActionV1::BeginNewChart | SemanticActionV1::OpenChart { .. }
        ) {
            Some(self.next_binding("chart")?)
        } else if matches!(
            action,
            SemanticActionV1::SaveDraft | SemanticActionV1::SaveWorkspace
        ) && action.capture_result(settled_model).is_ok()
        {
            Some(self.next_binding("resource")?)
        } else {
            None
        };
        if let Some(binding) = &bind {
            let result = action.capture_result(settled_model)?;
            self.bindings.insert(binding.clone(), result)?;
        }
        self.steps.push(MacroStepV1 {
            action,
            origin_control,
            bind,
        });
        Ok(())
    }

    pub fn finish(self) -> Result<MacroDocumentV1, MacroError> {
        MacroDocumentV1::new(self.name, self.steps)
    }

    pub fn steps(&self) -> &[MacroStepV1] {
        &self.steps
    }

    fn next_binding(&mut self, kind: &str) -> Result<MacroBindingName, MacroError> {
        let sequence = if kind == "chart" {
            let sequence = self.next_chart_binding;
            self.next_chart_binding = self.next_chart_binding.saturating_add(1);
            sequence
        } else {
            let sequence = self.next_resource_binding;
            self.next_resource_binding = self.next_resource_binding.saturating_add(1);
            sequence
        };
        MacroBindingName::new(format!("${kind}{sequence}"))
    }
}

fn captured_chart(
    id: InstanceId,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroInstanceSelector {
    bindings
        .0
        .iter()
        .find_map(|(binding, value)| {
            (*value == MacroBoundValue::Chart(id)).then(|| binding.clone())
        })
        .map_or_else(
            || {
                unique_recording_title(
                    model
                        .workspace
                        .charts
                        .iter()
                        .map(|item| (item.title.as_str(), item.instance_id)),
                    id,
                )
                .map_or(MacroInstanceSelector::Literal(id), |title| {
                    MacroInstanceSelector::Title { title }
                })
            },
            |binding| MacroInstanceSelector::Binding { binding },
        )
}

fn captured_chart_resource(
    id: ResourceId,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroResourceSelector {
    captured_resource(
        id,
        model
            .library
            .charts
            .iter()
            .map(|item| (item.title.as_str(), item.definition_id)),
        bindings,
    )
}

fn captured_workspace(
    id: ResourceId,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroResourceSelector {
    captured_resource(
        id,
        model
            .library
            .workspaces
            .iter()
            .map(|item| (item.title.as_str(), item.resource_id)),
        bindings,
    )
}

fn captured_aspect_set(
    id: ResourceId,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroResourceSelector {
    captured_resource(
        id,
        model
            .library
            .aspect_sets
            .iter()
            .map(|item| (item.title.as_str(), item.resource_id)),
        bindings,
    )
}

fn captured_resource<'a>(
    id: ResourceId,
    titles: impl Iterator<Item = (&'a str, ResourceId)>,
    bindings: &MacroBindings,
) -> MacroResourceSelector {
    bindings
        .0
        .iter()
        .find_map(|(binding, value)| {
            (*value == MacroBoundValue::Resource(id)).then(|| binding.clone())
        })
        .map_or_else(
            || {
                unique_recording_title(titles, id)
                    .map_or(MacroResourceSelector::Literal(id), |title| {
                        MacroResourceSelector::Title { title }
                    })
            },
            |binding| MacroResourceSelector::Binding { binding },
        )
}

fn captured_any_resource(
    id: ResourceId,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroResourceSelector {
    captured_resource(
        id,
        model.resources.inventories.iter().flat_map(|inventory| {
            inventory
                .resources
                .iter()
                .map(|resource| (resource.title.as_str(), resource.resource_id))
        }),
        bindings,
    )
}

fn resolve_any_resource(
    selector: &MacroResourceSelector,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<ResourceId, MacroError> {
    resolve_resource(selector, bindings, |title| {
        unique_title(
            title,
            model.resources.inventories.iter().flat_map(|inventory| {
                inventory
                    .resources
                    .iter()
                    .map(|resource| (resource.title.as_str(), resource.resource_id))
            }),
            "canonical resource",
        )
    })
}

fn metadata_resource_mutation(
    kind: crate::ResourceDraftKind,
    mutation: crate::ResourceMetadataMutation,
) -> crate::ResourceMutation {
    match kind {
        crate::ResourceDraftKind::ChartRecord => {
            crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::Metadata(mutation))
        }
        crate::ResourceDraftKind::ChartDefinition => crate::ResourceMutation::ChartDefinition(
            crate::ChartDefinitionMutation::Metadata(mutation),
        ),
        crate::ResourceDraftKind::PointSet => {
            crate::ResourceMutation::PointSet(crate::PointSetMutation::Metadata(mutation))
        }
        crate::ResourceDraftKind::AspectSet => {
            crate::ResourceMutation::AspectSet(crate::AspectSetMutation::Metadata(mutation))
        }
        crate::ResourceDraftKind::AnalysisProfile => crate::ResourceMutation::AnalysisProfile(
            crate::AnalysisProfileMutation::Metadata(mutation),
        ),
        crate::ResourceDraftKind::WheelTemplate => {
            crate::ResourceMutation::WheelTemplate(crate::WheelTemplateMutation::Metadata(mutation))
        }
        crate::ResourceDraftKind::ViewDocument => {
            crate::ResourceMutation::ViewDocument(crate::ViewDocumentMutation::Metadata(mutation))
        }
        crate::ResourceDraftKind::Theme => {
            crate::ResourceMutation::Theme(crate::ThemeMutation::Metadata(mutation))
        }
        crate::ResourceDraftKind::QueryDefinition => crate::ResourceMutation::QueryDefinition(
            crate::QueryDefinitionMutation::Metadata(mutation),
        ),
        crate::ResourceDraftKind::WorkspaceDocument => crate::ResourceMutation::WorkspaceDocument(
            crate::WorkspaceDocumentMutation::Metadata(mutation),
        ),
    }
}

fn captured_view(
    id: ViewInstanceId,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroViewSelector {
    bindings
        .0
        .iter()
        .find_map(|(binding, value)| (*value == MacroBoundValue::View(id)).then(|| binding.clone()))
        .map_or_else(
            || {
                unique_recording_title(
                    model
                        .workspace
                        .views
                        .iter()
                        .map(|item| (item.title.as_str(), item.view_id)),
                    id,
                )
                .map_or(MacroViewSelector::Literal(id), |title| {
                    MacroViewSelector::Title { title }
                })
            },
            |binding| MacroViewSelector::Binding { binding },
        )
}

fn unique_recording_title<'a, T: Copy + Eq>(
    items: impl Iterator<Item = (&'a str, T)>,
    target: T,
) -> Option<String> {
    let items = items.collect::<Vec<_>>();
    let title = items
        .iter()
        .find_map(|(title, id)| (*id == target).then_some(*title))?;
    (items
        .iter()
        .filter(|(candidate, _)| *candidate == title)
        .count()
        == 1)
        .then(|| title.to_owned())
}

fn instance_binding<'a>(
    selector: &'a MacroInstanceSelector,
    output: &mut Vec<&'a MacroBindingName>,
) {
    if let MacroInstanceSelector::Binding { binding } = selector {
        output.push(binding);
    }
}

fn resource_binding<'a>(
    selector: &'a MacroResourceSelector,
    output: &mut Vec<&'a MacroBindingName>,
) {
    if let MacroResourceSelector::Binding { binding } = selector {
        output.push(binding);
    }
}

fn view_binding<'a>(selector: &'a MacroViewSelector, output: &mut Vec<&'a MacroBindingName>) {
    if let MacroViewSelector::Binding { binding } = selector {
        output.push(binding);
    }
}

fn resolve_chart(
    selector: &MacroInstanceSelector,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<InstanceId, MacroError> {
    match selector {
        MacroInstanceSelector::Literal(id) => Ok(*id),
        MacroInstanceSelector::Binding { binding } => match bindings.get(binding)? {
            MacroBoundValue::Chart(id) => Ok(id),
            _ => Err(MacroError::BindingType {
                binding: binding.clone(),
                expected: "chart",
            }),
        },
        MacroInstanceSelector::Title { title } => unique_title(
            title,
            model
                .workspace
                .charts
                .iter()
                .map(|item| (item.title.as_str(), item.instance_id)),
            "open chart",
        ),
    }
}

fn resolve_resource(
    selector: &MacroResourceSelector,
    bindings: &MacroBindings,
    title: impl FnOnce(&str) -> Result<ResourceId, MacroError>,
) -> Result<ResourceId, MacroError> {
    match selector {
        MacroResourceSelector::Literal(id) => Ok(*id),
        MacroResourceSelector::Binding { binding } => match bindings.get(binding)? {
            MacroBoundValue::Resource(id) => Ok(id),
            _ => Err(MacroError::BindingType {
                binding: binding.clone(),
                expected: "resource",
            }),
        },
        MacroResourceSelector::Title { title: value } => title(value),
    }
}

fn resolve_aspect_set(
    selector: &MacroResourceSelector,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<ResourceId, MacroError> {
    resolve_resource(selector, bindings, |title| {
        unique_title(
            title,
            model
                .library
                .aspect_sets
                .iter()
                .map(|item| (item.title.as_str(), item.resource_id)),
            "Aspect Set",
        )
    })
}

fn resolve_view(
    selector: &MacroViewSelector,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<ViewInstanceId, MacroError> {
    match selector {
        MacroViewSelector::Literal(id) => Ok(*id),
        MacroViewSelector::Binding { binding } => match bindings.get(binding)? {
            MacroBoundValue::View(id) => Ok(id),
            _ => Err(MacroError::BindingType {
                binding: binding.clone(),
                expected: "view",
            }),
        },
        MacroViewSelector::Title { title } => unique_title(
            title,
            model
                .workspace
                .views
                .iter()
                .map(|item| (item.title.as_str(), item.view_id)),
            "view",
        ),
    }
}

fn unique_title<'a, T: Copy>(
    title: &str,
    items: impl Iterator<Item = (&'a str, T)>,
    kind: &'static str,
) -> Result<T, MacroError> {
    let mut matches = items.filter(|(candidate, _)| *candidate == title);
    let Some((_, value)) = matches.next() else {
        return Err(MacroError::TitleNotFound {
            kind,
            title: title.to_owned(),
        });
    };
    if matches.next().is_some() {
        return Err(MacroError::AmbiguousTitle {
            kind,
            title: title.to_owned(),
        });
    }
    Ok(value)
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum MacroError {
    #[error("macro JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("macro schema version {0} is unsupported")]
    UnsupportedVersion(u32),
    #[error("macro name must not be blank")]
    InvalidName,
    #[error("macro contains {0} steps; the limit is 512")]
    TooManySteps(usize),
    #[error("macro binding {0} must use $name syntax")]
    InvalidBinding(String),
    #[error("macro binding {0} is defined more than once")]
    DuplicateBinding(MacroBindingName),
    #[error("macro step {step} references undefined binding {binding}")]
    UndefinedBinding {
        step: usize,
        binding: MacroBindingName,
    },
    #[error("macro step {step} binds an action that has no result")]
    ActionHasNoResult { step: usize },
    #[error("macro binding {0} has not been resolved")]
    MissingBinding(MacroBindingName),
    #[error("macro binding {binding} does not contain a {expected}")]
    BindingType {
        binding: MacroBindingName,
        expected: &'static str,
    },
    #[error("{kind} titled {title:?} was not found")]
    TitleNotFound { kind: &'static str, title: String },
    #[error("{kind} title {title:?} is ambiguous")]
    AmbiguousTitle { kind: &'static str, title: String },
    #[error("macro action did not produce the expected {0}")]
    MissingResult(&'static str),
    #[error("macro value is invalid: {0}")]
    InvalidValue(String),
    #[error("application intent is not part of the macro whitelist: {0}")]
    UnsupportedIntent(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_rejects_forward_and_duplicate_bindings() {
        let chart = MacroBindingName::new("$chart1").expect("binding");
        let forward = MacroDocumentV1::new(
            "forward",
            vec![MacroStepV1 {
                action: SemanticActionV1::ActivateChart {
                    chart: MacroInstanceSelector::Binding {
                        binding: chart.clone(),
                    },
                },
                origin_control: None,
                bind: None,
            }],
        );
        assert!(matches!(forward, Err(MacroError::UndefinedBinding { .. })));

        let duplicate = MacroDocumentV1::new(
            "duplicate",
            vec![
                MacroStepV1 {
                    action: SemanticActionV1::BeginNewChart,
                    origin_control: None,
                    bind: Some(chart.clone()),
                },
                MacroStepV1 {
                    action: SemanticActionV1::BeginNewChart,
                    origin_control: None,
                    bind: Some(chart),
                },
            ],
        );
        assert!(matches!(duplicate, Err(MacroError::DuplicateBinding(_))));
    }

    #[test]
    fn macro_json_contains_no_selector_or_script_escape_hatches() {
        let document = MacroDocumentV1::new(
            "new chart",
            vec![MacroStepV1 {
                action: SemanticActionV1::BeginNewChart,
                origin_control: Some(ControlAddress::new(crate::ControlId::CHART_NEW)),
                bind: Some(MacroBindingName::new("$chart1").expect("binding")),
            }],
        )
        .expect("document");
        let json = serde_json::to_string(&document).expect("JSON");
        assert!(!json.contains("javascript"));
        assert!(!json.contains("selector"));
        assert!(!json.contains("coordinate"));
        assert_eq!(MacroDocumentV1::from_json(&json), Ok(document));
    }

    #[test]
    fn title_resolution_requires_exactly_one_match() {
        let mut model = AppReadModel::initializing();
        let first = InstanceId::new();
        let second = InstanceId::new();
        model.workspace.charts = vec![
            crate::OpenChartSummary {
                instance_id: first,
                title: "Repeated".into(),
                subtitle: String::new(),
                persistence: crate::ChartPersistence::Ephemeral,
            },
            crate::OpenChartSummary {
                instance_id: second,
                title: "Repeated".into(),
                subtitle: String::new(),
                persistence: crate::ChartPersistence::Ephemeral,
            },
        ];
        let action = SemanticActionV1::ActivateChart {
            chart: MacroInstanceSelector::Title {
                title: "Repeated".into(),
            },
        };
        assert!(matches!(
            action.resolve(&model, &MacroBindings::default()),
            Err(MacroError::AmbiguousTitle { .. })
        ));
    }

    #[test]
    fn recorder_binds_created_chart_and_reuses_symbolic_identity() {
        let mut model = AppReadModel::initializing();
        let chart = InstanceId::new();
        model.workspace.active_chart = Some(chart);
        model.workspace.charts = vec![crate::OpenChartSummary {
            instance_id: chart,
            title: "Recorded".into(),
            subtitle: String::new(),
            persistence: crate::ChartPersistence::Ephemeral,
        }];
        let mut recorder = MacroRecorder::new("portable chart").expect("recorder");
        recorder
            .capture(
                &AppIntent::BeginNewChart,
                Some(ControlAddress::new(crate::ControlId::CHART_NEW)),
                &model,
            )
            .expect("begin chart");
        recorder
            .capture(
                &AppIntent::ActivateChart { instance_id: chart },
                None,
                &model,
            )
            .expect("activate chart");
        let document = recorder.finish().expect("document");

        assert_eq!(
            document.steps[0]
                .bind
                .as_ref()
                .map(MacroBindingName::as_str),
            Some("$chart1")
        );
        assert!(matches!(
            &document.steps[1].action,
            SemanticActionV1::ActivateChart {
                chart: MacroInstanceSelector::Binding { binding }
            } if binding.as_str() == "$chart1"
        ));
    }

    #[test]
    fn macro_v1_additively_round_trips_typed_resource_lifecycle_and_metadata() {
        let model = AppReadModel::initializing();
        let bindings = MacroBindings::default();
        let create = SemanticActionV1::capture(
            &AppIntent::BeginResourceCreate {
                kind: crate::ResourceDraftKind::PointSet,
            },
            &model,
            &bindings,
        )
        .expect("capture create");
        let metadata = SemanticActionV1::capture(
            &AppIntent::ApplyResourceMutation(Box::new(crate::ResourceMutation::PointSet(
                crate::PointSetMutation::Metadata(crate::ResourceMetadataMutation::SetTitle(
                    "Macro points".into(),
                )),
            ))),
            &model,
            &bindings,
        )
        .expect("capture metadata");
        let document = MacroDocumentV1::new(
            "typed resource",
            vec![
                MacroStepV1 {
                    action: create,
                    origin_control: None,
                    bind: None,
                },
                MacroStepV1 {
                    action: metadata,
                    origin_control: None,
                    bind: None,
                },
            ],
        )
        .expect("document");
        let json = serde_json::to_string(&document).expect("json");
        assert_eq!(MacroDocumentV1::from_json(&json), Ok(document));
        assert!(!json.contains("draft_item"));
    }
}
