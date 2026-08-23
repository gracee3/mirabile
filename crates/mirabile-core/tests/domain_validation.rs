use mirabile_core::{
    AnalysisProfile, Angle, AspectClass, AspectDefinition, AspectFieldSpec, AspectId, AspectSet,
    CalendarSpec, ChartRecord, ChartSlot, ChartSlotId, CivilDate, CivilDateTime, CivilTime,
    CompositeMethod, DerivationSpec, DomainValidate, EventKind, HouseDisplaySpec, InstanceId,
    LabelSpec, Latitude, LifeEvent, LocationAssertion, Longitude, ObjectFrame, OrbPolicy,
    PointSelector, PointSet, QueryDefinition, QueryExpr, ResourceBinding, ResourceEnvelope,
    ResourceId, RingGeometry, RingSpec, SourceProvenance, SourceType, TemporalAssertion, Theme,
    TimeZoneAssertion, Timestamp, ViewDocument, ViewObject, WheelObject, WheelTemplate,
    WorkspaceDocument, WorkspaceDocumentChart, WorkspaceProfile, ZodiacDisplaySpec,
};

fn assertion(year: i32, calendar: CalendarSpec) -> TemporalAssertion {
    TemporalAssertion {
        civil_datetime: CivilDateTime {
            date: CivilDate::new(year, 2, 29).expect("structurally valid leap day"),
            time: CivilTime::new(12, 0, 0).expect("valid time"),
        },
        calendar,
        zone: TimeZoneAssertion::UniversalTime,
        disambiguation: None,
    }
}

fn point_set() -> PointSet {
    PointSet { points: Vec::new() }
}

fn aspect_set() -> AspectSet {
    AspectSet {
        aspects: Vec::new(),
    }
}

fn wheel() -> WheelTemplate {
    WheelTemplate {
        rings: vec![RingSpec {
            chart_slot: ChartSlotId::new("radix").expect("valid ID"),
            point_role: mirabile_core::PointRole::Primary,
            geometry: RingGeometry {
                inner_radius: 100.0,
                outer_radius: 150.0,
            },
        }],
        aspect_field: AspectFieldSpec { radius: 80.0 },
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

fn profile() -> WorkspaceProfile {
    let points = || ResourceBinding::Inline { value: point_set() };
    WorkspaceProfile {
        displayed_points: points(),
        aspected_points: points(),
        transit_points: points(),
        aspects: ResourceBinding::Inline {
            value: aspect_set(),
        },
        analysis: ResourceBinding::Inline {
            value: AnalysisProfile::default(),
        },
        theme: ResourceBinding::Inline {
            value: Theme {
                background: "white".into(),
                foreground: "black".into(),
                muted: "gray".into(),
                accent: "blue".into(),
                aspect_color: "red".into(),
            },
        },
        wheel: ResourceBinding::Inline { value: wheel() },
    }
}

#[test]
fn calendar_validation_reaches_nested_life_events() {
    let record = ChartRecord {
        event_kind: EventKind::Birth,
        subject: None,
        time: assertion(2000, CalendarSpec::ProlepticGregorian),
        location: Some(LocationAssertion {
            display_name: "Greenwich".into(),
            country_region: None,
            latitude: Latitude::from_degrees(51.48).expect("valid latitude"),
            longitude: Longitude::from_degrees(0.0).expect("valid longitude"),
            atlas_provenance: None,
        }),
        source: SourceProvenance {
            description: "fixture".into(),
            source_type: SourceType::UserAssertion,
            recorded_by: None,
        },
        notes: Vec::new(),
        life_events: vec![LifeEvent {
            title: "Invalid Gregorian leap day".into(),
            time: assertion(1900, CalendarSpec::ProlepticGregorian),
            location: None,
            notes: Vec::new(),
        }],
    };

    let error = record.domain_validate().expect_err("nested invalid date");
    assert_eq!(error.path, "life_events[0].time.civil_datetime.date");
}

#[test]
fn derivations_require_positive_harmonics_and_unique_composite_inputs() {
    let harmonic = DerivationSpec::Harmonic {
        radix: ResourceId::new(),
        harmonic: 0.0,
    };
    assert!(harmonic.domain_validate().is_err());

    let id = ResourceId::new();
    let composite = DerivationSpec::Composite {
        charts: vec![id, id],
        method: CompositeMethod::Midpoint,
    };
    assert!(composite.domain_validate().is_err());
}

#[test]
fn point_aspect_and_wheel_rules_reject_ambiguous_or_invalid_values() {
    let point = mirabile_core::PointId::new("sun").expect("valid ID");
    let duplicate_points = PointSet {
        points: vec![
            PointSelector::Point(point.clone()),
            PointSelector::Point(point),
        ],
    };
    assert!(duplicate_points.domain_validate().is_err());

    let aspect_id = AspectId::new("square").expect("valid ID");
    let duplicate_aspects = AspectSet {
        aspects: ["Square", "Renamed"]
            .into_iter()
            .map(|name| AspectDefinition {
                id: aspect_id.clone(),
                name: name.into(),
                angle: Angle::from_degrees(90.0).expect("valid number"),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: Angle::from_degrees(5.0).expect("valid number"),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            })
            .collect(),
    };
    assert!(duplicate_aspects.domain_validate().is_err());

    let mut invalid_wheel = wheel();
    invalid_wheel.rings[0].geometry.inner_radius = 160.0;
    assert!(invalid_wheel.domain_validate().is_err());
}

#[test]
fn views_queries_and_workspaces_validate_references_and_structure() {
    let radix = ChartSlotId::new("radix").expect("valid ID");
    let missing = ChartSlotId::new("missing").expect("valid ID");
    let view = ViewDocument {
        chart_slots: vec![ChartSlot {
            id: radix,
            label: "Radix".into(),
            required: true,
        }],
        objects: vec![ViewObject::Wheel(WheelObject {
            slot: missing,
            frame: ObjectFrame {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
        })],
        layout: mirabile_core::PageLayout {
            width: 800.0,
            height: 600.0,
        },
    };
    assert!(view.domain_validate().is_err());

    let query = QueryDefinition {
        expression: QueryExpr::And(Vec::new()),
        description: None,
    };
    assert!(query.domain_validate().is_err());

    let instance_id = InstanceId::new();
    let workspace = WorkspaceDocument {
        chart_instances: vec![
            WorkspaceDocumentChart {
                instance_id,
                definition: ResourceId::new(),
            },
            WorkspaceDocumentChart {
                instance_id,
                definition: ResourceId::new(),
            },
        ],
        views: Vec::new(),
        profile: profile(),
    };
    assert!(workspace.domain_validate().is_err());
}

#[test]
fn resource_metadata_requires_unique_tags_and_monotonic_timestamps() {
    let mut envelope =
        ResourceEnvelope::new("Points", point_set(), Timestamp::from_unix_millis(10));
    envelope.tags = vec!["core".into(), "core".into()];
    assert!(envelope.validate().is_err());

    envelope.tags = vec!["core".into()];
    envelope.modified_at = Timestamp::from_unix_millis(9);
    assert!(envelope.validate().is_err());
}
