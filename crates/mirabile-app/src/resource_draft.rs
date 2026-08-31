use std::fmt;

use mirabile_core::{
    AnalysisProfile, AspectDefinition, AspectSet, CalculationSpec, CanonicalResource,
    ChartDefinition, ChartRecord, ChartSlot, ChartSource, EventKind, LifeEvent, LocationAssertion,
    Note, PageLayout, PointSelector, PointSet, QueryDefinition, QueryExpr, ResourceId,
    ResourceKind, RingSpec, SourceProvenance, SubjectInfo, TemporalAssertion, Theme, ViewDocument,
    ViewInstance, ViewObject, WheelTemplate, WorkspaceDocument, WorkspaceDocumentChart,
    WorkspaceProfile,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DraftItemId(Uuid);

impl DraftItemId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DraftItemId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DraftItemId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceDraftKind {
    ChartRecord,
    ChartDefinition,
    PointSet,
    AspectSet,
    AnalysisProfile,
    WheelTemplate,
    ViewDocument,
    Theme,
    QueryDefinition,
    WorkspaceDocument,
}

impl ResourceDraftKind {
    pub const ALL: [Self; 10] = [
        Self::ChartRecord,
        Self::ChartDefinition,
        Self::PointSet,
        Self::AspectSet,
        Self::AnalysisProfile,
        Self::WheelTemplate,
        Self::ViewDocument,
        Self::Theme,
        Self::QueryDefinition,
        Self::WorkspaceDocument,
    ];

    pub const fn resource_kind(self) -> ResourceKind {
        match self {
            Self::ChartRecord => ResourceKind::ChartRecord,
            Self::ChartDefinition => ResourceKind::ChartDefinition,
            Self::PointSet => ResourceKind::PointSet,
            Self::AspectSet => ResourceKind::AspectSet,
            Self::AnalysisProfile => ResourceKind::AnalysisProfile,
            Self::WheelTemplate => ResourceKind::WheelTemplate,
            Self::ViewDocument => ResourceKind::ViewDocument,
            Self::Theme => ResourceKind::Theme,
            Self::QueryDefinition => ResourceKind::QueryDefinition,
            Self::WorkspaceDocument => ResourceKind::WorkspaceDocument,
        }
    }
}

impl TryFrom<ResourceKind> for ResourceDraftKind {
    type Error = ResourceKind;

    fn try_from(kind: ResourceKind) -> Result<Self, Self::Error> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.resource_kind() == kind)
            .ok_or(kind)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ResourceMetadataMutation {
    SetTitle(String),
    SetDescription(Option<String>),
    SetTags(Vec<String>),
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ResourceMutation {
    ChartRecord(ChartRecordMutation),
    ChartDefinition(ChartDefinitionMutation),
    PointSet(PointSetMutation),
    AspectSet(AspectSetMutation),
    AnalysisProfile(AnalysisProfileMutation),
    WheelTemplate(WheelTemplateMutation),
    ViewDocument(ViewDocumentMutation),
    Theme(ThemeMutation),
    QueryDefinition(QueryDefinitionMutation),
    WorkspaceDocument(WorkspaceDocumentMutation),
}

impl ResourceMutation {
    pub const fn kind(&self) -> ResourceDraftKind {
        match self {
            Self::ChartRecord(_) => ResourceDraftKind::ChartRecord,
            Self::ChartDefinition(_) => ResourceDraftKind::ChartDefinition,
            Self::PointSet(_) => ResourceDraftKind::PointSet,
            Self::AspectSet(_) => ResourceDraftKind::AspectSet,
            Self::AnalysisProfile(_) => ResourceDraftKind::AnalysisProfile,
            Self::WheelTemplate(_) => ResourceDraftKind::WheelTemplate,
            Self::ViewDocument(_) => ResourceDraftKind::ViewDocument,
            Self::Theme(_) => ResourceDraftKind::Theme,
            Self::QueryDefinition(_) => ResourceDraftKind::QueryDefinition,
            Self::WorkspaceDocument(_) => ResourceDraftKind::WorkspaceDocument,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "resource_type", content = "value", rename_all = "snake_case")]
pub enum ResourceDraftValueReadModel {
    ChartRecord(ChartRecord),
    ChartDefinition(ChartDefinition),
    PointSet(PointSet),
    AspectSet(AspectSet),
    AnalysisProfile(AnalysisProfile),
    WheelTemplate(WheelTemplate),
    ViewDocument(ViewDocument),
    Theme(Theme),
    QueryDefinition(QueryDefinition),
    WorkspaceDocument(WorkspaceDocument),
}

impl From<&CanonicalResource> for ResourceDraftValueReadModel {
    fn from(resource: &CanonicalResource) -> Self {
        match resource {
            CanonicalResource::ChartRecord(envelope) => Self::ChartRecord(envelope.payload.clone()),
            CanonicalResource::ChartDefinition(envelope) => {
                Self::ChartDefinition(envelope.payload.clone())
            }
            CanonicalResource::PointSet(envelope) => Self::PointSet(envelope.payload.clone()),
            CanonicalResource::AspectSet(envelope) => Self::AspectSet(envelope.payload.clone()),
            CanonicalResource::AnalysisProfile(envelope) => {
                Self::AnalysisProfile(envelope.payload.clone())
            }
            CanonicalResource::WheelTemplate(envelope) => {
                Self::WheelTemplate(envelope.payload.clone())
            }
            CanonicalResource::ViewDocument(envelope) => {
                Self::ViewDocument(envelope.payload.clone())
            }
            CanonicalResource::Theme(envelope) => Self::Theme(envelope.payload.clone()),
            CanonicalResource::QueryDefinition(envelope) => {
                Self::QueryDefinition(envelope.payload.clone())
            }
            CanonicalResource::WorkspaceDocument(envelope) => {
                Self::WorkspaceDocument(envelope.payload.clone())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChartRecordMutation {
    Metadata(ResourceMetadataMutation),
    SetEventKind(EventKind),
    SetSubject(Option<SubjectInfo>),
    SetTime(TemporalAssertion),
    SetLocation(Option<LocationAssertion>),
    SetSource(SourceProvenance),
    Notes(DraftListMutation<Note>),
    LifeEvents(DraftListMutation<LifeEvent>),
    LifeEventNotes {
        life_event_id: DraftItemId,
        mutation: DraftListMutation<Note>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChartDefinitionMutation {
    Metadata(ResourceMetadataMutation),
    SetSource(ChartSource),
    SwitchDerivedRecipe(DerivedRecipeKind),
    MutateDerivedRecipe(DerivedRecipeMutation),
    SetCalculation(CalculationSpec),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivedRecipeKind {
    Transit,
    Harmonic,
    Relocation,
    Composite,
}

impl DerivedRecipeKind {
    pub const ALL: [Self; 4] = [
        Self::Transit,
        Self::Harmonic,
        Self::Relocation,
        Self::Composite,
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub enum DerivedRecipeMutation {
    SetTransit {
        at: TemporalAssertion,
        location: LocationAssertion,
    },
    SetHarmonic {
        radix: ResourceId,
        harmonic: f64,
    },
    SetRelocation {
        radix: ResourceId,
        location: LocationAssertion,
    },
    SetCompositeMethod(mirabile_core::CompositeMethod),
    CompositeCharts(DraftListMutation<ResourceId>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PointSetMutation {
    Metadata(ResourceMetadataMutation),
    Selectors(DraftListMutation<PointSelector>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AspectSetMutation {
    Metadata(ResourceMetadataMutation),
    Aspects(DraftListMutation<AspectDefinition>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnalysisProfileMutation {
    Metadata(ResourceMetadataMutation),
    SetProfile(AnalysisProfile),
}

#[derive(Clone, Debug, PartialEq)]
pub enum WheelTemplateMutation {
    Metadata(ResourceMetadataMutation),
    Rings(DraftListMutation<RingSpec>),
    SetTemplateFields(WheelTemplate),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewDocumentMutation {
    Metadata(ResourceMetadataMutation),
    ChartSlots(DraftListMutation<ChartSlot>),
    InsertChartSlotDefault {
        after: Option<DraftItemId>,
    },
    RenameChartSlot {
        item_id: DraftItemId,
        slot: ChartSlot,
    },
    Objects(DraftListMutation<ViewObject>),
    PointTablePoints {
        object_id: DraftItemId,
        mutation: DraftListMutation<mirabile_core::PointId>,
    },
    SetLayout(PageLayout),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ThemeMutation {
    Metadata(ResourceMetadataMutation),
    SetTheme(Theme),
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryDefinitionMutation {
    Metadata(ResourceMetadataMutation),
    SetDescription(Option<String>),
    Tree(QueryTreeMutation),
}

#[derive(Clone, Debug, PartialEq)]
pub enum QueryTreeMutation {
    Replace {
        node_id: DraftItemId,
        expression: QueryExpr,
    },
    InsertChild {
        parent_id: DraftItemId,
        after: Option<DraftItemId>,
        expression: QueryExpr,
    },
    Remove {
        node_id: DraftItemId,
    },
    Move {
        node_id: DraftItemId,
        new_parent_id: DraftItemId,
        before: Option<DraftItemId>,
    },
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum WorkspaceDocumentMutation {
    Metadata(ResourceMetadataMutation),
    ChartInstances(DraftListMutation<WorkspaceDocumentChart>),
    Views(DraftListMutation<ViewInstance>),
    SetProfile(Box<WorkspaceProfile>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum DraftListMutation<T> {
    Insert {
        after: Option<DraftItemId>,
        value: T,
    },
    Update {
        item_id: DraftItemId,
        value: T,
    },
    Remove {
        item_id: DraftItemId,
    },
    Move {
        item_id: DraftItemId,
        before: Option<DraftItemId>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct StableDraftList<T> {
    items: Vec<StableDraftItem<T>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StableDraftItem<T> {
    pub id: DraftItemId,
    pub value: T,
}

impl<T: Clone> StableDraftList<T> {
    pub fn from_canonical(values: &[T]) -> Self {
        Self {
            items: values
                .iter()
                .cloned()
                .map(|value| StableDraftItem {
                    id: DraftItemId::new(),
                    value,
                })
                .collect(),
        }
    }

    pub fn canonical_values(&self) -> Vec<T> {
        self.items.iter().map(|item| item.value.clone()).collect()
    }

    pub fn items(&self) -> &[StableDraftItem<T>] {
        &self.items
    }

    pub(crate) fn items_mut(&mut self) -> &mut [StableDraftItem<T>] {
        &mut self.items
    }

    pub fn apply(&mut self, mutation: DraftListMutation<T>) -> Result<(), &'static str> {
        match mutation {
            DraftListMutation::Insert { after, value } => {
                let index = after.map_or(Ok(0), |after| {
                    self.items
                        .iter()
                        .position(|item| item.id == after)
                        .map(|index| index + 1)
                        .ok_or("Draft list insertion anchor was not found")
                })?;
                self.items.insert(
                    index,
                    StableDraftItem {
                        id: DraftItemId::new(),
                        value,
                    },
                );
            }
            DraftListMutation::Update { item_id, value } => {
                self.items
                    .iter_mut()
                    .find(|item| item.id == item_id)
                    .ok_or("Draft list item was not found")?
                    .value = value;
            }
            DraftListMutation::Remove { item_id } => {
                let index = self
                    .items
                    .iter()
                    .position(|item| item.id == item_id)
                    .ok_or("Draft list item was not found")?;
                self.items.remove(index);
            }
            DraftListMutation::Move { item_id, before } => {
                if before == Some(item_id) {
                    return Ok(());
                }
                let index = self
                    .items
                    .iter()
                    .position(|item| item.id == item_id)
                    .ok_or("Draft list item was not found")?;
                let item = self.items.remove(index);
                let destination = before.map_or(Ok(self.items.len()), |before| {
                    self.items
                        .iter()
                        .position(|item| item.id == before)
                        .ok_or("Draft list move target was not found")
                })?;
                self.items.insert(destination, item);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DraftItemAddressReadModel {
    pub collection: String,
    pub item_id: DraftItemId,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StableDraftItemReadModel<T> {
    pub item_id: DraftItemId,
    pub value: T,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LifeEventDraftReadModel {
    pub item_id: DraftItemId,
    pub value: LifeEvent,
    pub notes: Vec<StableDraftItemReadModel<Note>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ViewObjectDraftReadModel {
    pub item_id: DraftItemId,
    pub value: ViewObject,
    pub point_table_points: Vec<StableDraftItemReadModel<mirabile_core::PointId>>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceDraftValidationIssue {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct QueryNodeDraftReadModel {
    pub node_id: DraftItemId,
    pub expression: QueryExpr,
    pub children: Vec<QueryNodeDraftReadModel>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "items", rename_all = "snake_case")]
pub enum NestedResourceDraftReadModel {
    None,
    ChartRecord {
        notes: Vec<StableDraftItemReadModel<Note>>,
        life_events: Vec<LifeEventDraftReadModel>,
    },
    ChartDefinition {
        composite_charts: Vec<StableDraftItemReadModel<ResourceId>>,
    },
    PointSet(Vec<StableDraftItemReadModel<PointSelector>>),
    AspectSet(Vec<StableDraftItemReadModel<AspectDefinition>>),
    WheelTemplate(Vec<StableDraftItemReadModel<RingSpec>>),
    ViewDocument {
        chart_slots: Vec<StableDraftItemReadModel<ChartSlot>>,
        objects: Vec<ViewObjectDraftReadModel>,
    },
    QueryDefinition(QueryNodeDraftReadModel),
    WorkspaceDocument {
        charts: Vec<StableDraftItemReadModel<WorkspaceDocumentChart>>,
        views: Vec<StableDraftItemReadModel<ViewInstance>>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceDraftConflictReadModel {
    pub resource_id: ResourceId,
    pub expected_revision: crate::Revision,
    pub actual_revision: crate::Revision,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_kinds_match_exactly_the_canonical_payload_set() {
        assert_eq!(ResourceDraftKind::ALL.len(), 10);
        assert_eq!(
            ResourceDraftKind::ALL.map(ResourceDraftKind::resource_kind),
            mirabile_core::CanonicalResource::KINDS
        );
    }

    #[test]
    fn stable_list_ids_survive_update_and_reordering_but_not_persistence() {
        let mut list = StableDraftList::from_canonical(&["first".to_owned(), "second".to_owned()]);
        let first = list.items()[0].id;
        let second = list.items()[1].id;
        list.apply(DraftListMutation::Update {
            item_id: first,
            value: "updated".into(),
        })
        .expect("update");
        list.apply(DraftListMutation::Move {
            item_id: second,
            before: Some(first),
        })
        .expect("move");
        assert_eq!(list.items()[0].id, second);
        assert_eq!(list.items()[1].id, first);
        assert_eq!(
            list.canonical_values(),
            vec!["second".to_owned(), "updated".to_owned()]
        );
        let json = serde_json::to_string(&list.canonical_values()).expect("canonical JSON");
        assert!(!json.contains(&first.to_string()));
        assert!(!json.contains(&second.to_string()));
    }
}
