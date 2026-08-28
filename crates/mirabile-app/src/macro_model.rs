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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "by", rename_all = "snake_case")]
pub enum MacroListItemSelectorV1 {
    Key { collection: String, key: String },
    Ordinal { collection: String, ordinal: usize },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MacroListMutationV1<T> {
    Insert {
        after: Option<MacroListItemSelectorV1>,
        value: T,
    },
    Update {
        item: MacroListItemSelectorV1,
        value: T,
    },
    Remove {
        item: MacroListItemSelectorV1,
    },
    Move {
        item: MacroListItemSelectorV1,
        before: Option<MacroListItemSelectorV1>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MacroQueryNodeSelectorV1 {
    pub path: Vec<usize>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum MacroQueryTreeMutationV1 {
    Replace {
        node: MacroQueryNodeSelectorV1,
        expression: crate::QueryExpr,
    },
    InsertChild {
        parent: MacroQueryNodeSelectorV1,
        after: Option<MacroQueryNodeSelectorV1>,
        expression: crate::QueryExpr,
    },
    Remove {
        node: MacroQueryNodeSelectorV1,
    },
    Move {
        node: MacroQueryNodeSelectorV1,
        new_parent: MacroQueryNodeSelectorV1,
        before: Option<MacroQueryNodeSelectorV1>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "mutation", content = "value", rename_all = "snake_case")]
pub enum MacroResourceMutationV1 {
    ChartRecordEventKind(crate::EventKind),
    ChartRecordSubject(Option<mirabile_core::SubjectInfo>),
    ChartRecordTime(mirabile_core::TemporalAssertion),
    ChartRecordLocation(Option<mirabile_core::LocationAssertion>),
    ChartRecordSource(mirabile_core::SourceProvenance),
    ChartRecordNotes(MacroListMutationV1<mirabile_core::Note>),
    ChartRecordLifeEvents(MacroListMutationV1<mirabile_core::LifeEvent>),
    ChartRecordLifeEventNotes {
        life_event: MacroListItemSelectorV1,
        mutation: MacroListMutationV1<mirabile_core::Note>,
    },
    ChartDefinitionSource(mirabile_core::ChartSource),
    ChartDefinitionRecipe(MacroDerivedRecipeMutationV1),
    ChartDefinitionCalculation(mirabile_core::CalculationSpec),
    PointSetSelectors(MacroListMutationV1<mirabile_core::PointSelector>),
    AspectSetAspects(MacroListMutationV1<mirabile_core::AspectDefinition>),
    AnalysisProfile(mirabile_core::AnalysisProfile),
    WheelTemplateRings(MacroListMutationV1<mirabile_core::RingSpec>),
    WheelTemplateFields(mirabile_core::WheelTemplate),
    ViewDocumentChartSlots(MacroListMutationV1<mirabile_core::ChartSlot>),
    ViewDocumentRenameChartSlot {
        item: MacroListItemSelectorV1,
        slot: mirabile_core::ChartSlot,
    },
    ViewDocumentObjects(MacroListMutationV1<mirabile_core::ViewObject>),
    ViewDocumentLayout(mirabile_core::PageLayout),
    Theme(mirabile_core::Theme),
    QueryDescription(Option<String>),
    QueryTree(MacroQueryTreeMutationV1),
    WorkspaceCharts(MacroListMutationV1<mirabile_core::WorkspaceDocumentChart>),
    WorkspaceViews(MacroListMutationV1<mirabile_core::ViewInstance>),
    WorkspaceProfile(Box<mirabile_core::WorkspaceProfile>),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "recipe", rename_all = "snake_case")]
pub enum MacroDerivedRecipeMutationV1 {
    Transit {
        at: mirabile_core::TemporalAssertion,
        location: mirabile_core::LocationAssertion,
    },
    Harmonic {
        radix: MacroResourceSelector,
        harmonic: f64,
    },
    Relocation {
        radix: MacroResourceSelector,
        location: mirabile_core::LocationAssertion,
    },
    CompositeMethod {
        method: mirabile_core::CompositeMethod,
    },
    CompositeCharts(MacroListMutationV1<ResourceId>),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case")]
pub enum MacroWorkspaceBindingSlotV1 {
    DisplayedPoints,
    AspectedPoints,
    TransitPoints,
    Aspects,
    Analysis,
    Theme,
    Wheel,
    ViewDocument { view: MacroViewSelector },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum MacroWorkspaceBindingSelectionV1 {
    Follow {
        resource: MacroResourceSelector,
    },
    Pinned {
        resource: MacroResourceSelector,
        revision: crate::Revision,
    },
    Inline {
        resource: MacroResourceSelector,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "mutation", rename_all = "snake_case")]
pub enum MacroWorkspaceCompositionMutationV1 {
    MoveChart {
        chart: MacroInstanceSelector,
        before: Option<MacroInstanceSelector>,
    },
    AddView {
        document: MacroWorkspaceBindingSelectionV1,
    },
    RemoveView {
        view: MacroViewSelector,
    },
    MoveView {
        view: MacroViewSelector,
        before: Option<MacroViewSelector>,
    },
    SetRotation {
        view: MacroViewSelector,
        rotation: Option<crate::Angle>,
    },
    SetPointHidden {
        view: MacroViewSelector,
        point_id: PointId,
        hidden: bool,
    },
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
    SetChartRecordDetails {
        record: Box<mirabile_core::ChartRecord>,
    },
    SetChartCalculation {
        calculation: mirabile_core::CalculationSpec,
    },
    MutateChartNotes {
        mutation: MacroListMutationV1<mirabile_core::Note>,
    },
    MutateChartLifeEvents {
        mutation: MacroListMutationV1<mirabile_core::LifeEvent>,
    },
    MutateChartLifeEventNotes {
        life_event: MacroListItemSelectorV1,
        mutation: MacroListMutationV1<mirabile_core::Note>,
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
    SetWorkspaceBinding {
        slot: MacroWorkspaceBindingSlotV1,
        selection: MacroWorkspaceBindingSelectionV1,
    },
    MutateWorkspaceComposition {
        mutation: MacroWorkspaceCompositionMutationV1,
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
    MutateResource {
        kind: crate::ResourceDraftKind,
        mutation: Box<MacroResourceMutationV1>,
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
                ChartMutation::SetRecordDetails(record) => Self::SetChartRecordDetails {
                    record: record.clone(),
                },
                ChartMutation::SetCalculation(calculation) => Self::SetChartCalculation {
                    calculation: calculation.clone(),
                },
                ChartMutation::Notes(mutation) => {
                    let editor = chart_editor(model)?;
                    Self::MutateChartNotes {
                        mutation: capture_list_mutation(
                            mutation,
                            &editor.notes,
                            "chart.notes",
                            |_| None,
                        )?,
                    }
                }
                ChartMutation::LifeEvents(mutation) => {
                    let editor = chart_editor(model)?;
                    let rows = life_event_rows(&editor.life_events);
                    Self::MutateChartLifeEvents {
                        mutation: capture_list_mutation(
                            mutation,
                            &rows,
                            "chart.life_events",
                            |_| None,
                        )?,
                    }
                }
                ChartMutation::LifeEventNotes {
                    life_event_id,
                    mutation,
                } => {
                    let editor = chart_editor(model)?;
                    let rows = life_event_rows(&editor.life_events);
                    let life_event =
                        capture_list_selector(*life_event_id, &rows, "chart.life_events", |_| {
                            None
                        })?;
                    let event = editor
                        .life_events
                        .iter()
                        .find(|event| event.item_id == *life_event_id)
                        .ok_or_else(|| topology("chart life event was not found"))?;
                    Self::MutateChartLifeEventNotes {
                        life_event,
                        mutation: capture_list_mutation(
                            mutation,
                            &event.notes,
                            "chart.life_event.notes",
                            |_| None,
                        )?,
                    }
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
            AppIntent::SetWorkspaceBinding { slot, selection } => Self::SetWorkspaceBinding {
                slot: capture_workspace_binding_slot(*slot, model, bindings),
                selection: capture_workspace_binding_selection(*selection, model, bindings),
            },
            AppIntent::ApplyWorkspaceComposition(mutation) => Self::MutateWorkspaceComposition {
                mutation: capture_workspace_composition(mutation, model, bindings),
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
                    _ => {
                        return Ok(Self::MutateResource {
                            kind,
                            mutation: Box::new(capture_resource_mutation(
                                mutation, model, bindings,
                            )?),
                        });
                    }
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
            | AppIntent::ConfirmDeleteResource { .. } => {
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
            Self::SetChartRecordDetails { record } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetRecordDetails(record.clone()))
            }
            Self::SetChartCalculation { calculation } => {
                AppIntent::ApplyChartMutation(ChartMutation::SetCalculation(calculation.clone()))
            }
            Self::MutateChartNotes { mutation } => {
                let editor = chart_editor(model)?;
                AppIntent::ApplyChartMutation(ChartMutation::Notes(resolve_list_mutation(
                    mutation,
                    &editor.notes,
                    "chart.notes",
                    |_| None,
                )?))
            }
            Self::MutateChartLifeEvents { mutation } => {
                let editor = chart_editor(model)?;
                let rows = life_event_rows(&editor.life_events);
                AppIntent::ApplyChartMutation(ChartMutation::LifeEvents(resolve_list_mutation(
                    mutation,
                    &rows,
                    "chart.life_events",
                    |_| None,
                )?))
            }
            Self::MutateChartLifeEventNotes {
                life_event,
                mutation,
            } => {
                let editor = chart_editor(model)?;
                let rows = life_event_rows(&editor.life_events);
                let life_event_id =
                    resolve_list_selector(life_event, &rows, "chart.life_events", |_| None)?;
                let event = editor
                    .life_events
                    .iter()
                    .find(|event| event.item_id == life_event_id)
                    .ok_or_else(|| topology("chart life event topology changed"))?;
                AppIntent::ApplyChartMutation(ChartMutation::LifeEventNotes {
                    life_event_id,
                    mutation: resolve_list_mutation(
                        mutation,
                        &event.notes,
                        "chart.life_event.notes",
                        |_| None,
                    )?,
                })
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
            Self::SetWorkspaceBinding { slot, selection } => AppIntent::SetWorkspaceBinding {
                slot: resolve_workspace_binding_slot(slot, model, bindings)?,
                selection: resolve_workspace_binding_selection(selection, model, bindings)?,
            },
            Self::MutateWorkspaceComposition { mutation } => AppIntent::ApplyWorkspaceComposition(
                resolve_workspace_composition(mutation, model, bindings)?,
            ),
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
            Self::MutateResource { kind, mutation } => AppIntent::ApplyResourceMutation(Box::new(
                resolve_resource_mutation(*kind, mutation, model, bindings)?,
            )),
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
                | Self::MutateWorkspaceComposition {
                    mutation: MacroWorkspaceCompositionMutationV1::AddView { .. }
                }
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
            Self::MutateWorkspaceComposition {
                mutation: MacroWorkspaceCompositionMutationV1::AddView { .. },
            } => model
                .workspace
                .active_view
                .map(MacroBoundValue::View)
                .ok_or(MacroError::MissingResult("created view")),
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
            Self::SetWorkspaceBinding { slot, selection } => {
                if let MacroWorkspaceBindingSlotV1::ViewDocument { view } = slot {
                    view_binding(view, &mut bindings);
                }
                workspace_selection_binding(selection, &mut bindings);
            }
            Self::MutateWorkspaceComposition { mutation } => {
                workspace_composition_bindings(mutation, &mut bindings);
            }
            Self::BeginAspectSetEdit { aspect_set }
            | Self::DuplicateAspectSet { aspect_set }
            | Self::SetWorkspaceAspectSet { aspect_set } => {
                resource_binding(aspect_set, &mut bindings);
            }
            Self::BeginResourceEdit { resource } => resource_binding(resource, &mut bindings),
            Self::MutateResource { mutation, .. } => {
                if let MacroResourceMutationV1::ChartDefinitionRecipe(
                    MacroDerivedRecipeMutationV1::Harmonic { radix, .. }
                    | MacroDerivedRecipeMutationV1::Relocation { radix, .. },
                ) = mutation.as_ref()
                {
                    resource_binding(radix, &mut bindings);
                }
            }
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
    next_view_binding: usize,
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
            next_view_binding: 1,
        })
    }

    pub fn capture(
        &mut self,
        intent: &AppIntent,
        origin_control: Option<ControlAddress>,
        before_model: &AppReadModel,
        settled_model: &AppReadModel,
    ) -> Result<(), MacroError> {
        if self.steps.len() == MACRO_STEP_LIMIT {
            return Err(MacroError::TooManySteps(self.steps.len() + 1));
        }
        let action = SemanticActionV1::capture(intent, before_model, &self.bindings)?;
        let bind = if matches!(
            action,
            SemanticActionV1::BeginNewChart | SemanticActionV1::OpenChart { .. }
        ) {
            Some(self.next_binding("chart")?)
        } else if matches!(
            action,
            SemanticActionV1::SaveDraft
                | SemanticActionV1::SaveWorkspace
                | SemanticActionV1::SaveResourceDraft { .. }
        ) && action.capture_result(settled_model).is_ok()
        {
            Some(self.next_binding("resource")?)
        } else if matches!(
            action,
            SemanticActionV1::MutateWorkspaceComposition {
                mutation: MacroWorkspaceCompositionMutationV1::AddView { .. }
            }
        ) {
            Some(self.next_binding("view")?)
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
        } else if kind == "resource" {
            let sequence = self.next_resource_binding;
            self.next_resource_binding = self.next_resource_binding.saturating_add(1);
            sequence
        } else {
            let sequence = self.next_view_binding;
            self.next_view_binding = self.next_view_binding.saturating_add(1);
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

fn chart_editor(model: &AppReadModel) -> Result<&crate::ChartEditorReadModel, MacroError> {
    model
        .chart_editor
        .as_ref()
        .ok_or_else(|| topology("the expected chart editor is not open"))
}

fn resource_draft(
    model: &AppReadModel,
    kind: crate::ResourceDraftKind,
) -> Result<&crate::TypedResourceDraftReadModel, MacroError> {
    model
        .resource_editor
        .drafts
        .iter()
        .find(|draft| draft.kind == kind)
        .ok_or_else(|| topology(format!("the expected {kind:?} draft is not open")))
}

fn life_event_rows(
    events: &[crate::LifeEventDraftReadModel],
) -> Vec<crate::StableDraftItemReadModel<mirabile_core::LifeEvent>> {
    events
        .iter()
        .map(|event| crate::StableDraftItemReadModel {
            item_id: event.item_id,
            value: event.value.clone(),
        })
        .collect()
}

fn capture_list_selector<T, F>(
    item_id: crate::DraftItemId,
    rows: &[crate::StableDraftItemReadModel<T>],
    collection: &str,
    key: F,
) -> Result<MacroListItemSelectorV1, MacroError>
where
    F: Fn(&T) -> Option<String>,
{
    let ordinal = rows
        .iter()
        .position(|row| row.item_id == item_id)
        .ok_or_else(|| topology(format!("{collection} item was not found")))?;
    if let Some(value) = key(&rows[ordinal].value)
        && rows
            .iter()
            .filter(|row| key(&row.value).as_deref() == Some(value.as_str()))
            .count()
            == 1
    {
        return Ok(MacroListItemSelectorV1::Key {
            collection: collection.into(),
            key: value,
        });
    }
    Ok(MacroListItemSelectorV1::Ordinal {
        collection: collection.into(),
        ordinal,
    })
}

fn resolve_list_selector<T, F>(
    selector: &MacroListItemSelectorV1,
    rows: &[crate::StableDraftItemReadModel<T>],
    collection: &str,
    key: F,
) -> Result<crate::DraftItemId, MacroError>
where
    F: Fn(&T) -> Option<String>,
{
    match selector {
        MacroListItemSelectorV1::Key {
            collection: actual,
            key: expected,
        } => {
            if actual != collection {
                return Err(topology(format!(
                    "expected collection {collection}, found {actual}"
                )));
            }
            let mut matches = rows
                .iter()
                .filter(|row| key(&row.value).as_deref() == Some(expected.as_str()));
            let row = matches
                .next()
                .ok_or_else(|| topology(format!("{collection} key {expected:?} was not found")))?;
            if matches.next().is_some() {
                return Err(topology(format!(
                    "{collection} key {expected:?} is ambiguous"
                )));
            }
            Ok(row.item_id)
        }
        MacroListItemSelectorV1::Ordinal {
            collection: actual,
            ordinal,
        } => {
            if actual != collection {
                return Err(topology(format!(
                    "expected collection {collection}, found {actual}"
                )));
            }
            rows.get(*ordinal).map(|row| row.item_id).ok_or_else(|| {
                topology(format!(
                    "{collection} ordinal {ordinal} is outside the current topology"
                ))
            })
        }
    }
}

fn capture_list_mutation<T: Clone, F>(
    mutation: &crate::DraftListMutation<T>,
    rows: &[crate::StableDraftItemReadModel<T>],
    collection: &str,
    key: F,
) -> Result<MacroListMutationV1<T>, MacroError>
where
    F: Fn(&T) -> Option<String> + Copy,
{
    Ok(match mutation {
        crate::DraftListMutation::Insert { after, value } => MacroListMutationV1::Insert {
            after: after
                .map(|item| capture_list_selector(item, rows, collection, key))
                .transpose()?,
            value: value.clone(),
        },
        crate::DraftListMutation::Update { item_id, value } => MacroListMutationV1::Update {
            item: capture_list_selector(*item_id, rows, collection, key)?,
            value: value.clone(),
        },
        crate::DraftListMutation::Remove { item_id } => MacroListMutationV1::Remove {
            item: capture_list_selector(*item_id, rows, collection, key)?,
        },
        crate::DraftListMutation::Move { item_id, before } => MacroListMutationV1::Move {
            item: capture_list_selector(*item_id, rows, collection, key)?,
            before: before
                .map(|item| capture_list_selector(item, rows, collection, key))
                .transpose()?,
        },
    })
}

fn resolve_list_mutation<T: Clone, F>(
    mutation: &MacroListMutationV1<T>,
    rows: &[crate::StableDraftItemReadModel<T>],
    collection: &str,
    key: F,
) -> Result<crate::DraftListMutation<T>, MacroError>
where
    F: Fn(&T) -> Option<String> + Copy,
{
    Ok(match mutation {
        MacroListMutationV1::Insert { after, value } => crate::DraftListMutation::Insert {
            after: after
                .as_ref()
                .map(|item| resolve_list_selector(item, rows, collection, key))
                .transpose()?,
            value: value.clone(),
        },
        MacroListMutationV1::Update { item, value } => crate::DraftListMutation::Update {
            item_id: resolve_list_selector(item, rows, collection, key)?,
            value: value.clone(),
        },
        MacroListMutationV1::Remove { item } => crate::DraftListMutation::Remove {
            item_id: resolve_list_selector(item, rows, collection, key)?,
        },
        MacroListMutationV1::Move { item, before } => crate::DraftListMutation::Move {
            item_id: resolve_list_selector(item, rows, collection, key)?,
            before: before
                .as_ref()
                .map(|item| resolve_list_selector(item, rows, collection, key))
                .transpose()?,
        },
    })
}

#[allow(clippy::unnecessary_wraps)]
fn point_selector_key(value: &mirabile_core::PointSelector) -> Option<String> {
    Some(match value {
        mirabile_core::PointSelector::Point(point) => format!("point:{}", point.as_str()),
        mirabile_core::PointSelector::Category(category) => format!("category:{category}"),
    })
}

#[allow(clippy::unnecessary_wraps)]
fn aspect_key(value: &mirabile_core::AspectDefinition) -> Option<String> {
    Some(value.id.as_str().into())
}

#[allow(clippy::unnecessary_wraps)]
fn slot_key(value: &mirabile_core::ChartSlot) -> Option<String> {
    Some(value.id.as_str().into())
}

#[allow(clippy::unnecessary_wraps)]
fn resource_key(value: &ResourceId) -> Option<String> {
    Some(value.to_string())
}

fn query_path(
    root: &crate::QueryNodeDraftReadModel,
    node_id: crate::DraftItemId,
) -> Option<Vec<usize>> {
    if root.node_id == node_id {
        return Some(Vec::new());
    }
    root.children.iter().enumerate().find_map(|(index, child)| {
        query_path(child, node_id).map(|mut path| {
            path.insert(0, index);
            path
        })
    })
}

fn query_node_at_path<'a>(
    root: &'a crate::QueryNodeDraftReadModel,
    selector: &MacroQueryNodeSelectorV1,
) -> Result<&'a crate::QueryNodeDraftReadModel, MacroError> {
    let mut node = root;
    for index in &selector.path {
        node = node.children.get(*index).ok_or_else(|| {
            topology(format!(
                "query path {:?} does not match the current topology",
                selector.path
            ))
        })?;
    }
    Ok(node)
}

fn captured_query_node(
    root: &crate::QueryNodeDraftReadModel,
    node_id: crate::DraftItemId,
) -> Result<MacroQueryNodeSelectorV1, MacroError> {
    query_path(root, node_id)
        .map(|path| MacroQueryNodeSelectorV1 { path })
        .ok_or_else(|| topology("query node was not found"))
}

fn capture_query_mutation(
    mutation: &crate::QueryTreeMutation,
    root: &crate::QueryNodeDraftReadModel,
) -> Result<MacroQueryTreeMutationV1, MacroError> {
    Ok(match mutation {
        crate::QueryTreeMutation::Replace {
            node_id,
            expression,
        } => MacroQueryTreeMutationV1::Replace {
            node: captured_query_node(root, *node_id)?,
            expression: expression.clone(),
        },
        crate::QueryTreeMutation::InsertChild {
            parent_id,
            after,
            expression,
        } => MacroQueryTreeMutationV1::InsertChild {
            parent: captured_query_node(root, *parent_id)?,
            after: after
                .map(|node| captured_query_node(root, node))
                .transpose()?,
            expression: expression.clone(),
        },
        crate::QueryTreeMutation::Remove { node_id } => MacroQueryTreeMutationV1::Remove {
            node: captured_query_node(root, *node_id)?,
        },
        crate::QueryTreeMutation::Move {
            node_id,
            new_parent_id,
            before,
        } => MacroQueryTreeMutationV1::Move {
            node: captured_query_node(root, *node_id)?,
            new_parent: captured_query_node(root, *new_parent_id)?,
            before: before
                .map(|node| captured_query_node(root, node))
                .transpose()?,
        },
    })
}

fn resolve_query_mutation(
    mutation: &MacroQueryTreeMutationV1,
    root: &crate::QueryNodeDraftReadModel,
) -> Result<crate::QueryTreeMutation, MacroError> {
    Ok(match mutation {
        MacroQueryTreeMutationV1::Replace { node, expression } => {
            crate::QueryTreeMutation::Replace {
                node_id: query_node_at_path(root, node)?.node_id,
                expression: expression.clone(),
            }
        }
        MacroQueryTreeMutationV1::InsertChild {
            parent,
            after,
            expression,
        } => crate::QueryTreeMutation::InsertChild {
            parent_id: query_node_at_path(root, parent)?.node_id,
            after: after
                .as_ref()
                .map(|node| query_node_at_path(root, node).map(|node| node.node_id))
                .transpose()?,
            expression: expression.clone(),
        },
        MacroQueryTreeMutationV1::Remove { node } => crate::QueryTreeMutation::Remove {
            node_id: query_node_at_path(root, node)?.node_id,
        },
        MacroQueryTreeMutationV1::Move {
            node,
            new_parent,
            before,
        } => crate::QueryTreeMutation::Move {
            node_id: query_node_at_path(root, node)?.node_id,
            new_parent_id: query_node_at_path(root, new_parent)?.node_id,
            before: before
                .as_ref()
                .map(|node| query_node_at_path(root, node).map(|node| node.node_id))
                .transpose()?,
        },
    })
}

#[allow(clippy::too_many_lines)]
fn capture_resource_mutation(
    mutation: &crate::ResourceMutation,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<MacroResourceMutationV1, MacroError> {
    use crate::NestedResourceDraftReadModel as Nested;
    let draft = resource_draft(model, mutation.kind())?;
    Ok(match mutation {
        crate::ResourceMutation::ChartRecord(value) => match value {
            crate::ChartRecordMutation::SetEventKind(value) => {
                MacroResourceMutationV1::ChartRecordEventKind(value.clone())
            }
            crate::ChartRecordMutation::SetSubject(value) => {
                MacroResourceMutationV1::ChartRecordSubject(value.clone())
            }
            crate::ChartRecordMutation::SetTime(value) => {
                MacroResourceMutationV1::ChartRecordTime(value.clone())
            }
            crate::ChartRecordMutation::SetLocation(value) => {
                MacroResourceMutationV1::ChartRecordLocation(value.clone())
            }
            crate::ChartRecordMutation::SetSource(value) => {
                MacroResourceMutationV1::ChartRecordSource(value.clone())
            }
            crate::ChartRecordMutation::Notes(value) => {
                let Nested::ChartRecord { notes, .. } = &draft.nested else {
                    return Err(topology("ChartRecord note topology is unavailable"));
                };
                MacroResourceMutationV1::ChartRecordNotes(capture_list_mutation(
                    value,
                    notes,
                    "resource.chart_record.notes",
                    |_| None,
                )?)
            }
            crate::ChartRecordMutation::LifeEvents(value) => {
                let Nested::ChartRecord { life_events, .. } = &draft.nested else {
                    return Err(topology("ChartRecord life-event topology is unavailable"));
                };
                let rows = life_event_rows(life_events);
                MacroResourceMutationV1::ChartRecordLifeEvents(capture_list_mutation(
                    value,
                    &rows,
                    "resource.chart_record.life_events",
                    |_| None,
                )?)
            }
            crate::ChartRecordMutation::LifeEventNotes {
                life_event_id,
                mutation,
            } => {
                let Nested::ChartRecord { life_events, .. } = &draft.nested else {
                    return Err(topology("ChartRecord life-event topology is unavailable"));
                };
                let event = life_events
                    .iter()
                    .find(|event| event.item_id == *life_event_id)
                    .ok_or_else(|| topology("ChartRecord life event was not found"))?;
                MacroResourceMutationV1::ChartRecordLifeEventNotes {
                    life_event: capture_list_selector(
                        *life_event_id,
                        &life_event_rows(life_events),
                        "resource.chart_record.life_events",
                        |_| None,
                    )?,
                    mutation: capture_list_mutation(
                        mutation,
                        &event.notes,
                        "resource.chart_record.life_event.notes",
                        |_| None,
                    )?,
                }
            }
            crate::ChartRecordMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::ChartDefinition(value) => match value {
            crate::ChartDefinitionMutation::SetSource(value) => {
                MacroResourceMutationV1::ChartDefinitionSource(value.clone())
            }
            crate::ChartDefinitionMutation::SetCalculation(value) => {
                MacroResourceMutationV1::ChartDefinitionCalculation(value.clone())
            }
            crate::ChartDefinitionMutation::MutateDerivedRecipe(value) => {
                let recipe = match value {
                    crate::DerivedRecipeMutation::SetTransit { at, location } => {
                        MacroDerivedRecipeMutationV1::Transit {
                            at: at.clone(),
                            location: location.clone(),
                        }
                    }
                    crate::DerivedRecipeMutation::SetHarmonic { radix, harmonic } => {
                        MacroDerivedRecipeMutationV1::Harmonic {
                            radix: captured_any_resource(*radix, model, bindings),
                            harmonic: *harmonic,
                        }
                    }
                    crate::DerivedRecipeMutation::SetRelocation { radix, location } => {
                        MacroDerivedRecipeMutationV1::Relocation {
                            radix: captured_any_resource(*radix, model, bindings),
                            location: location.clone(),
                        }
                    }
                    crate::DerivedRecipeMutation::SetCompositeMethod(method) => {
                        MacroDerivedRecipeMutationV1::CompositeMethod { method: *method }
                    }
                    crate::DerivedRecipeMutation::CompositeCharts(value) => {
                        let Nested::ChartDefinition { composite_charts } = &draft.nested else {
                            return Err(topology("Composite chart topology is unavailable"));
                        };
                        MacroDerivedRecipeMutationV1::CompositeCharts(capture_list_mutation(
                            value,
                            composite_charts,
                            "resource.chart_definition.composite_charts",
                            resource_key,
                        )?)
                    }
                };
                MacroResourceMutationV1::ChartDefinitionRecipe(recipe)
            }
            crate::ChartDefinitionMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::PointSet(value) => match value {
            crate::PointSetMutation::Selectors(value) => {
                let Nested::PointSet(rows) = &draft.nested else {
                    return Err(topology("PointSet topology is unavailable"));
                };
                MacroResourceMutationV1::PointSetSelectors(capture_list_mutation(
                    value,
                    rows,
                    "resource.point_set.selectors",
                    point_selector_key,
                )?)
            }
            crate::PointSetMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::AspectSet(value) => match value {
            crate::AspectSetMutation::Aspects(value) => {
                let Nested::AspectSet(rows) = &draft.nested else {
                    return Err(topology("AspectSet topology is unavailable"));
                };
                MacroResourceMutationV1::AspectSetAspects(capture_list_mutation(
                    value,
                    rows,
                    "resource.aspect_set.aspects",
                    aspect_key,
                )?)
            }
            crate::AspectSetMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::AnalysisProfile(value) => match value {
            crate::AnalysisProfileMutation::SetProfile(value) => {
                MacroResourceMutationV1::AnalysisProfile(value.clone())
            }
            crate::AnalysisProfileMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::WheelTemplate(value) => match value {
            crate::WheelTemplateMutation::Rings(value) => {
                let Nested::WheelTemplate(rows) = &draft.nested else {
                    return Err(topology("WheelTemplate ring topology is unavailable"));
                };
                MacroResourceMutationV1::WheelTemplateRings(capture_list_mutation(
                    value,
                    rows,
                    "resource.wheel_template.rings",
                    |_| None,
                )?)
            }
            crate::WheelTemplateMutation::SetTemplateFields(value) => {
                MacroResourceMutationV1::WheelTemplateFields(value.clone())
            }
            crate::WheelTemplateMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::ViewDocument(value) => match value {
            crate::ViewDocumentMutation::ChartSlots(value) => {
                let Nested::ViewDocument { chart_slots, .. } = &draft.nested else {
                    return Err(topology("ViewDocument slot topology is unavailable"));
                };
                MacroResourceMutationV1::ViewDocumentChartSlots(capture_list_mutation(
                    value,
                    chart_slots,
                    "resource.view_document.chart_slots",
                    slot_key,
                )?)
            }
            crate::ViewDocumentMutation::RenameChartSlot { item_id, slot } => {
                let Nested::ViewDocument { chart_slots, .. } = &draft.nested else {
                    return Err(topology("ViewDocument slot topology is unavailable"));
                };
                MacroResourceMutationV1::ViewDocumentRenameChartSlot {
                    item: capture_list_selector(
                        *item_id,
                        chart_slots,
                        "resource.view_document.chart_slots",
                        slot_key,
                    )?,
                    slot: slot.clone(),
                }
            }
            crate::ViewDocumentMutation::Objects(value) => {
                let Nested::ViewDocument { objects, .. } = &draft.nested else {
                    return Err(topology("ViewDocument object topology is unavailable"));
                };
                MacroResourceMutationV1::ViewDocumentObjects(capture_list_mutation(
                    value,
                    objects,
                    "resource.view_document.objects",
                    |_| None,
                )?)
            }
            crate::ViewDocumentMutation::SetLayout(value) => {
                MacroResourceMutationV1::ViewDocumentLayout(value.clone())
            }
            crate::ViewDocumentMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::Theme(value) => match value {
            crate::ThemeMutation::SetTheme(value) => MacroResourceMutationV1::Theme(value.clone()),
            crate::ThemeMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::QueryDefinition(value) => match value {
            crate::QueryDefinitionMutation::SetDescription(value) => {
                MacroResourceMutationV1::QueryDescription(value.clone())
            }
            crate::QueryDefinitionMutation::Tree(value) => {
                let Nested::QueryDefinition(root) = &draft.nested else {
                    return Err(topology("QueryDefinition topology is unavailable"));
                };
                MacroResourceMutationV1::QueryTree(capture_query_mutation(value, root)?)
            }
            crate::QueryDefinitionMutation::Metadata(_) => unreachable!(),
        },
        crate::ResourceMutation::WorkspaceDocument(value) => match value {
            crate::WorkspaceDocumentMutation::ChartInstances(value) => {
                let Nested::WorkspaceDocument { charts, .. } = &draft.nested else {
                    return Err(topology("WorkspaceDocument chart topology is unavailable"));
                };
                MacroResourceMutationV1::WorkspaceCharts(capture_list_mutation(
                    value,
                    charts,
                    "resource.workspace_document.charts",
                    |_| None,
                )?)
            }
            crate::WorkspaceDocumentMutation::Views(value) => {
                let Nested::WorkspaceDocument { views, .. } = &draft.nested else {
                    return Err(topology("WorkspaceDocument view topology is unavailable"));
                };
                MacroResourceMutationV1::WorkspaceViews(capture_list_mutation(
                    value,
                    views,
                    "resource.workspace_document.views",
                    |_| None,
                )?)
            }
            crate::WorkspaceDocumentMutation::SetProfile(value) => {
                MacroResourceMutationV1::WorkspaceProfile(value.clone())
            }
            crate::WorkspaceDocumentMutation::Metadata(_) => unreachable!(),
        },
    })
}

#[allow(clippy::too_many_lines)]
fn resolve_resource_mutation(
    kind: crate::ResourceDraftKind,
    mutation: &MacroResourceMutationV1,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<crate::ResourceMutation, MacroError> {
    use crate::NestedResourceDraftReadModel as Nested;
    let draft = resource_draft(model, kind)?;
    let resolved = match mutation {
        MacroResourceMutationV1::ChartRecordEventKind(value) => {
            crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::SetEventKind(
                value.clone(),
            ))
        }
        MacroResourceMutationV1::ChartRecordSubject(value) => crate::ResourceMutation::ChartRecord(
            crate::ChartRecordMutation::SetSubject(value.clone()),
        ),
        MacroResourceMutationV1::ChartRecordTime(value) => {
            crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::SetTime(value.clone()))
        }
        MacroResourceMutationV1::ChartRecordLocation(value) => {
            crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::SetLocation(
                value.clone(),
            ))
        }
        MacroResourceMutationV1::ChartRecordSource(value) => crate::ResourceMutation::ChartRecord(
            crate::ChartRecordMutation::SetSource(value.clone()),
        ),
        MacroResourceMutationV1::ChartRecordNotes(value) => {
            let Nested::ChartRecord { notes, .. } = &draft.nested else {
                return Err(topology("ChartRecord note topology is unavailable"));
            };
            crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::Notes(
                resolve_list_mutation(value, notes, "resource.chart_record.notes", |_| None)?,
            ))
        }
        MacroResourceMutationV1::ChartRecordLifeEvents(value) => {
            let Nested::ChartRecord { life_events, .. } = &draft.nested else {
                return Err(topology("ChartRecord life-event topology is unavailable"));
            };
            let rows = life_event_rows(life_events);
            crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::LifeEvents(
                resolve_list_mutation(value, &rows, "resource.chart_record.life_events", |_| None)?,
            ))
        }
        MacroResourceMutationV1::ChartRecordLifeEventNotes {
            life_event,
            mutation,
        } => {
            let Nested::ChartRecord { life_events, .. } = &draft.nested else {
                return Err(topology("ChartRecord life-event topology is unavailable"));
            };
            let life_event_id = resolve_list_selector(
                life_event,
                &life_event_rows(life_events),
                "resource.chart_record.life_events",
                |_| None,
            )?;
            let event = life_events
                .iter()
                .find(|event| event.item_id == life_event_id)
                .ok_or_else(|| topology("ChartRecord life-event topology changed"))?;
            crate::ResourceMutation::ChartRecord(crate::ChartRecordMutation::LifeEventNotes {
                life_event_id,
                mutation: resolve_list_mutation(
                    mutation,
                    &event.notes,
                    "resource.chart_record.life_event.notes",
                    |_| None,
                )?,
            })
        }
        MacroResourceMutationV1::ChartDefinitionSource(value) => {
            crate::ResourceMutation::ChartDefinition(crate::ChartDefinitionMutation::SetSource(
                value.clone(),
            ))
        }
        MacroResourceMutationV1::ChartDefinitionCalculation(value) => {
            crate::ResourceMutation::ChartDefinition(
                crate::ChartDefinitionMutation::SetCalculation(value.clone()),
            )
        }
        MacroResourceMutationV1::ChartDefinitionRecipe(value) => {
            let value = match value {
                MacroDerivedRecipeMutationV1::Transit { at, location } => {
                    crate::DerivedRecipeMutation::SetTransit {
                        at: at.clone(),
                        location: location.clone(),
                    }
                }
                MacroDerivedRecipeMutationV1::Harmonic { radix, harmonic } => {
                    crate::DerivedRecipeMutation::SetHarmonic {
                        radix: resolve_any_resource(radix, model, bindings)?,
                        harmonic: *harmonic,
                    }
                }
                MacroDerivedRecipeMutationV1::Relocation { radix, location } => {
                    crate::DerivedRecipeMutation::SetRelocation {
                        radix: resolve_any_resource(radix, model, bindings)?,
                        location: location.clone(),
                    }
                }
                MacroDerivedRecipeMutationV1::CompositeMethod { method } => {
                    crate::DerivedRecipeMutation::SetCompositeMethod(*method)
                }
                MacroDerivedRecipeMutationV1::CompositeCharts(value) => {
                    let Nested::ChartDefinition { composite_charts } = &draft.nested else {
                        return Err(topology("Composite chart topology is unavailable"));
                    };
                    crate::DerivedRecipeMutation::CompositeCharts(resolve_list_mutation(
                        value,
                        composite_charts,
                        "resource.chart_definition.composite_charts",
                        resource_key,
                    )?)
                }
            };
            crate::ResourceMutation::ChartDefinition(
                crate::ChartDefinitionMutation::MutateDerivedRecipe(value),
            )
        }
        MacroResourceMutationV1::PointSetSelectors(value) => {
            let Nested::PointSet(rows) = &draft.nested else {
                return Err(topology("PointSet topology is unavailable"));
            };
            crate::ResourceMutation::PointSet(crate::PointSetMutation::Selectors(
                resolve_list_mutation(
                    value,
                    rows,
                    "resource.point_set.selectors",
                    point_selector_key,
                )?,
            ))
        }
        MacroResourceMutationV1::AspectSetAspects(value) => {
            let Nested::AspectSet(rows) = &draft.nested else {
                return Err(topology("AspectSet topology is unavailable"));
            };
            crate::ResourceMutation::AspectSet(crate::AspectSetMutation::Aspects(
                resolve_list_mutation(value, rows, "resource.aspect_set.aspects", aspect_key)?,
            ))
        }
        MacroResourceMutationV1::AnalysisProfile(value) => {
            crate::ResourceMutation::AnalysisProfile(crate::AnalysisProfileMutation::SetProfile(
                value.clone(),
            ))
        }
        MacroResourceMutationV1::WheelTemplateRings(value) => {
            let Nested::WheelTemplate(rows) = &draft.nested else {
                return Err(topology("WheelTemplate ring topology is unavailable"));
            };
            crate::ResourceMutation::WheelTemplate(crate::WheelTemplateMutation::Rings(
                resolve_list_mutation(value, rows, "resource.wheel_template.rings", |_| None)?,
            ))
        }
        MacroResourceMutationV1::WheelTemplateFields(value) => {
            crate::ResourceMutation::WheelTemplate(crate::WheelTemplateMutation::SetTemplateFields(
                value.clone(),
            ))
        }
        MacroResourceMutationV1::ViewDocumentChartSlots(value) => {
            let Nested::ViewDocument { chart_slots, .. } = &draft.nested else {
                return Err(topology("ViewDocument slot topology is unavailable"));
            };
            crate::ResourceMutation::ViewDocument(crate::ViewDocumentMutation::ChartSlots(
                resolve_list_mutation(
                    value,
                    chart_slots,
                    "resource.view_document.chart_slots",
                    slot_key,
                )?,
            ))
        }
        MacroResourceMutationV1::ViewDocumentRenameChartSlot { item, slot } => {
            let Nested::ViewDocument { chart_slots, .. } = &draft.nested else {
                return Err(topology("ViewDocument slot topology is unavailable"));
            };
            crate::ResourceMutation::ViewDocument(crate::ViewDocumentMutation::RenameChartSlot {
                item_id: resolve_list_selector(
                    item,
                    chart_slots,
                    "resource.view_document.chart_slots",
                    slot_key,
                )?,
                slot: slot.clone(),
            })
        }
        MacroResourceMutationV1::ViewDocumentObjects(value) => {
            let Nested::ViewDocument { objects, .. } = &draft.nested else {
                return Err(topology("ViewDocument object topology is unavailable"));
            };
            crate::ResourceMutation::ViewDocument(crate::ViewDocumentMutation::Objects(
                resolve_list_mutation(value, objects, "resource.view_document.objects", |_| None)?,
            ))
        }
        MacroResourceMutationV1::ViewDocumentLayout(value) => {
            crate::ResourceMutation::ViewDocument(crate::ViewDocumentMutation::SetLayout(
                value.clone(),
            ))
        }
        MacroResourceMutationV1::Theme(value) => {
            crate::ResourceMutation::Theme(crate::ThemeMutation::SetTheme(value.clone()))
        }
        MacroResourceMutationV1::QueryDescription(value) => {
            crate::ResourceMutation::QueryDefinition(
                crate::QueryDefinitionMutation::SetDescription(value.clone()),
            )
        }
        MacroResourceMutationV1::QueryTree(value) => {
            let Nested::QueryDefinition(root) = &draft.nested else {
                return Err(topology("QueryDefinition topology is unavailable"));
            };
            crate::ResourceMutation::QueryDefinition(crate::QueryDefinitionMutation::Tree(
                resolve_query_mutation(value, root)?,
            ))
        }
        MacroResourceMutationV1::WorkspaceCharts(value) => {
            let Nested::WorkspaceDocument { charts, .. } = &draft.nested else {
                return Err(topology("WorkspaceDocument chart topology is unavailable"));
            };
            crate::ResourceMutation::WorkspaceDocument(
                crate::WorkspaceDocumentMutation::ChartInstances(resolve_list_mutation(
                    value,
                    charts,
                    "resource.workspace_document.charts",
                    |_| None,
                )?),
            )
        }
        MacroResourceMutationV1::WorkspaceViews(value) => {
            let Nested::WorkspaceDocument { views, .. } = &draft.nested else {
                return Err(topology("WorkspaceDocument view topology is unavailable"));
            };
            crate::ResourceMutation::WorkspaceDocument(crate::WorkspaceDocumentMutation::Views(
                resolve_list_mutation(value, views, "resource.workspace_document.views", |_| None)?,
            ))
        }
        MacroResourceMutationV1::WorkspaceProfile(value) => {
            crate::ResourceMutation::WorkspaceDocument(
                crate::WorkspaceDocumentMutation::SetProfile(value.clone()),
            )
        }
    };
    if resolved.kind() != kind {
        return Err(topology(format!(
            "macro resource mutation expects {kind:?}, found {:?}",
            resolved.kind()
        )));
    }
    Ok(resolved)
}

fn capture_workspace_binding_slot(
    slot: crate::WorkspaceBindingSlot,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroWorkspaceBindingSlotV1 {
    match slot {
        crate::WorkspaceBindingSlot::DisplayedPoints => {
            MacroWorkspaceBindingSlotV1::DisplayedPoints
        }
        crate::WorkspaceBindingSlot::AspectedPoints => MacroWorkspaceBindingSlotV1::AspectedPoints,
        crate::WorkspaceBindingSlot::TransitPoints => MacroWorkspaceBindingSlotV1::TransitPoints,
        crate::WorkspaceBindingSlot::Aspects => MacroWorkspaceBindingSlotV1::Aspects,
        crate::WorkspaceBindingSlot::Analysis => MacroWorkspaceBindingSlotV1::Analysis,
        crate::WorkspaceBindingSlot::Theme => MacroWorkspaceBindingSlotV1::Theme,
        crate::WorkspaceBindingSlot::Wheel => MacroWorkspaceBindingSlotV1::Wheel,
        crate::WorkspaceBindingSlot::ViewDocument { view_id } => {
            MacroWorkspaceBindingSlotV1::ViewDocument {
                view: captured_view(view_id, model, bindings),
            }
        }
    }
}

fn resolve_workspace_binding_slot(
    slot: &MacroWorkspaceBindingSlotV1,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<crate::WorkspaceBindingSlot, MacroError> {
    Ok(match slot {
        MacroWorkspaceBindingSlotV1::DisplayedPoints => {
            crate::WorkspaceBindingSlot::DisplayedPoints
        }
        MacroWorkspaceBindingSlotV1::AspectedPoints => crate::WorkspaceBindingSlot::AspectedPoints,
        MacroWorkspaceBindingSlotV1::TransitPoints => crate::WorkspaceBindingSlot::TransitPoints,
        MacroWorkspaceBindingSlotV1::Aspects => crate::WorkspaceBindingSlot::Aspects,
        MacroWorkspaceBindingSlotV1::Analysis => crate::WorkspaceBindingSlot::Analysis,
        MacroWorkspaceBindingSlotV1::Theme => crate::WorkspaceBindingSlot::Theme,
        MacroWorkspaceBindingSlotV1::Wheel => crate::WorkspaceBindingSlot::Wheel,
        MacroWorkspaceBindingSlotV1::ViewDocument { view } => {
            crate::WorkspaceBindingSlot::ViewDocument {
                view_id: resolve_view(view, model, bindings)?,
            }
        }
    })
}

fn capture_workspace_binding_selection(
    selection: crate::WorkspaceBindingSelection,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroWorkspaceBindingSelectionV1 {
    match selection {
        crate::WorkspaceBindingSelection::Follow { resource_id } => {
            MacroWorkspaceBindingSelectionV1::Follow {
                resource: captured_any_resource(resource_id, model, bindings),
            }
        }
        crate::WorkspaceBindingSelection::Pinned {
            resource_id,
            revision,
        } => MacroWorkspaceBindingSelectionV1::Pinned {
            resource: captured_any_resource(resource_id, model, bindings),
            revision,
        },
        crate::WorkspaceBindingSelection::Inline { resource_id } => {
            MacroWorkspaceBindingSelectionV1::Inline {
                resource: captured_any_resource(resource_id, model, bindings),
            }
        }
    }
}

fn resolve_workspace_binding_selection(
    selection: &MacroWorkspaceBindingSelectionV1,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<crate::WorkspaceBindingSelection, MacroError> {
    Ok(match selection {
        MacroWorkspaceBindingSelectionV1::Follow { resource } => {
            crate::WorkspaceBindingSelection::Follow {
                resource_id: resolve_any_resource(resource, model, bindings)?,
            }
        }
        MacroWorkspaceBindingSelectionV1::Pinned { resource, revision } => {
            crate::WorkspaceBindingSelection::Pinned {
                resource_id: resolve_any_resource(resource, model, bindings)?,
                revision: *revision,
            }
        }
        MacroWorkspaceBindingSelectionV1::Inline { resource } => {
            crate::WorkspaceBindingSelection::Inline {
                resource_id: resolve_any_resource(resource, model, bindings)?,
            }
        }
    })
}

fn capture_workspace_composition(
    mutation: &crate::WorkspaceCompositionMutation,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> MacroWorkspaceCompositionMutationV1 {
    match mutation {
        crate::WorkspaceCompositionMutation::MoveChart {
            instance_id,
            before,
        } => MacroWorkspaceCompositionMutationV1::MoveChart {
            chart: captured_chart(*instance_id, model, bindings),
            before: before.map(|chart| captured_chart(chart, model, bindings)),
        },
        crate::WorkspaceCompositionMutation::AddView { document } => {
            MacroWorkspaceCompositionMutationV1::AddView {
                document: capture_workspace_binding_selection(*document, model, bindings),
            }
        }
        crate::WorkspaceCompositionMutation::RemoveView { view_id } => {
            MacroWorkspaceCompositionMutationV1::RemoveView {
                view: captured_view(*view_id, model, bindings),
            }
        }
        crate::WorkspaceCompositionMutation::MoveView { view_id, before } => {
            MacroWorkspaceCompositionMutationV1::MoveView {
                view: captured_view(*view_id, model, bindings),
                before: before.map(|view| captured_view(view, model, bindings)),
            }
        }
        crate::WorkspaceCompositionMutation::SetRotation { view_id, rotation } => {
            MacroWorkspaceCompositionMutationV1::SetRotation {
                view: captured_view(*view_id, model, bindings),
                rotation: *rotation,
            }
        }
        crate::WorkspaceCompositionMutation::SetPointHidden {
            view_id,
            point_id,
            hidden,
        } => MacroWorkspaceCompositionMutationV1::SetPointHidden {
            view: captured_view(*view_id, model, bindings),
            point_id: point_id.clone(),
            hidden: *hidden,
        },
    }
}

fn resolve_workspace_composition(
    mutation: &MacroWorkspaceCompositionMutationV1,
    model: &AppReadModel,
    bindings: &MacroBindings,
) -> Result<crate::WorkspaceCompositionMutation, MacroError> {
    Ok(match mutation {
        MacroWorkspaceCompositionMutationV1::MoveChart { chart, before } => {
            crate::WorkspaceCompositionMutation::MoveChart {
                instance_id: resolve_chart(chart, model, bindings)?,
                before: before
                    .as_ref()
                    .map(|chart| resolve_chart(chart, model, bindings))
                    .transpose()?,
            }
        }
        MacroWorkspaceCompositionMutationV1::AddView { document } => {
            crate::WorkspaceCompositionMutation::AddView {
                document: resolve_workspace_binding_selection(document, model, bindings)?,
            }
        }
        MacroWorkspaceCompositionMutationV1::RemoveView { view } => {
            crate::WorkspaceCompositionMutation::RemoveView {
                view_id: resolve_view(view, model, bindings)?,
            }
        }
        MacroWorkspaceCompositionMutationV1::MoveView { view, before } => {
            crate::WorkspaceCompositionMutation::MoveView {
                view_id: resolve_view(view, model, bindings)?,
                before: before
                    .as_ref()
                    .map(|view| resolve_view(view, model, bindings))
                    .transpose()?,
            }
        }
        MacroWorkspaceCompositionMutationV1::SetRotation { view, rotation } => {
            crate::WorkspaceCompositionMutation::SetRotation {
                view_id: resolve_view(view, model, bindings)?,
                rotation: *rotation,
            }
        }
        MacroWorkspaceCompositionMutationV1::SetPointHidden {
            view,
            point_id,
            hidden,
        } => crate::WorkspaceCompositionMutation::SetPointHidden {
            view_id: resolve_view(view, model, bindings)?,
            point_id: point_id.clone(),
            hidden: *hidden,
        },
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

fn workspace_selection_binding<'a>(
    selection: &'a MacroWorkspaceBindingSelectionV1,
    output: &mut Vec<&'a MacroBindingName>,
) {
    match selection {
        MacroWorkspaceBindingSelectionV1::Follow { resource }
        | MacroWorkspaceBindingSelectionV1::Pinned { resource, .. }
        | MacroWorkspaceBindingSelectionV1::Inline { resource } => {
            resource_binding(resource, output);
        }
    }
}

fn workspace_composition_bindings<'a>(
    mutation: &'a MacroWorkspaceCompositionMutationV1,
    output: &mut Vec<&'a MacroBindingName>,
) {
    match mutation {
        MacroWorkspaceCompositionMutationV1::MoveChart { chart, before } => {
            instance_binding(chart, output);
            if let Some(before) = before {
                instance_binding(before, output);
            }
        }
        MacroWorkspaceCompositionMutationV1::AddView { document } => {
            workspace_selection_binding(document, output);
        }
        MacroWorkspaceCompositionMutationV1::RemoveView { view }
        | MacroWorkspaceCompositionMutationV1::SetRotation { view, .. }
        | MacroWorkspaceCompositionMutationV1::SetPointHidden { view, .. } => {
            view_binding(view, output);
        }
        MacroWorkspaceCompositionMutationV1::MoveView { view, before } => {
            view_binding(view, output);
            if let Some(before) = before {
                view_binding(before, output);
            }
        }
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

fn topology(message: impl Into<String>) -> MacroError {
    MacroError::TopologyMismatch(message.into())
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
    #[error("macro topology mismatch: {0}")]
    TopologyMismatch(String),
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
                &model,
            )
            .expect("begin chart");
        recorder
            .capture(
                &AppIntent::ActivateChart { instance_id: chart },
                None,
                &model,
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

    #[test]
    fn structural_list_selectors_rebind_new_draft_ids_and_report_topology_mismatch() {
        fn model_with_points(points: Vec<(crate::DraftItemId, &str)>) -> AppReadModel {
            let mut model = AppReadModel::initializing();
            let rows = points
                .into_iter()
                .map(|(item_id, point)| crate::StableDraftItemReadModel {
                    item_id,
                    value: mirabile_core::PointSelector::Point(
                        PointId::new(point).expect("point ID"),
                    ),
                })
                .collect::<Vec<_>>();
            model
                .resource_editor
                .drafts
                .push(crate::TypedResourceDraftReadModel {
                    kind: crate::ResourceDraftKind::PointSet,
                    resource_id: None,
                    title: "Structural points".into(),
                    description: None,
                    tags: Vec::new(),
                    state: crate::DraftState::New,
                    conflicts: Vec::new(),
                    nested: crate::NestedResourceDraftReadModel::PointSet(rows.clone()),
                    value: crate::ResourceDraftValueReadModel::PointSet(mirabile_core::PointSet {
                        points: rows.into_iter().map(|row| row.value).collect(),
                    }),
                });
            model
        }

        let old_sun = crate::DraftItemId::new();
        let old_moon = crate::DraftItemId::new();
        let before = model_with_points(vec![(old_sun, "sun"), (old_moon, "moon")]);
        let intent = AppIntent::ApplyResourceMutation(Box::new(crate::ResourceMutation::PointSet(
            crate::PointSetMutation::Selectors(crate::DraftListMutation::Update {
                item_id: old_moon,
                value: mirabile_core::PointSelector::Point(PointId::new("mars").expect("point ID")),
            }),
        )));
        let action = SemanticActionV1::capture(&intent, &before, &MacroBindings::default())
            .expect("capture structural update");
        let json = serde_json::to_string(&action).expect("serialize action");
        assert!(!json.contains(&old_sun.to_string()));
        assert!(!json.contains(&old_moon.to_string()));

        let new_sun = crate::DraftItemId::new();
        let new_moon = crate::DraftItemId::new();
        let replay = model_with_points(vec![(new_sun, "sun"), (new_moon, "moon")]);
        let resolved = action
            .resolve(&replay, &MacroBindings::default())
            .expect("resolve against fresh draft IDs");
        assert!(matches!(
            resolved,
            AppIntent::ApplyResourceMutation(mutation)
                if matches!(
                    mutation.as_ref(),
                    crate::ResourceMutation::PointSet(crate::PointSetMutation::Selectors(
                        crate::DraftListMutation::Update { item_id, .. }
                    )) if *item_id == new_moon
                )
        ));

        let changed = model_with_points(vec![(new_sun, "sun")]);
        assert!(matches!(
            action.resolve(&changed, &MacroBindings::default()),
            Err(MacroError::TopologyMismatch(_))
        ));
    }

    #[test]
    fn query_paths_rebind_new_node_ids_and_fail_explicitly_on_shape_change() {
        fn predicate(point: &str) -> crate::QueryExpr {
            crate::QueryExpr::Predicate(mirabile_core::Predicate::InSign {
                point: PointId::new(point).expect("point ID"),
                sign_index: 0,
            })
        }
        fn model_with_query(child_count: usize) -> AppReadModel {
            let children = ["sun", "moon"]
                .into_iter()
                .take(child_count)
                .map(|point| crate::QueryNodeDraftReadModel {
                    node_id: crate::DraftItemId::new(),
                    expression: predicate(point),
                    children: Vec::new(),
                })
                .collect::<Vec<_>>();
            let expression = crate::QueryExpr::And(
                children
                    .iter()
                    .map(|child| child.expression.clone())
                    .collect(),
            );
            let mut model = AppReadModel::initializing();
            model
                .resource_editor
                .drafts
                .push(crate::TypedResourceDraftReadModel {
                    kind: crate::ResourceDraftKind::QueryDefinition,
                    resource_id: None,
                    title: "Structural query".into(),
                    description: None,
                    tags: Vec::new(),
                    state: crate::DraftState::New,
                    conflicts: Vec::new(),
                    nested: crate::NestedResourceDraftReadModel::QueryDefinition(
                        crate::QueryNodeDraftReadModel {
                            node_id: crate::DraftItemId::new(),
                            expression: expression.clone(),
                            children,
                        },
                    ),
                    value: crate::ResourceDraftValueReadModel::QueryDefinition(
                        mirabile_core::QueryDefinition {
                            expression,
                            description: None,
                        },
                    ),
                });
            model
        }

        let before = model_with_query(2);
        let crate::NestedResourceDraftReadModel::QueryDefinition(root) =
            &before.resource_editor.drafts[0].nested
        else {
            panic!("query topology");
        };
        let old_node = root.children[1].node_id;
        let intent =
            AppIntent::ApplyResourceMutation(Box::new(crate::ResourceMutation::QueryDefinition(
                crate::QueryDefinitionMutation::Tree(crate::QueryTreeMutation::Replace {
                    node_id: old_node,
                    expression: predicate("venus"),
                }),
            )));
        let action = SemanticActionV1::capture(&intent, &before, &MacroBindings::default())
            .expect("capture query path");
        let json = serde_json::to_string(&action).expect("serialize query action");
        assert!(!json.contains(&old_node.to_string()));
        assert!(json.contains("\"path\":[1]"));

        let replay = model_with_query(2);
        let crate::NestedResourceDraftReadModel::QueryDefinition(replay_root) =
            &replay.resource_editor.drafts[0].nested
        else {
            panic!("query topology");
        };
        let expected = replay_root.children[1].node_id;
        assert!(matches!(
            action.resolve(&replay, &MacroBindings::default()),
            Ok(AppIntent::ApplyResourceMutation(mutation))
                if matches!(
                    mutation.as_ref(),
                    crate::ResourceMutation::QueryDefinition(crate::QueryDefinitionMutation::Tree(
                        crate::QueryTreeMutation::Replace { node_id, .. }
                    )) if *node_id == expected
                )
        ));
        assert!(matches!(
            action.resolve(&model_with_query(1), &MacroBindings::default()),
            Err(MacroError::TopologyMismatch(_))
        ));
    }
}
