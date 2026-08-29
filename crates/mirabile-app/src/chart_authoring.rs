use mirabile_core::{
    CalculationSpec, CalendarSpec, ChartDefinition, ChartRecord, ChartSource, CivilDate,
    CivilDateTime, CivilTime, CoordinateSystem, CorrectionSpec, EventKind, HouseSystem, Latitude,
    LifeEvent, LocationAssertion, Longitude, Note, Offset, ResourceEnvelope, SchemaVersion,
    SourceProvenance, SourceType, SubjectInfo, TemporalAssertion, TimeZoneAssertion, Timestamp,
    ZodiacSpec,
};
use serde::{Deserialize, Serialize};

use crate::{
    ChartDraft, DraftItemId, DraftListMutation, InstanceId, LifeEventDraftReadModel, ResourceId,
    Revision, StableDraftItemReadModel, StableDraftList,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ChartMutation {
    SetTitle(String),
    SetDefinitionDescription(Option<String>),
    SetDefinitionTags(Vec<String>),
    SetRecordTitle(String),
    SetRecordDescription(Option<String>),
    SetRecordTags(Vec<String>),
    SetEventKind(EventKind),
    SetSubjectName(Option<String>),
    SetCivilDate(CivilDate),
    SetCivilTime(CivilTime),
    SetTimezone(ChartTimezone),
    SetLocationEnabled(bool),
    SetLocationName(String),
    SetCountryRegion(Option<String>),
    SetLatitude(Option<Latitude>),
    SetLongitude(Option<Longitude>),
    SetZodiac(ZodiacSpec),
    SetHouseSystem(HouseSystem),
    SetCoordinateSystem(CoordinateSystem),
    SetRecordDetails(Box<ChartRecord>),
    Notes(DraftListMutation<Note>),
    LifeEvents(DraftListMutation<LifeEvent>),
    LifeEventNotes {
        life_event_id: DraftItemId,
        mutation: DraftListMutation<Note>,
    },
    SetCalculation(CalculationSpec),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", content = "offset", rename_all = "snake_case")]
pub enum ChartTimezone {
    UniversalTime,
    FixedOffset(Offset),
    NamedZone(String),
    LocalMeanTime,
    LocalApparentTime,
    Unknown,
}

impl From<&TimeZoneAssertion> for ChartTimezone {
    fn from(value: &TimeZoneAssertion) -> Self {
        match value {
            TimeZoneAssertion::UniversalTime => Self::UniversalTime,
            TimeZoneAssertion::FixedOffset(offset) => Self::FixedOffset(*offset),
            TimeZoneAssertion::NamedZone(name) => Self::NamedZone(name.clone()),
            TimeZoneAssertion::LocalMeanTime => Self::LocalMeanTime,
            TimeZoneAssertion::LocalApparentTime => Self::LocalApparentTime,
            TimeZoneAssertion::Unknown => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum ChartEditorTarget {
    New {
        instance_id: InstanceId,
    },
    Saved {
        instance_id: InstanceId,
        record_id: ResourceId,
        definition_id: ResourceId,
        record_base_revision: Revision,
        definition_base_revision: Revision,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartEditorState {
    Clean,
    Dirty,
    Saving,
    Conflict,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartConflictComponent {
    Record,
    Definition,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChartEditorConflict {
    pub component: ChartConflictComponent,
    pub resource_id: ResourceId,
    pub expected_revision: Revision,
    pub actual_revision: Revision,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChartValidationIssue {
    pub field: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ManualLocationReadModel {
    pub enabled: bool,
    pub display_name: String,
    pub country_region: Option<String>,
    pub latitude: Option<Latitude>,
    pub longitude: Option<Longitude>,
}

impl ManualLocationReadModel {
    pub const fn is_complete(&self) -> bool {
        self.enabled
            && !self.display_name.is_empty()
            && self.latitude.is_some()
            && self.longitude.is_some()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartEditorFieldsReadModel {
    pub definition_metadata: ChartComponentMetadataReadModel,
    pub record_metadata: ChartComponentMetadataReadModel,
    pub event_kind: EventKind,
    pub subject_name: Option<String>,
    pub civil_date: CivilDate,
    pub civil_time: CivilTime,
    pub timezone: ChartTimezone,
    pub location: ManualLocationReadModel,
    pub zodiac: ZodiacSpec,
    pub houses: HouseSystem,
    pub coordinates: CoordinateSystem,
    pub record: ChartRecord,
    pub calculation: CalculationSpec,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartComponentMetadataReadModel {
    pub resource_id: Option<ResourceId>,
    pub schema_version: Option<SchemaVersion>,
    pub revision: Option<Revision>,
    pub created_at: Option<Timestamp>,
    pub modified_at: Option<Timestamp>,
    pub title: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartEditorReadModel {
    pub target: ChartEditorTarget,
    pub state: ChartEditorState,
    pub fields: ChartEditorFieldsReadModel,
    pub validation: Vec<ChartValidationIssue>,
    pub last_valid_preview_present: bool,
    pub factual_mutations_enabled: bool,
    pub factual_mutations_disabled_reason: Option<String>,
    pub conflicts: Vec<ChartEditorConflict>,
    pub notes: Vec<StableDraftItemReadModel<Note>>,
    pub life_events: Vec<LifeEventDraftReadModel>,
}

#[derive(Clone)]
pub(crate) struct ChartAuthoringEditor {
    pub target: ChartEditorTarget,
    pub state: ChartEditorState,
    draft: ChartAuthoringDraft,
    pub last_valid: ChartDraft,
    pub validation: Vec<ChartValidationIssue>,
    saved: Option<SavedChartBases>,
    pub conflicts: Vec<ChartEditorConflict>,
    notes: StableDraftList<Note>,
    life_events: StableDraftList<ChartLifeEventDraft>,
}

#[derive(Clone, Debug, PartialEq)]
struct ChartLifeEventDraft {
    value: LifeEvent,
    notes: StableDraftList<Note>,
}

impl ChartLifeEventDraft {
    fn from_canonical(value: &LifeEvent) -> Self {
        Self {
            value: value.clone(),
            notes: StableDraftList::from_canonical(&value.notes),
        }
    }
    fn materialize(&self) -> LifeEvent {
        let mut value = self.value.clone();
        value.notes = self.notes.canonical_values();
        value
    }
}

#[derive(Clone)]
pub(crate) struct SavedChartBases {
    pub record: ResourceEnvelope<ChartRecord>,
    pub definition: ResourceEnvelope<ChartDefinition>,
    pub shared_record: bool,
}

impl ChartAuthoringEditor {
    pub(crate) fn new(
        instance_id: InstanceId,
        civil_datetime: CivilDateTime,
        corrections: CorrectionSpec,
    ) -> Self {
        let draft = ChartAuthoringDraft {
            title: "Untitled Chart".into(),
            definition_description: None,
            definition_tags: Vec::new(),
            record_title: "Untitled Chart source".into(),
            record_description: None,
            record_tags: Vec::new(),
            event_kind: EventKind::Birth,
            subject_name: None,
            civil_date: civil_datetime.date,
            civil_time: civil_datetime.time,
            timezone: ChartTimezone::UniversalTime,
            location_enabled: false,
            location_name: String::new(),
            country_region: None,
            latitude: None,
            longitude: None,
            calculation: CalculationSpec {
                zodiac: ZodiacSpec::Tropical,
                houses: HouseSystem::NoHouses,
                coordinates: CoordinateSystem::Geocentric,
                corrections,
                ..CalculationSpec::default()
            },
            record_template: ChartRecord {
                event_kind: EventKind::Birth,
                subject: None,
                time: TemporalAssertion {
                    civil_datetime,
                    calendar: CalendarSpec::ProlepticGregorian,
                    zone: TimeZoneAssertion::UniversalTime,
                    disambiguation: None,
                },
                location: None,
                source: SourceProvenance {
                    description: "Entered in Mirabile Workbench".into(),
                    source_type: SourceType::UserAssertion,
                    recorded_by: None,
                },
                notes: Vec::new(),
                life_events: Vec::new(),
            },
        };
        let last_valid = draft
            .materialize()
            .expect("application-owned new chart defaults are complete");
        Self {
            target: ChartEditorTarget::New { instance_id },
            state: ChartEditorState::Clean,
            draft,
            last_valid,
            validation: Vec::new(),
            saved: None,
            conflicts: Vec::new(),
            notes: StableDraftList::from_canonical(&[]),
            life_events: StableDraftList::from_canonical(&[]),
        }
    }

    pub(crate) fn from_saved(
        instance_id: InstanceId,
        record: ResourceEnvelope<ChartRecord>,
        definition: ResourceEnvelope<ChartDefinition>,
        shared_record: bool,
    ) -> Result<Self, &'static str> {
        if !matches!(definition.payload.source, ChartSource::Radix { .. }) {
            return Err("Derived chart editing remains intentionally deferred");
        }
        let timezone = ChartTimezone::from(&record.payload.time.zone);
        let location = record.payload.location.as_ref();
        let draft = ChartAuthoringDraft {
            title: definition.title.clone(),
            definition_description: definition.description.clone(),
            definition_tags: definition.tags.clone(),
            record_title: record.title.clone(),
            record_description: record.description.clone(),
            record_tags: record.tags.clone(),
            event_kind: record.payload.event_kind.clone(),
            subject_name: record
                .payload
                .subject
                .as_ref()
                .map(|subject| subject.display_name.clone()),
            civil_date: record.payload.time.civil_datetime.date,
            civil_time: record.payload.time.civil_datetime.time,
            timezone,
            location_enabled: location.is_some(),
            location_name: location
                .map(|location| location.display_name.clone())
                .unwrap_or_default(),
            country_region: location.and_then(|location| location.country_region.clone()),
            latitude: location.map(|location| location.latitude),
            longitude: location.map(|location| location.longitude),
            calculation: definition.payload.calculation.clone(),
            record_template: record.payload.clone(),
        };
        let last_valid = ChartDraft {
            title: definition.title.clone(),
            definition_description: definition.description.clone(),
            definition_tags: definition.tags.clone(),
            record_title: record.title.clone(),
            record_description: record.description.clone(),
            record_tags: record.tags.clone(),
            record: record.payload.clone(),
            calculation: definition.payload.calculation.clone(),
        };
        let notes = StableDraftList::from_canonical(&record.payload.notes);
        let life_events = StableDraftList::from_canonical(
            &record
                .payload
                .life_events
                .iter()
                .map(ChartLifeEventDraft::from_canonical)
                .collect::<Vec<_>>(),
        );
        Ok(Self {
            target: ChartEditorTarget::Saved {
                instance_id,
                record_id: record.id,
                definition_id: definition.id,
                record_base_revision: record.revision,
                definition_base_revision: definition.revision,
            },
            state: ChartEditorState::Clean,
            draft,
            last_valid,
            validation: Vec::new(),
            saved: Some(SavedChartBases {
                record,
                definition,
                shared_record,
            }),
            conflicts: Vec::new(),
            notes,
            life_events,
        })
    }

    pub(crate) fn saved_bases(&self) -> Option<&SavedChartBases> {
        self.saved.as_ref()
    }

    pub(crate) fn factual_mutations_enabled(&self) -> bool {
        !self.saved.as_ref().is_some_and(|saved| saved.shared_record)
    }

    pub(crate) const fn is_factual_mutation(mutation: &ChartMutation) -> bool {
        matches!(
            mutation,
            ChartMutation::SetEventKind(_)
                | ChartMutation::SetRecordTitle(_)
                | ChartMutation::SetRecordDescription(_)
                | ChartMutation::SetRecordTags(_)
                | ChartMutation::SetSubjectName(_)
                | ChartMutation::SetCivilDate(_)
                | ChartMutation::SetCivilTime(_)
                | ChartMutation::SetTimezone(_)
                | ChartMutation::SetLocationEnabled(_)
                | ChartMutation::SetLocationName(_)
                | ChartMutation::SetCountryRegion(_)
                | ChartMutation::SetLatitude(_)
                | ChartMutation::SetLongitude(_)
                | ChartMutation::SetRecordDetails(_)
                | ChartMutation::Notes(_)
                | ChartMutation::LifeEvents(_)
                | ChartMutation::LifeEventNotes { .. }
        )
    }

    pub(crate) fn instance_id(&self) -> InstanceId {
        match self.target {
            ChartEditorTarget::New { instance_id }
            | ChartEditorTarget::Saved { instance_id, .. } => instance_id,
        }
    }

    pub(crate) fn location_complete(&self) -> bool {
        self.draft.location_enabled
            && !self.draft.location_name.trim().is_empty()
            && self.draft.latitude.is_some()
            && self.draft.longitude.is_some()
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn apply(&mut self, mutation: ChartMutation) -> Option<ChartDraft> {
        match mutation {
            ChartMutation::SetTitle(value) => self.draft.title = value,
            ChartMutation::SetDefinitionDescription(value) => {
                self.draft.definition_description = value;
            }
            ChartMutation::SetDefinitionTags(value) => self.draft.definition_tags = value,
            ChartMutation::SetRecordTitle(value) => self.draft.record_title = value,
            ChartMutation::SetRecordDescription(value) => self.draft.record_description = value,
            ChartMutation::SetRecordTags(value) => self.draft.record_tags = value,
            ChartMutation::SetEventKind(value) => self.draft.event_kind = value,
            ChartMutation::SetSubjectName(value) => self.draft.subject_name = value,
            ChartMutation::SetCivilDate(value) => self.draft.civil_date = value,
            ChartMutation::SetCivilTime(value) => self.draft.civil_time = value,
            ChartMutation::SetTimezone(value) => self.draft.timezone = value,
            ChartMutation::SetLocationEnabled(value) => self.draft.location_enabled = value,
            ChartMutation::SetLocationName(value) => self.draft.location_name = value,
            ChartMutation::SetCountryRegion(value) => self.draft.country_region = value,
            ChartMutation::SetLatitude(value) => self.draft.latitude = value,
            ChartMutation::SetLongitude(value) => self.draft.longitude = value,
            ChartMutation::SetZodiac(value) => self.draft.calculation.zodiac = value,
            ChartMutation::SetHouseSystem(value) => self.draft.calculation.houses = value,
            ChartMutation::SetCoordinateSystem(value) => {
                self.draft.calculation.coordinates = value;
            }
            ChartMutation::SetRecordDetails(value) => {
                self.draft.event_kind = value.event_kind.clone();
                self.draft.subject_name = value
                    .subject
                    .as_ref()
                    .map(|subject| subject.display_name.clone());
                self.draft.civil_date = value.time.civil_datetime.date;
                self.draft.civil_time = value.time.civil_datetime.time;
                self.draft.timezone = ChartTimezone::from(&value.time.zone);
                self.draft.location_enabled = value.location.is_some();
                self.draft.location_name = value
                    .location
                    .as_ref()
                    .map(|location| location.display_name.clone())
                    .unwrap_or_default();
                self.draft.country_region = value
                    .location
                    .as_ref()
                    .and_then(|location| location.country_region.clone());
                self.draft.latitude = value.location.as_ref().map(|location| location.latitude);
                self.draft.longitude = value.location.as_ref().map(|location| location.longitude);
                self.draft.record_template = *value;
            }
            ChartMutation::Notes(mutation) => {
                if let Err(message) = self.notes.apply(mutation) {
                    self.validation = vec![ChartValidationIssue {
                        field: "notes".into(),
                        message: message.into(),
                    }];
                    return None;
                }
            }
            ChartMutation::LifeEvents(mutation) => {
                let mutation = match mutation {
                    DraftListMutation::Insert { after, value } => DraftListMutation::Insert {
                        after,
                        value: ChartLifeEventDraft::from_canonical(&value),
                    },
                    DraftListMutation::Update { item_id, value } => {
                        let notes = self
                            .life_events
                            .items()
                            .iter()
                            .find(|item| item.id == item_id)
                            .map_or_else(
                                || StableDraftList::from_canonical(&value.notes),
                                |item| item.value.notes.clone(),
                            );
                        DraftListMutation::Update {
                            item_id,
                            value: ChartLifeEventDraft { value, notes },
                        }
                    }
                    DraftListMutation::Remove { item_id } => DraftListMutation::Remove { item_id },
                    DraftListMutation::Move { item_id, before } => {
                        DraftListMutation::Move { item_id, before }
                    }
                };
                if let Err(message) = self.life_events.apply(mutation) {
                    self.validation = vec![ChartValidationIssue {
                        field: "life_events".into(),
                        message: message.into(),
                    }];
                    return None;
                }
            }
            ChartMutation::LifeEventNotes {
                life_event_id,
                mutation,
            } => {
                let Some(event) = self
                    .life_events
                    .items_mut()
                    .iter_mut()
                    .find(|item| item.id == life_event_id)
                else {
                    self.validation = vec![ChartValidationIssue {
                        field: "life_events.notes".into(),
                        message: "Life event was not found".into(),
                    }];
                    return None;
                };
                if let Err(message) = event.value.notes.apply(mutation) {
                    self.validation = vec![ChartValidationIssue {
                        field: "life_events.notes".into(),
                        message: message.into(),
                    }];
                    return None;
                }
            }
            ChartMutation::SetCalculation(value) => self.draft.calculation = value,
        }
        self.draft.record_template.notes = self.notes.canonical_values();
        self.draft.record_template.life_events = self
            .life_events
            .items()
            .iter()
            .map(|item| item.value.materialize())
            .collect();
        self.state = ChartEditorState::Dirty;
        self.conflicts.clear();
        match self.draft.materialize() {
            Ok(materialized) => {
                self.validation.clear();
                self.last_valid = materialized.clone();
                Some(materialized)
            }
            Err(validation) => {
                self.validation = validation;
                None
            }
        }
    }

    pub(crate) fn read_model(&self) -> ChartEditorReadModel {
        let notes = self
            .notes
            .items()
            .iter()
            .map(|item| StableDraftItemReadModel {
                item_id: item.id,
                value: item.value.clone(),
            })
            .collect();
        let life_events = self
            .life_events
            .items()
            .iter()
            .map(|item| LifeEventDraftReadModel {
                item_id: item.id,
                value: item.value.materialize(),
                notes: item
                    .value
                    .notes
                    .items()
                    .iter()
                    .map(|note| StableDraftItemReadModel {
                        item_id: note.id,
                        value: note.value.clone(),
                    })
                    .collect(),
            })
            .collect();
        ChartEditorReadModel {
            target: self.target.clone(),
            state: self.state,
            fields: ChartEditorFieldsReadModel {
                definition_metadata: component_metadata(
                    self.saved.as_ref().map(|saved| &saved.definition),
                    &self.draft.title,
                    self.draft.definition_description.as_deref(),
                    &self.draft.definition_tags,
                ),
                record_metadata: component_metadata(
                    self.saved.as_ref().map(|saved| &saved.record),
                    &self.draft.record_title,
                    self.draft.record_description.as_deref(),
                    &self.draft.record_tags,
                ),
                event_kind: self.draft.event_kind.clone(),
                subject_name: self.draft.subject_name.clone(),
                civil_date: self.draft.civil_date,
                civil_time: self.draft.civil_time,
                timezone: self.draft.timezone.clone(),
                location: ManualLocationReadModel {
                    enabled: self.draft.location_enabled,
                    display_name: self.draft.location_name.clone(),
                    country_region: self.draft.country_region.clone(),
                    latitude: self.draft.latitude,
                    longitude: self.draft.longitude,
                },
                zodiac: self.draft.calculation.zodiac.clone(),
                houses: self.draft.calculation.houses,
                coordinates: self.draft.calculation.coordinates,
                record: self.last_valid.record.clone(),
                calculation: self.draft.calculation.clone(),
            },
            validation: self.validation.clone(),
            last_valid_preview_present: true,
            factual_mutations_enabled: self.factual_mutations_enabled(),
            factual_mutations_disabled_reason: (!self.factual_mutations_enabled()).then(|| {
                "This ChartRecord is shared by multiple definitions; copy/detach is required before factual editing"
                    .into()
            }),
            conflicts: self.conflicts.clone(),
            notes,
            life_events,
        }
    }
}

#[derive(Clone)]
struct ChartAuthoringDraft {
    title: String,
    definition_description: Option<String>,
    definition_tags: Vec<String>,
    record_title: String,
    record_description: Option<String>,
    record_tags: Vec<String>,
    event_kind: EventKind,
    subject_name: Option<String>,
    civil_date: CivilDate,
    civil_time: CivilTime,
    timezone: ChartTimezone,
    location_enabled: bool,
    location_name: String,
    country_region: Option<String>,
    latitude: Option<Latitude>,
    longitude: Option<Longitude>,
    calculation: CalculationSpec,
    record_template: ChartRecord,
}

impl ChartAuthoringDraft {
    #[allow(clippy::too_many_lines)]
    fn materialize(&self) -> Result<ChartDraft, Vec<ChartValidationIssue>> {
        let mut validation = Vec::new();
        validate_component_metadata(
            "definition",
            &self.title,
            &self.definition_tags,
            &mut validation,
        );
        validate_component_metadata(
            "record",
            &self.record_title,
            &self.record_tags,
            &mut validation,
        );
        let location = if self.location_enabled {
            if self.location_name.trim().is_empty() {
                validation.push(ChartValidationIssue {
                    field: "location.display_name".into(),
                    message: "Location name is required when manual location is enabled".into(),
                });
            }
            if self.latitude.is_none() {
                validation.push(ChartValidationIssue {
                    field: "location.latitude".into(),
                    message: "Latitude is required when manual location is enabled".into(),
                });
            }
            if self.longitude.is_none() {
                validation.push(ChartValidationIssue {
                    field: "location.longitude".into(),
                    message: "Longitude is required when manual location is enabled".into(),
                });
            }
            match (self.latitude, self.longitude) {
                (Some(latitude), Some(longitude)) if !self.location_name.trim().is_empty() => {
                    Some(LocationAssertion {
                        display_name: self.location_name.trim().into(),
                        country_region: self.country_region.clone(),
                        latitude,
                        longitude,
                        atlas_provenance: None,
                    })
                }
                _ => None,
            }
        } else {
            None
        };
        if self.calculation.houses != HouseSystem::NoHouses && location.is_none() {
            validation.push(ChartValidationIssue {
                field: "houses".into(),
                message: "A complete manual location is required for houses".into(),
            });
        }
        if !validation.is_empty() {
            return Err(validation);
        }
        let zone = match &self.timezone {
            ChartTimezone::UniversalTime => TimeZoneAssertion::UniversalTime,
            ChartTimezone::FixedOffset(offset) => TimeZoneAssertion::FixedOffset(*offset),
            ChartTimezone::NamedZone(name) => TimeZoneAssertion::NamedZone(name.clone()),
            ChartTimezone::LocalMeanTime => TimeZoneAssertion::LocalMeanTime,
            ChartTimezone::LocalApparentTime => TimeZoneAssertion::LocalApparentTime,
            ChartTimezone::Unknown => TimeZoneAssertion::Unknown,
        };
        let mut record = self.record_template.clone();
        record.event_kind = self.event_kind.clone();
        record.subject = self.subject_name.as_ref().and_then(|name| {
            (!name.trim().is_empty()).then(|| SubjectInfo {
                display_name: name.trim().into(),
                pronouns: self.record_template.subject.as_ref().and_then(|subject| {
                    (subject.display_name.trim() == name.trim())
                        .then(|| subject.pronouns.clone())
                        .flatten()
                }),
            })
        });
        record.time.civil_datetime = CivilDateTime {
            date: self.civil_date,
            time: self.civil_time,
        };
        record.time.zone = zone;
        record.location = location.map(|mut location| {
            if self
                .record_template
                .location
                .as_ref()
                .is_some_and(|original| {
                    original.display_name == location.display_name
                        && original.country_region == location.country_region
                        && original.latitude == location.latitude
                        && original.longitude == location.longitude
                })
            {
                location.atlas_provenance = self
                    .record_template
                    .location
                    .as_ref()
                    .and_then(|original| original.atlas_provenance.clone());
            }
            location
        });
        Ok(ChartDraft {
            title: self.title.trim().into(),
            definition_description: self.definition_description.clone(),
            definition_tags: self.definition_tags.clone(),
            record_title: self.record_title.trim().into(),
            record_description: self.record_description.clone(),
            record_tags: self.record_tags.clone(),
            record,
            calculation: self.calculation.clone(),
        })
    }
}

fn component_metadata<T>(
    envelope: Option<&ResourceEnvelope<T>>,
    title: &str,
    description: Option<&str>,
    tags: &[String],
) -> ChartComponentMetadataReadModel {
    ChartComponentMetadataReadModel {
        resource_id: envelope.map(|value| value.id),
        schema_version: envelope.map(|value| value.schema_version),
        revision: envelope.map(|value| value.revision),
        created_at: envelope.map(|value| value.created_at),
        modified_at: envelope.map(|value| value.modified_at),
        title: title.into(),
        description: description.map(str::to_owned),
        tags: tags.to_vec(),
    }
}

fn validate_component_metadata(
    component: &str,
    title: &str,
    tags: &[String],
    validation: &mut Vec<ChartValidationIssue>,
) {
    if title.trim().is_empty() {
        validation.push(ChartValidationIssue {
            field: format!("{component}.title"),
            message: "Resource title is required".into(),
        });
    }
    if tags.iter().any(|tag| tag.trim().is_empty()) {
        validation.push(ChartValidationIssue {
            field: format!("{component}.tags"),
            message: "Tags must not be empty".into(),
        });
    }
    let mut normalized = tags
        .iter()
        .map(|tag| tag.trim().to_owned())
        .collect::<Vec<_>>();
    normalized.sort();
    if normalized.windows(2).any(|pair| pair[0] == pair[1]) {
        validation.push(ChartValidationIssue {
            field: format!("{component}.tags"),
            message: "Tags must be unique".into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> ChartAuthoringEditor {
        ChartAuthoringEditor::new(
            InstanceId::new(),
            CivilDateTime {
                date: CivilDate::new(2026, 8, 24).expect("date"),
                time: CivilTime::new(12, 0, 0).expect("time"),
            },
            CorrectionSpec::default(),
        )
    }

    #[test]
    fn incomplete_location_retains_last_valid_preview() {
        let mut editor = editor();
        let original = editor.last_valid.clone();
        assert!(
            editor
                .apply(ChartMutation::SetLocationEnabled(true))
                .is_none()
        );
        assert_eq!(editor.last_valid, original);
        assert_eq!(editor.validation.len(), 3);
        editor.apply(ChartMutation::SetLocationName("Baltimore".into()));
        editor.apply(ChartMutation::SetLatitude(Some(
            Latitude::from_degrees(39.29).expect("latitude"),
        )));
        let materialized = editor
            .apply(ChartMutation::SetLongitude(Some(
                Longitude::from_degrees(-76.61).expect("longitude"),
            )))
            .expect("complete location materializes");
        assert_eq!(
            materialized.record.location.expect("location").display_name,
            "Baltimore"
        );
        assert!(editor.validation.is_empty());
    }

    #[test]
    fn nested_chart_facts_keep_stable_ids_and_materialize_in_order() {
        let mut editor = editor();
        let first = Note {
            text: "first".into(),
            created_at: mirabile_core::Timestamp::from_unix_millis(10),
        };
        let second = Note {
            text: "second".into(),
            created_at: mirabile_core::Timestamp::from_unix_millis(20),
        };
        editor.apply(ChartMutation::Notes(DraftListMutation::Insert {
            after: None,
            value: first,
        }));
        let first_id = editor.read_model().notes[0].item_id;
        editor.apply(ChartMutation::Notes(DraftListMutation::Insert {
            after: Some(first_id),
            value: second.clone(),
        }));
        let second_id = editor.read_model().notes[1].item_id;
        editor.apply(ChartMutation::Notes(DraftListMutation::Move {
            item_id: second_id,
            before: Some(first_id),
        }));
        assert_eq!(
            editor
                .read_model()
                .notes
                .iter()
                .map(|row| row.item_id)
                .collect::<Vec<_>>(),
            vec![second_id, first_id]
        );

        let event = LifeEvent {
            title: "Milestone".into(),
            time: editor.last_valid.record.time.clone(),
            location: None,
            notes: Vec::new(),
        };
        editor.apply(ChartMutation::LifeEvents(DraftListMutation::Insert {
            after: None,
            value: event,
        }));
        let event_id = editor.read_model().life_events[0].item_id;
        editor.apply(ChartMutation::LifeEventNotes {
            life_event_id: event_id,
            mutation: DraftListMutation::Insert {
                after: None,
                value: Note {
                    text: "nested".into(),
                    created_at: mirabile_core::Timestamp::from_unix_millis(30),
                },
            },
        });
        let read = editor.read_model();
        assert_eq!(read.life_events[0].item_id, event_id);
        assert_eq!(read.life_events[0].notes[0].value.text, "nested");
        assert_eq!(
            editor.last_valid.record.notes,
            vec![
                second,
                Note {
                    text: "first".into(),
                    created_at: mirabile_core::Timestamp::from_unix_millis(10)
                }
            ]
        );
        assert_eq!(
            editor.last_valid.record.life_events[0].notes[0].text,
            "nested"
        );
    }

    #[test]
    fn complete_record_details_sync_authoritative_timezone_and_location_provenance() {
        let mut editor = editor();
        let mut record = editor.last_valid.record.clone();
        record.time.calendar = CalendarSpec::HistoricalTransition {
            identifier: "british-1752".into(),
        };
        record.time.zone = TimeZoneAssertion::NamedZone("America/New_York".into());
        record.time.disambiguation = Some(mirabile_core::TimeChoice::Later);
        record.location = Some(LocationAssertion {
            display_name: "Baltimore".into(),
            country_region: Some("US-MD".into()),
            latitude: Latitude::from_degrees(39.2904).expect("latitude"),
            longitude: Longitude::from_degrees(-76.6122).expect("longitude"),
            atlas_provenance: Some(mirabile_core::AtlasRef {
                provider: "Test Atlas".into(),
                record_id: Some("bwi".into()),
                data_version: Some("2026a".into()),
            }),
        });
        let materialized = editor
            .apply(ChartMutation::SetRecordDetails(Box::new(record.clone())))
            .expect("complete details");
        assert_eq!(materialized.record, record);
        assert_eq!(
            editor.read_model().fields.timezone,
            ChartTimezone::NamedZone("America/New_York".into())
        );
    }

    #[test]
    fn component_metadata_is_independent_and_invalid_tags_retain_last_valid_preview() {
        let mut editor = editor();
        editor
            .apply(ChartMutation::SetRecordTitle("Factual record".into()))
            .expect("record title");
        editor
            .apply(ChartMutation::SetRecordDescription(Some(
                "Factual metadata".into(),
            )))
            .expect("record description");
        editor
            .apply(ChartMutation::SetDefinitionDescription(Some(
                "Calculation metadata".into(),
            )))
            .expect("definition description");
        let valid = editor.last_valid.clone();
        assert_eq!(valid.record_title, "Factual record");
        assert_eq!(
            valid.definition_description.as_deref(),
            Some("Calculation metadata")
        );

        assert!(
            editor
                .apply(ChartMutation::SetRecordTags(vec![
                    "duplicate".into(),
                    "duplicate".into(),
                ]))
                .is_none()
        );
        assert_eq!(editor.last_valid, valid);
        assert_eq!(editor.validation[0].field, "record.tags");
        editor
            .apply(ChartMutation::SetRecordTags(vec!["source".into()]))
            .expect("corrected tags");
        assert!(editor.validation.is_empty());
    }
}
