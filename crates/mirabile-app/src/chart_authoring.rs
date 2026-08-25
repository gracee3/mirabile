use mirabile_core::{
    CalculationSpec, CalendarSpec, ChartRecord, CivilDate, CivilDateTime, CivilTime,
    CoordinateSystem, CorrectionSpec, EventKind, HouseSystem, Latitude, LocationAssertion,
    Longitude, Offset, SourceProvenance, SourceType, SubjectInfo, TemporalAssertion,
    TimeZoneAssertion, ZodiacSpec,
};
use serde::{Deserialize, Serialize};

use crate::{ChartDraft, InstanceId, ResourceId, Revision};

#[derive(Clone, Debug, PartialEq)]
pub enum ChartMutation {
    SetTitle(String),
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
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", content = "offset", rename_all = "snake_case")]
pub enum ChartTimezone {
    UniversalTime,
    FixedOffset(Offset),
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
    pub title: String,
    pub event_kind: EventKind,
    pub subject_name: Option<String>,
    pub civil_date: CivilDate,
    pub civil_time: CivilTime,
    pub timezone: ChartTimezone,
    pub location: ManualLocationReadModel,
    pub zodiac: ZodiacSpec,
    pub houses: HouseSystem,
    pub coordinates: CoordinateSystem,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartEditorReadModel {
    pub target: ChartEditorTarget,
    pub state: ChartEditorState,
    pub fields: ChartEditorFieldsReadModel,
    pub validation: Vec<ChartValidationIssue>,
    pub last_valid_preview_present: bool,
}

#[derive(Clone)]
pub(crate) struct ChartAuthoringEditor {
    pub target: ChartEditorTarget,
    pub state: ChartEditorState,
    draft: ChartAuthoringDraft,
    pub last_valid: ChartDraft,
    pub validation: Vec<ChartValidationIssue>,
}

impl ChartAuthoringEditor {
    pub(crate) fn new(
        instance_id: InstanceId,
        civil_datetime: CivilDateTime,
        corrections: CorrectionSpec,
    ) -> Self {
        let draft = ChartAuthoringDraft {
            title: "Untitled Chart".into(),
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
        }
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

    pub(crate) fn apply(&mut self, mutation: ChartMutation) -> Option<ChartDraft> {
        match mutation {
            ChartMutation::SetTitle(value) => self.draft.title = value,
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
        }
        self.state = ChartEditorState::Dirty;
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
        ChartEditorReadModel {
            target: self.target.clone(),
            state: self.state,
            fields: ChartEditorFieldsReadModel {
                title: self.draft.title.clone(),
                event_kind: self.draft.event_kind.clone(),
                subject_name: self.draft.subject_name.clone(),
                civil_date: self.draft.civil_date,
                civil_time: self.draft.civil_time,
                timezone: self.draft.timezone,
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
            },
            validation: self.validation.clone(),
            last_valid_preview_present: true,
        }
    }
}

#[derive(Clone)]
struct ChartAuthoringDraft {
    title: String,
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
}

impl ChartAuthoringDraft {
    fn materialize(&self) -> Result<ChartDraft, Vec<ChartValidationIssue>> {
        let mut validation = Vec::new();
        if self.title.trim().is_empty() {
            validation.push(ChartValidationIssue {
                field: "title".into(),
                message: "Chart title is required".into(),
            });
        }
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
        let zone = match self.timezone {
            ChartTimezone::UniversalTime => TimeZoneAssertion::UniversalTime,
            ChartTimezone::FixedOffset(offset) => TimeZoneAssertion::FixedOffset(offset),
        };
        Ok(ChartDraft {
            title: self.title.trim().into(),
            record: ChartRecord {
                event_kind: self.event_kind.clone(),
                subject: self.subject_name.as_ref().and_then(|name| {
                    (!name.trim().is_empty()).then(|| SubjectInfo {
                        display_name: name.trim().into(),
                        pronouns: None,
                    })
                }),
                time: TemporalAssertion {
                    civil_datetime: CivilDateTime {
                        date: self.civil_date,
                        time: self.civil_time,
                    },
                    calendar: CalendarSpec::ProlepticGregorian,
                    zone,
                    disambiguation: None,
                },
                location,
                source: SourceProvenance {
                    description: "Entered in Mirabile Workbench".into(),
                    source_type: SourceType::UserAssertion,
                    recorded_by: None,
                },
                notes: Vec::new(),
                life_events: Vec::new(),
            },
            calculation: self.calculation.clone(),
        })
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
}
