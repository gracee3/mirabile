use std::str::FromStr;

use astra_core::{
    AnalysisProfile, Angle, AspectClass, AspectDefinition, AspectFieldSpec, AspectId, AspectSet,
    CalculationSpec, CalendarSpec, ChartDefinition, ChartRecord, ChartSlotId, ChartSource,
    CivilDate, CivilDateTime, CivilTime, EventKind, HouseDisplaySpec, LabelSpec, Latitude,
    LocationAssertion, Longitude, OrbPolicy, PointId, PointRole, PointSelector, PointSet,
    ResourceEnvelope, ResourceId, RingGeometry, RingSpec, SourceProvenance, SourceType,
    SubjectInfo, TemporalAssertion, Theme, TimeZoneAssertion, Timestamp, WheelTemplate,
    ZodiacDisplaySpec,
};

const RECORD_ID: &str = "b0388048-c627-4fe3-a37f-93445bb6828a";
const DEFINITION_ID: &str = "687d75b4-c8e1-4775-912b-1f38046fbd6f";
const ASPECT_SET_ID: &str = "3e552f02-b997-48d2-9677-b7e6c217bef4";

pub fn record() -> ResourceEnvelope<ChartRecord> {
    ResourceEnvelope::with_id(
        ResourceId::from_str(RECORD_ID).expect("fixture ID is valid"),
        "Architecture demonstration",
        ChartRecord {
            event_kind: EventKind::Birth,
            subject: Some(SubjectInfo {
                display_name: "Astra example".into(),
                pronouns: None,
            }),
            time: TemporalAssertion {
                civil_datetime: CivilDateTime {
                    date: CivilDate::new(2000, 1, 1).expect("fixture date is valid"),
                    time: CivilTime::new(12, 0, 0).expect("fixture time is valid"),
                },
                calendar: CalendarSpec::ProlepticGregorian,
                zone: TimeZoneAssertion::UniversalTime,
                disambiguation: None,
            },
            location: LocationAssertion {
                display_name: "Greenwich".into(),
                country_region: Some("GB".into()),
                latitude: Latitude::from_degrees(51.48).expect("fixture latitude is valid"),
                longitude: Longitude::from_degrees(0.0).expect("fixture longitude is valid"),
                atlas_provenance: None,
            },
            source: SourceProvenance {
                description: "Deterministic architecture fixture".into(),
                source_type: SourceType::UserAssertion,
                recorded_by: None,
            },
            notes: Vec::new(),
            life_events: Vec::new(),
        },
        Timestamp::from_unix_millis(0),
    )
}

pub fn definition(record: ResourceId) -> ResourceEnvelope<ChartDefinition> {
    ResourceEnvelope::with_id(
        ResourceId::from_str(DEFINITION_ID).expect("fixture ID is valid"),
        "Natal definition",
        ChartDefinition {
            source: ChartSource::Radix { record },
            calculation: CalculationSpec::default(),
        },
        Timestamp::from_unix_millis(0),
    )
}

pub fn points() -> PointSet {
    PointSet {
        points: ["sun", "moon", "mercury", "venus", "mars", "jupiter"]
            .into_iter()
            .map(|id| PointSelector::Point(PointId::new(id).expect("fixture point ID is valid")))
            .collect(),
    }
}

pub fn aspect_resource() -> ResourceEnvelope<AspectSet> {
    ResourceEnvelope::with_id(
        ResourceId::from_str(ASPECT_SET_ID).expect("fixture ID is valid"),
        "Demo aspects",
        AspectSet {
            aspects: vec![
                AspectDefinition {
                    id: AspectId::new("conjunction").expect("fixture aspect ID is valid"),
                    name: "Conjunction".into(),
                    angle: Angle::from_degrees(0.0).expect("fixture angle is valid"),
                    enabled: true,
                    orbs: OrbPolicy {
                        maximum: Angle::from_degrees(8.0).expect("fixture orb is valid"),
                        applying_multiplier: 1.0,
                    },
                    classification: AspectClass::Major,
                },
                AspectDefinition {
                    id: AspectId::new("sextile").expect("fixture aspect ID is valid"),
                    name: "Sextile".into(),
                    angle: Angle::from_degrees(60.0).expect("fixture angle is valid"),
                    enabled: true,
                    orbs: OrbPolicy {
                        maximum: Angle::from_degrees(5.0).expect("fixture orb is valid"),
                        applying_multiplier: 1.0,
                    },
                    classification: AspectClass::Major,
                },
                AspectDefinition {
                    id: AspectId::new("square").expect("fixture aspect ID is valid"),
                    name: "Square".into(),
                    angle: Angle::from_degrees(90.0).expect("fixture angle is valid"),
                    enabled: true,
                    orbs: OrbPolicy {
                        maximum: Angle::from_degrees(5.0).expect("fixture orb is valid"),
                        applying_multiplier: 1.0,
                    },
                    classification: AspectClass::Major,
                },
                AspectDefinition {
                    id: AspectId::new("trine").expect("fixture aspect ID is valid"),
                    name: "Trine".into(),
                    angle: Angle::from_degrees(120.0).expect("fixture angle is valid"),
                    enabled: true,
                    orbs: OrbPolicy {
                        maximum: Angle::from_degrees(6.0).expect("fixture orb is valid"),
                        applying_multiplier: 1.0,
                    },
                    classification: AspectClass::Major,
                },
                AspectDefinition {
                    id: AspectId::new("opposition").expect("fixture aspect ID is valid"),
                    name: "Opposition".into(),
                    angle: Angle::from_degrees(180.0).expect("fixture angle is valid"),
                    enabled: true,
                    orbs: OrbPolicy {
                        maximum: Angle::from_degrees(7.0).expect("fixture orb is valid"),
                        applying_multiplier: 1.0,
                    },
                    classification: AspectClass::Major,
                },
            ],
        },
        Timestamp::from_unix_millis(0),
    )
}

pub fn analysis_profile() -> AnalysisProfile {
    AnalysisProfile::default()
}

pub fn wheel() -> WheelTemplate {
    WheelTemplate {
        rings: vec![RingSpec {
            chart_slot: ChartSlotId::new("radix").expect("fixture slot ID is valid"),
            point_role: PointRole::Primary,
            geometry: RingGeometry {
                inner_radius: 125.0,
                outer_radius: 150.0,
            },
        }],
        aspect_field: AspectFieldSpec { radius: 105.0 },
        houses: HouseDisplaySpec {
            show_cusps: true,
            show_numbers: true,
        },
        zodiac: ZodiacDisplaySpec {
            show_boundaries: true,
            show_labels: true,
        },
        labels: LabelSpec {
            show_degrees: true,
            show_retrograde: true,
        },
    }
}

pub fn dark_theme() -> Theme {
    Theme {
        background: "#111827".into(),
        foreground: "#f8fafc".into(),
        muted: "#64748b".into(),
        accent: "#67e8f9".into(),
        aspect_color: "#f472b6".into(),
    }
}

pub fn light_theme() -> Theme {
    Theme {
        background: "#f8fafc".into(),
        foreground: "#172033".into(),
        muted: "#94a3b8".into(),
        accent: "#2563eb".into(),
        aspect_color: "#be185d".into(),
    }
}
