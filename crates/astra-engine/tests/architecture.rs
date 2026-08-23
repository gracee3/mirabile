use astra_core::{
    AnalysisProfile, Angle, AspectClass, AspectDefinition, AspectFieldSpec, AspectId, AspectSet,
    CalculationSpec, CalendarSpec, ChartDefinition, ChartRecord, ChartSlotId, ChartSource,
    CivilDate, CivilDateTime, CivilTime, EventKind, HouseDisplaySpec, HouseSystem, LabelSpec,
    Latitude, LocationAssertion, Longitude, OrbPolicy, PointId, PointSelector, PointSet,
    ResourceEnvelope, ResourceId, RingGeometry, RingSpec, SourceProvenance, SourceType,
    TemporalAssertion, Theme, TimeZoneAssertion, Timestamp, WheelTemplate, ZodiacDisplaySpec,
};
use astra_engine::{
    AspectAnalyzer, CalculationEngine, ComputationCache, DeterministicEphemeris, Scene,
    layout_wheel, render_key,
};

fn sample_resources() -> (
    ResourceEnvelope<ChartRecord>,
    ResourceEnvelope<ChartDefinition>,
) {
    let record_id = ResourceId::new();
    let record = ResourceEnvelope::with_id(
        record_id,
        "Example person",
        ChartRecord {
            event_kind: EventKind::Birth,
            subject: None,
            time: TemporalAssertion {
                civil_datetime: CivilDateTime {
                    date: CivilDate::new(2000, 1, 1).expect("valid date"),
                    time: CivilTime::new(12, 0, 0).expect("valid time"),
                },
                calendar: CalendarSpec::ProlepticGregorian,
                zone: TimeZoneAssertion::UniversalTime,
                disambiguation: None,
            },
            location: LocationAssertion {
                display_name: "Greenwich".into(),
                country_region: Some("GB".into()),
                latitude: Latitude::from_degrees(51.48).expect("valid latitude"),
                longitude: Longitude::from_degrees(0.0).expect("valid longitude"),
                atlas_provenance: None,
            },
            source: SourceProvenance {
                description: "Architecture fixture".into(),
                source_type: SourceType::UserAssertion,
                recorded_by: None,
            },
            notes: Vec::new(),
            life_events: Vec::new(),
        },
        Timestamp::from_unix_millis(0),
    );
    let definition = ResourceEnvelope::new(
        "Natal definition",
        ChartDefinition {
            source: ChartSource::Radix { record: record_id },
            calculation: CalculationSpec::default(),
        },
        Timestamp::from_unix_millis(0),
    );
    (record, definition)
}

fn points() -> PointSet {
    PointSet {
        points: ["sun", "moon", "mercury", "venus", "mars", "jupiter"]
            .into_iter()
            .map(|id| PointSelector::Point(PointId::new(id).expect("valid point ID")))
            .collect(),
    }
}

fn aspects(conjunction_orb: f64) -> AspectSet {
    AspectSet {
        aspects: vec![
            AspectDefinition {
                id: AspectId::new("conjunction").expect("valid aspect ID"),
                name: "Conjunction".into(),
                angle: Angle::from_degrees(0.0).expect("valid angle"),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: Angle::from_degrees(conjunction_orb).expect("valid angle"),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            },
            AspectDefinition {
                id: AspectId::new("square").expect("valid aspect ID"),
                name: "Square".into(),
                angle: Angle::from_degrees(90.0).expect("valid angle"),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: Angle::from_degrees(5.0).expect("valid angle"),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            },
        ],
    }
}

fn wheel() -> WheelTemplate {
    WheelTemplate {
        rings: vec![RingSpec {
            chart_slot: ChartSlotId::new("radix").expect("valid slot"),
            point_role: astra_core::PointRole::Primary,
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

fn theme(accent: &str) -> Theme {
    Theme {
        background: "#ffffff".into(),
        foreground: "#111111".into(),
        muted: "#888888".into(),
        accent: accent.into(),
        aspect_color: "#b04050".into(),
    }
}

#[test]
fn birth_time_changes_calculation_key() {
    let (record, definition) = sample_resources();
    let mut edited = record.clone();
    edited.payload.time.civil_datetime.time = CivilTime::new(12, 1, 0).expect("valid time");
    let engine = CalculationEngine::new(DeterministicEphemeris, "engine-v1", "fixture-tz-v1");

    assert_ne!(
        engine.calc_key(&definition, &record).expect("first key"),
        engine.calc_key(&definition, &edited).expect("edited key")
    );
}

#[test]
fn metadata_only_rename_does_not_invalidate_calculation() {
    let (record, definition) = sample_resources();
    let mut renamed = record.clone();
    renamed.title = "Renamed example person".into();
    let engine = CalculationEngine::new(DeterministicEphemeris, "engine-v1", "fixture-tz-v1");

    assert_eq!(
        engine.calc_key(&definition, &record).expect("first key"),
        engine.calc_key(&definition, &renamed).expect("renamed key")
    );
}

#[test]
fn aspect_change_reuses_snapshot_and_changes_only_downstream_keys() {
    let (record, definition) = sample_resources();
    let engine = CalculationEngine::new(DeterministicEphemeris, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let broad = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("broad analysis");
    let narrow = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(6.0),
        &AnalysisProfile::default(),
    )
    .expect("narrow analysis");

    assert_eq!(broad.snapshot_key, narrow.snapshot_key);
    assert_ne!(broad.analysis_key, narrow.analysis_key);
    assert_ne!(broad.aspects, narrow.aspects);
}

#[test]
fn theme_changes_render_key_but_not_calculation_analysis_or_layout() {
    let (record, definition) = sample_resources();
    let engine = CalculationEngine::new(DeterministicEphemeris, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("analysis");
    let layout = layout_wheel(&snapshot, &analysis, &points(), &wheel(), None).expect("layout");
    let calc_key = snapshot.calc_key.clone();
    let analysis_key = analysis.analysis_key.clone();
    let layout_key = layout.key.clone();
    let first = render_key(&layout, &theme("#3454d1")).expect("first render key");
    let second = render_key(&layout, &theme("#e76f51")).expect("second render key");

    assert_eq!(analysis.snapshot_key, calc_key);
    assert_eq!(analysis.analysis_key, analysis_key);
    assert_eq!(layout.key, layout_key);
    assert_ne!(first, second);
}

#[test]
fn defaults_do_not_rewrite_existing_chart_definition() {
    let (_, definition) = sample_resources();
    let existing = definition.payload.clone();
    let changed_defaults = CalculationSpec {
        houses: HouseSystem::WholeSign,
        ..CalculationSpec::default()
    };

    assert_eq!(existing.calculation.houses, HouseSystem::Placidus);
    assert_eq!(changed_defaults.houses, HouseSystem::WholeSign);
}

#[test]
fn disposable_cache_can_be_reconstructed_from_canonical_inputs() {
    let (record, definition) = sample_resources();
    let engine = CalculationEngine::new(DeterministicEphemeris, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("analysis");
    let mut cache = ComputationCache::default();
    cache.insert_snapshot(snapshot.clone());
    cache.insert_analysis(analysis.clone());
    cache.clear();
    assert!(cache.is_empty());

    let rebuilt_snapshot = engine
        .calculate(&definition, &record)
        .expect("rebuilt snapshot");
    let rebuilt_analysis = AspectAnalyzer::analyze(
        &rebuilt_snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("rebuilt analysis");
    assert_eq!(snapshot.calc_key, rebuilt_snapshot.calc_key);
    assert_eq!(analysis.analysis_key, rebuilt_analysis.analysis_key);
}

#[test]
fn layout_produces_astrology_free_scene_primitives() {
    let (record, definition) = sample_resources();
    let engine = CalculationEngine::new(DeterministicEphemeris, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("analysis");
    let layout = layout_wheel(&snapshot, &analysis, &points(), &wheel(), None).expect("layout");
    let scene = Scene::from_wheel(&layout);

    assert!(!scene.circles.is_empty());
    assert_eq!(scene.labels.len(), layout.points.len());
}
