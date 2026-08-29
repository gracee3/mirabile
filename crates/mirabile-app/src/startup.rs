use std::collections::BTreeMap;

use mirabile_core::{
    AnalysisProfile, AspectFieldSpec, AspectSet, CalculationSpec, CalendarSpec, ChartRecord,
    ChartSlot, ChartSlotId, CivilDate, CivilDateTime, CivilTime, CoordinateSystem, CorrectionSpec,
    EventKind, HouseDisplaySpec, HouseSystem, InstanceId, LabelSpec, ObjectFrame, PageLayout,
    PointId, PointRole, PointSelector, PointSet, ResourceBinding, ResourceId, RingGeometry,
    RingSpec, SourceProvenance, SourceType, TemporalAssertion, Theme, TimeZoneAssertion,
    ViewDocument, ViewInstance, ViewInstanceId, ViewObject, ViewOverrides, WheelObject,
    WheelTemplate, WorkspaceDocument, WorkspaceProfile, ZodiacDisplaySpec, ZodiacSpec,
};

use crate::{ChartDraft, WorkspaceSession, WorkspaceSessionDraftChart};

/// Application-level startup behavior. Session recovery is intentionally a future concern.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum StartupPolicy {
    #[default]
    RestorePreviousSession,
    CurrentTransits,
    BlankWorkspace,
    OpenWorkspace(ResourceId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StartupCalculationProfile {
    Baseline,
    #[cfg(feature = "xalen-backend")]
    ApparentPlace,
}

impl StartupCalculationProfile {
    fn corrections(self) -> CorrectionSpec {
        match self {
            Self::Baseline => CorrectionSpec::default(),
            #[cfg(feature = "xalen-backend")]
            Self::ApparentPlace => CorrectionSpec {
                aberration: true,
                light_time: true,
                nutation: true,
            },
        }
    }
}

pub(crate) fn current_unix_millis() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        #[allow(clippy::cast_possible_truncation)]
        return js_sys::Date::now().round() as i64;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};

        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
    }
}

pub(crate) fn blank_workspace_session() -> WorkspaceSession {
    WorkspaceSession::unsaved(WorkspaceDocument {
        chart_instances: Vec::new(),
        views: Vec::new(),
        profile: session_profile(ChartSlotId::new("primary").expect("built-in slot ID is valid")),
    })
}

pub(crate) fn current_transits_session(
    unix_millis: i64,
    profile: StartupCalculationProfile,
) -> WorkspaceSession {
    let chart_instance = InstanceId::new();
    let view_id = ViewInstanceId::new();
    let slot = ChartSlotId::new("primary").expect("built-in slot ID is valid");
    let draft = ChartDraft {
        title: "Current Transits".into(),
        definition_description: None,
        definition_tags: Vec::new(),
        record_title: "Current Transits source".into(),
        record_description: None,
        record_tags: Vec::new(),
        record: ChartRecord {
            event_kind: EventKind::Event,
            subject: None,
            time: TemporalAssertion {
                civil_datetime: utc_civil_datetime(unix_millis),
                calendar: CalendarSpec::ProlepticGregorian,
                zone: TimeZoneAssertion::UniversalTime,
                disambiguation: None,
            },
            location: None,
            source: SourceProvenance {
                description: "Current browser/system clock".into(),
                source_type: SourceType::SystemClock,
                recorded_by: None,
            },
            notes: Vec::new(),
            life_events: Vec::new(),
        },
        calculation: CalculationSpec {
            zodiac: ZodiacSpec::Tropical,
            houses: HouseSystem::NoHouses,
            coordinates: CoordinateSystem::Geocentric,
            corrections: profile.corrections(),
            ..CalculationSpec::default()
        },
    };
    let view_document = ViewDocument {
        chart_slots: vec![ChartSlot {
            id: slot.clone(),
            label: "Current Transits".into(),
            required: true,
        }],
        objects: vec![ViewObject::Wheel(WheelObject {
            slot: slot.clone(),
            frame: ObjectFrame {
                x: 0.0,
                y: 0.0,
                width: 400.0,
                height: 400.0,
            },
        })],
        layout: PageLayout {
            width: 400.0,
            height: 400.0,
        },
    };
    let document = WorkspaceDocument {
        chart_instances: Vec::new(),
        views: vec![ViewInstance {
            id: view_id,
            document: ResourceBinding::Inline {
                value: view_document,
            },
            charts: BTreeMap::new(),
            overrides: ViewOverrides::default(),
        }],
        profile: session_profile(slot.clone()),
    };
    let mut session = WorkspaceSession::unsaved(document);
    session.draft_charts.push(WorkspaceSessionDraftChart {
        instance_id: chart_instance,
        draft,
    });
    session
        .draft_chart_assignments
        .entry(view_id)
        .or_default()
        .insert(slot, chart_instance);
    session.active_chart = Some(chart_instance);
    session.active_view = Some(view_id);
    session
}

fn session_profile(slot: ChartSlotId) -> WorkspaceProfile {
    let points = supported_point_set();
    WorkspaceProfile {
        displayed_points: ResourceBinding::Inline {
            value: points.clone(),
        },
        aspected_points: ResourceBinding::Inline {
            value: points.clone(),
        },
        transit_points: ResourceBinding::Inline { value: points },
        aspects: ResourceBinding::Inline {
            value: AspectSet {
                aspects: Vec::new(),
            },
        },
        analysis: ResourceBinding::Inline {
            value: AnalysisProfile::default(),
        },
        theme: ResourceBinding::Inline {
            value: Theme {
                background: "#121416".into(),
                foreground: "#f4f1e8".into(),
                muted: "#7f8790".into(),
                accent: "#c79a5b".into(),
                aspect_color: "#a96772".into(),
            },
        },
        wheel: ResourceBinding::Inline {
            value: WheelTemplate {
                rings: vec![RingSpec {
                    chart_slot: slot,
                    point_role: PointRole::Primary,
                    geometry: RingGeometry {
                        inner_radius: 125.0,
                        outer_radius: 150.0,
                    },
                }],
                aspect_field: AspectFieldSpec { radius: 105.0 },
                houses: HouseDisplaySpec {
                    show_cusps: false,
                    show_numbers: false,
                },
                zodiac: ZodiacDisplaySpec {
                    show_boundaries: true,
                    show_labels: true,
                },
                labels: LabelSpec {
                    show_degrees: true,
                    show_retrograde: true,
                },
            },
        },
    }
}

fn supported_point_set() -> PointSet {
    PointSet {
        points: ["sun", "moon", "mercury", "venus", "mars", "jupiter"]
            .into_iter()
            .map(|point| {
                PointSelector::Point(PointId::new(point).expect("built-in point ID is valid"))
            })
            .collect(),
    }
}

pub(crate) fn utc_civil_datetime(unix_millis: i64) -> CivilDateTime {
    let unix_seconds = unix_millis.div_euclid(1_000);
    let days = unix_seconds.div_euclid(86_400);
    let seconds = unix_seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_unix_days(days);
    CivilDateTime {
        date: CivilDate::new(year, month, day).expect("UTC conversion produces a valid date"),
        time: CivilTime::new(
            u8::try_from(seconds / 3_600).expect("UTC hour fits in u8"),
            u8::try_from((seconds % 3_600) / 60).expect("UTC minute fits in u8"),
            u8::try_from(seconds % 60).expect("UTC second fits in u8"),
        )
        .expect("UTC conversion produces a valid time"),
    }
}

fn civil_from_unix_days(days: i64) -> (i32, u8, u8) {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (
        i32::try_from(year).expect("system clock year fits in i32"),
        u8::try_from(month).expect("UTC month fits in u8"),
        u8::try_from(day).expect("UTC day fits in u8"),
    )
}

#[cfg(test)]
mod tests {
    use mirabile_core::{DomainValidate, HouseSystem, SourceType};

    use super::*;

    #[test]
    fn unix_time_conversion_is_utc_and_handles_negative_instants() {
        assert_eq!(
            utc_civil_datetime(0),
            CivilDateTime {
                date: CivilDate::new(1970, 1, 1).expect("date"),
                time: CivilTime::new(0, 0, 0).expect("time"),
            }
        );
        assert_eq!(
            utc_civil_datetime(-1_000),
            CivilDateTime {
                date: CivilDate::new(1969, 12, 31).expect("date"),
                time: CivilTime::new(23, 59, 59).expect("time"),
            }
        );
    }

    #[test]
    fn current_transits_are_ephemeral_locationless_and_domain_valid() {
        let session =
            current_transits_session(946_728_000_000, StartupCalculationProfile::Baseline);
        assert!(session.document.chart_instances.is_empty());
        assert!(session.document.views[0].charts.is_empty());
        assert!(!session.document_dirty);
        assert_eq!(session.draft_charts.len(), 1);
        assert_eq!(
            session.effective_chart_assignment(
                session.active_view.expect("current view"),
                &ChartSlotId::new("primary").expect("built-in slot ID"),
            ),
            session.active_chart,
        );
        let draft = &session.draft_charts[0].draft;
        assert_eq!(draft.record.source.source_type, SourceType::SystemClock);
        assert_eq!(draft.record.location, None);
        assert_eq!(draft.calculation.houses, HouseSystem::NoHouses);
        draft.record.domain_validate().expect("record validates");
        draft
            .calculation
            .domain_validate()
            .expect("calculation validates");
        session
            .document
            .domain_validate()
            .expect("workspace validates");
    }
}
