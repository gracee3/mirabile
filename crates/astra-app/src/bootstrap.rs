use std::{collections::BTreeMap, str::FromStr};

use astra_core::{
    AnalysisProfile, Angle, AspectClass, AspectDefinition, AspectFieldSpec, AspectId, AspectSet,
    CalculationSpec, CalendarSpec, CanonicalResource, ChartDefinition, ChartRecord, ChartSlot,
    ChartSlotId, ChartSource, CivilDate, CivilDateTime, CivilTime, EventKind, HouseDisplaySpec,
    LabelSpec, Latitude, LocationAssertion, Longitude, ObjectFrame, Offset, OrbPolicy, PageLayout,
    PointId, PointRole, PointSelector, PointSet, ResourceBinding, ResourceEnvelope, ResourceId,
    RingGeometry, RingSpec, SourceProvenance, SourceType, TemporalAssertion, Theme,
    TimeZoneAssertion, Timestamp, ViewDocument, ViewInstance, ViewInstanceId, ViewObject,
    ViewOverrides, WheelObject, WheelTemplate, Workspace, WorkspaceChart, WorkspaceProfile,
    ZodiacDisplaySpec,
};

const CHART_RECORD_A: &str = "11000000-0000-4000-8000-000000000001";
const CHART_RECORD_B: &str = "11000000-0000-4000-8000-000000000002";
const CHART_DEFINITION_A: &str = "12000000-0000-4000-8000-000000000001";
const CHART_DEFINITION_B: &str = "12000000-0000-4000-8000-000000000002";
const ASPECT_SET_STANDARD: &str = "13000000-0000-4000-8000-000000000001";
const ASPECT_SET_TIGHT: &str = "13000000-0000-4000-8000-000000000002";
const WORKSPACE: &str = "14000000-0000-4000-8000-000000000001";
const CHART_INSTANCE_A: &str = "15000000-0000-4000-8000-000000000001";
const VIEW: &str = "16000000-0000-4000-8000-000000000001";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapIds {
    pub chart_record_a: ResourceId,
    pub chart_record_b: ResourceId,
    pub chart_definition_a: ResourceId,
    pub chart_definition_b: ResourceId,
    pub aspect_set_standard: ResourceId,
    pub aspect_set_tight: ResourceId,
    pub workspace: ResourceId,
    pub chart_instance_a: astra_core::InstanceId,
    pub view: ViewInstanceId,
}

/// Returns the stable identities reserved for the deterministic first-run bootstrap.
///
/// # Panics
///
/// Panics only if a checked-in bootstrap UUID literal is invalid.
pub fn bootstrap_ids() -> BootstrapIds {
    BootstrapIds {
        chart_record_a: resource_id(CHART_RECORD_A),
        chart_record_b: resource_id(CHART_RECORD_B),
        chart_definition_a: resource_id(CHART_DEFINITION_A),
        chart_definition_b: resource_id(CHART_DEFINITION_B),
        aspect_set_standard: resource_id(ASPECT_SET_STANDARD),
        aspect_set_tight: resource_id(ASPECT_SET_TIGHT),
        workspace: resource_id(WORKSPACE),
        chart_instance_a: CHART_INSTANCE_A
            .parse()
            .expect("bootstrap chart instance ID is valid"),
        view: VIEW.parse().expect("bootstrap view ID is valid"),
    }
}

pub(crate) fn bootstrap_resources() -> Vec<CanonicalResource> {
    let ids = bootstrap_ids();
    let now = Timestamp::from_unix_millis(1);
    let record_a = chart_record(
        ids.chart_record_a,
        "Example Natal Record",
        EventKind::Birth,
        CivilDate::new(2000, 1, 1).expect("bootstrap date is valid"),
        CivilTime::new(12, 0, 0).expect("bootstrap time is valid"),
        TimeZoneAssertion::UniversalTime,
        "Greenwich",
        "GB",
        51.48,
        0.0,
        now,
    );
    let record_b = chart_record(
        ids.chart_record_b,
        "Example Event Record",
        EventKind::Event,
        CivilDate::new(1985, 7, 4).expect("bootstrap date is valid"),
        CivilTime::new(9, 30, 0).expect("bootstrap time is valid"),
        TimeZoneAssertion::FixedOffset(
            Offset::from_seconds(-14_400).expect("bootstrap offset is valid"),
        ),
        "New York",
        "US",
        40.7128,
        -74.006,
        now,
    );
    let definition_a = ResourceEnvelope::with_id(
        ids.chart_definition_a,
        "Example Natal",
        ChartDefinition {
            source: ChartSource::Radix {
                record: ids.chart_record_a,
            },
            calculation: CalculationSpec::default(),
        },
        now,
    );
    let definition_b = ResourceEnvelope::with_id(
        ids.chart_definition_b,
        "Example Event",
        ChartDefinition {
            source: ChartSource::Radix {
                record: ids.chart_record_b,
            },
            calculation: CalculationSpec::default(),
        },
        now,
    );
    let standard = ResourceEnvelope::with_id(
        ids.aspect_set_standard,
        "Standard",
        aspect_set(8.0, 6.0),
        now,
    );
    let tight = ResourceEnvelope::with_id(ids.aspect_set_tight, "Tight", aspect_set(4.0, 3.0), now);
    let workspace =
        ResourceEnvelope::with_id(ids.workspace, "Astra Workspace", workspace(ids), now);

    vec![
        CanonicalResource::ChartRecord(record_a),
        CanonicalResource::ChartDefinition(definition_a),
        CanonicalResource::ChartRecord(record_b),
        CanonicalResource::ChartDefinition(definition_b),
        CanonicalResource::AspectSet(standard),
        CanonicalResource::AspectSet(tight),
        CanonicalResource::Workspace(workspace),
    ]
}

#[allow(clippy::too_many_arguments)]
fn chart_record(
    id: ResourceId,
    title: &str,
    event_kind: EventKind,
    date: CivilDate,
    time: CivilTime,
    zone: TimeZoneAssertion,
    location: &str,
    country_region: &str,
    latitude: f64,
    longitude: f64,
    now: Timestamp,
) -> ResourceEnvelope<ChartRecord> {
    ResourceEnvelope::with_id(
        id,
        title,
        ChartRecord {
            event_kind,
            subject: None,
            time: TemporalAssertion {
                civil_datetime: CivilDateTime { date, time },
                calendar: CalendarSpec::ProlepticGregorian,
                zone,
                disambiguation: None,
            },
            location: LocationAssertion {
                display_name: location.into(),
                country_region: Some(country_region.into()),
                latitude: Latitude::from_degrees(latitude).expect("bootstrap latitude is valid"),
                longitude: Longitude::from_degrees(longitude)
                    .expect("bootstrap longitude is valid"),
                atlas_provenance: None,
            },
            source: SourceProvenance {
                description: "Deterministic Astra bootstrap fixture".into(),
                source_type: SourceType::UserAssertion,
                recorded_by: None,
            },
            notes: Vec::new(),
            life_events: Vec::new(),
        },
        now,
    )
}

fn aspect_set(conjunction_orb: f64, square_orb: f64) -> AspectSet {
    AspectSet {
        aspects: vec![
            AspectDefinition {
                id: AspectId::new("conjunction").expect("bootstrap aspect ID is valid"),
                name: "Conjunction".into(),
                angle: angle(0.0),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: angle(conjunction_orb),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            },
            AspectDefinition {
                id: AspectId::new("square").expect("bootstrap aspect ID is valid"),
                name: "Square".into(),
                angle: angle(90.0),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: angle(square_orb),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            },
        ],
    }
}

fn workspace(ids: BootstrapIds) -> Workspace {
    let radix = ChartSlotId::new("radix").expect("bootstrap slot ID is valid");
    let comparison = ChartSlotId::new("comparison").expect("bootstrap slot ID is valid");
    let points = point_set();
    let document = ViewDocument {
        chart_slots: vec![
            ChartSlot {
                id: radix.clone(),
                label: "Radix".into(),
                required: true,
            },
            ChartSlot {
                id: comparison,
                label: "Comparison".into(),
                required: false,
            },
        ],
        objects: vec![ViewObject::Wheel(WheelObject {
            slot: radix.clone(),
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
    let view = ViewInstance {
        id: ids.view,
        document: ResourceBinding::Inline { value: document },
        charts: BTreeMap::from([(radix.clone(), ids.chart_instance_a)]),
        overrides: ViewOverrides::default(),
    };

    Workspace {
        chart_instances: vec![WorkspaceChart::Saved {
            instance_id: ids.chart_instance_a,
            definition: ids.chart_definition_a,
        }],
        active_chart: Some(ids.chart_instance_a),
        selected_charts: Vec::new(),
        views: vec![view],
        active_view: Some(ids.view),
        profile: WorkspaceProfile {
            displayed_points: ResourceBinding::Inline {
                value: points.clone(),
            },
            aspected_points: ResourceBinding::Inline {
                value: points.clone(),
            },
            transit_points: ResourceBinding::Inline { value: points },
            aspects: ResourceBinding::Follow {
                id: ids.aspect_set_standard,
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
                        chart_slot: radix,
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
                },
            },
        },
    }
}

fn point_set() -> PointSet {
    PointSet {
        points: ["sun", "moon", "mercury", "venus", "mars", "jupiter"]
            .into_iter()
            .map(|value| {
                PointSelector::Point(PointId::new(value).expect("bootstrap point ID is valid"))
            })
            .collect(),
    }
}

fn angle(value: f64) -> Angle {
    Angle::from_degrees(value).expect("bootstrap angle is valid")
}

fn resource_id(value: &str) -> ResourceId {
    ResourceId::from_str(value).expect("bootstrap resource ID is valid")
}

#[cfg(test)]
mod tests {
    use astra_core::DomainValidate;

    use super::*;

    #[test]
    fn bootstrap_is_small_deterministic_and_domain_valid() {
        let first = bootstrap_resources();
        let second = bootstrap_resources();

        assert_eq!(first, second);
        assert_eq!(first.len(), 7);
        for resource in &first {
            resource.validate().expect("bootstrap resource validates");
        }
        let CanonicalResource::Workspace(workspace) = &first[6] else {
            panic!("last bootstrap resource is the workspace");
        };
        workspace
            .payload
            .domain_validate()
            .expect("bootstrap workspace validates");
    }
}
