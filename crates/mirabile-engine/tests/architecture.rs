use mirabile_core::{
    AnalysisProfile, Angle, AngleState, AspectClass, AspectDefinition, AspectFieldSpec, AspectId,
    AspectSet, CalculationSpec, CalendarSpec, ChartDefinition, ChartRecord, ChartSlotId,
    ChartSource, CivilDate, CivilDateTime, CivilTime, EventKind, HouseDisplaySpec, HouseState,
    HouseSystem, LabelSpec, Latitude, LocationAssertion, Longitude, Note, Offset, OrbPolicy,
    PointId, PointSelector, PointSet, ResourceEnvelope, ResourceId, RingGeometry, RingSpec,
    SourceProvenance, SourceType, SubjectInfo, TemporalAssertion, Theme, TimeZoneAssertion,
    Timestamp, WheelTemplate, ZodiacDisplaySpec,
};
use mirabile_engine::{
    AnalysisKey, AspectAnalyzer, AspectHit, AspectVisualStyle, BackendDescriptor, CalcKey,
    CalculationBackend, CalculationBackendError, CalculationBackendResult, CalculationEngine,
    CalculationError, CalculationValue, ChartSnapshot, ComputationCache, DeterministicBackend,
    ImplementationIdentity, KeyError, ResolvedCalculationRequest, Scene, WheelLayoutBounds,
    format_longitude, layout_wheel, layout_wheel_in_bounds, render_key,
};

#[derive(Clone, Copy, Debug)]
struct AlternateIdentityBackend;

impl CalculationBackend for AlternateIdentityBackend {
    fn descriptor(&self) -> BackendDescriptor {
        let mut descriptor = DeterministicBackend.descriptor();
        descriptor.fingerprint.backend.id = "alternate-test-backend".into();
        descriptor.fingerprint.backend.version = "2".into();
        descriptor.fingerprint.backend.revision = Some("alternate-r1".into());
        descriptor
    }

    fn calculate(
        &self,
        request: &ResolvedCalculationRequest,
    ) -> Result<CalculationBackendResult, CalculationBackendError> {
        let mut result = DeterministicBackend.calculate(request)?;
        result.provenance.backend = self.descriptor().fingerprint.backend;
        Ok(result)
    }
}

struct TestEngine<B> {
    engine: CalculationEngine,
    backend: B,
}

impl<B: CalculationBackend> TestEngine<B> {
    fn new(backend: B, engine_version: &str, timezone_data_version: &str) -> Self {
        let descriptor = backend.descriptor();
        Self {
            engine: CalculationEngine::new(
                descriptor,
                ImplementationIdentity {
                    id: "mirabile-test-calculation-engine".into(),
                    version: engine_version.into(),
                    revision: None,
                },
                timezone_data_version,
            ),
            backend,
        }
    }

    fn prepare(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
    ) -> Result<mirabile_engine::PreparedCalculation, CalculationError> {
        self.engine
            .prepare(definition, record, &points(), &points())
    }

    fn calc_key(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
    ) -> Result<CalcKey, CalculationError> {
        self.prepare(definition, record)
            .map(|prepared| prepared.calc_key)
    }

    fn calculate(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
    ) -> Result<ChartSnapshot, CalculationError> {
        let prepared = self.prepare(definition, record)?;
        let backend = self
            .backend
            .calculate(&prepared.request)
            .expect("test backend calculation succeeds");
        let value = self.engine.complete(&prepared, backend)?;
        Ok(CalculationEngine::snapshot(&prepared, value))
    }

    fn snapshot_from_cached(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
        calculation: CalculationValue,
    ) -> Result<ChartSnapshot, CalculationError> {
        let prepared = self.prepare(definition, record)?;
        Ok(CalculationEngine::snapshot(&prepared, calculation))
    }
}

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
            location: Some(LocationAssertion {
                display_name: "Greenwich".into(),
                country_region: Some("GB".into()),
                latitude: Latitude::from_degrees(51.48).expect("valid latitude"),
                longitude: Longitude::from_degrees(0.0).expect("valid longitude"),
                atlas_provenance: None,
            }),
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
            calculation: CalculationSpec {
                houses: HouseSystem::Equal,
                ..CalculationSpec::default()
            },
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
            point_role: mirabile_core::PointRole::Primary,
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
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");

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
    renamed.description = Some("resource-level description".into());
    renamed.tags = vec!["example".into()];
    renamed.payload.subject = Some(SubjectInfo {
        display_name: "Different display name".into(),
        pronouns: Some("they/them".into()),
    });
    renamed.payload.notes.push(Note {
        text: "Non-calculation note".into(),
        created_at: Timestamp::from_unix_millis(1),
    });
    renamed.payload.source.description = "Different source wording".into();
    renamed
        .payload
        .location
        .as_mut()
        .expect("fixture location")
        .display_name = "Different atlas label".into();
    let mut renamed_definition = definition.clone();
    renamed_definition.title = "Renamed definition".into();
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");

    assert_eq!(
        engine.calc_key(&definition, &record).expect("first key"),
        engine
            .calc_key(&renamed_definition, &renamed)
            .expect("metadata-edited key")
    );
}

#[test]
fn every_calculation_dependency_changes_the_calculation_key() {
    let (record, definition) = sample_resources();
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");
    let original = engine.calc_key(&definition, &record).expect("original key");

    let mut moved = record.clone();
    moved
        .payload
        .location
        .as_mut()
        .expect("fixture location")
        .longitude = Longitude::from_degrees(1.0).expect("valid longitude");
    assert_ne!(
        original,
        engine.calc_key(&definition, &moved).expect("moved key")
    );

    let mut reconfigured = definition.clone();
    reconfigured.payload.calculation.houses = HouseSystem::WholeSign;
    assert_ne!(
        original,
        engine
            .calc_key(&reconfigured, &record)
            .expect("configuration key")
    );
    assert_ne!(
        original,
        TestEngine::new(DeterministicBackend, "engine-v2", "fixture-tz-v1")
            .calc_key(&definition, &record)
            .expect("engine identity key")
    );
    assert_ne!(
        original,
        TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v2")
            .calc_key(&definition, &record)
            .expect("timezone identity key")
    );
    assert_ne!(
        original,
        TestEngine::new(AlternateIdentityBackend, "engine-v1", "fixture-tz-v1")
            .calc_key(&definition, &record)
            .expect("provider identity key")
    );
}

#[test]
fn analysis_identity_consumes_semantic_metadata_but_ignores_order() {
    let (record, definition) = sample_resources();
    let snapshot = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1")
        .calculate(&definition, &record)
        .expect("snapshot");
    let baseline_points = points();
    let baseline_aspects = aspects(8.0);
    let baseline = AnalysisKey::derive(
        std::slice::from_ref(&snapshot.calc_key),
        &baseline_points,
        &baseline_aspects,
        &AnalysisProfile::default(),
    )
    .expect("baseline key");

    let mut reordered_aspects = baseline_aspects.clone();
    reordered_aspects.aspects.reverse();
    let mut reordered_points = baseline_points.clone();
    reordered_points.points.reverse();
    let unused_profile = AnalysisProfile {
        include_patterns: true,
        ..AnalysisProfile::default()
    };
    assert_eq!(
        baseline,
        AnalysisKey::derive(
            std::slice::from_ref(&snapshot.calc_key),
            &reordered_points,
            &reordered_aspects,
            &unused_profile,
        )
        .expect("order-independent key")
    );

    let mut semantic_change = baseline_aspects.clone();
    for aspect in &mut semantic_change.aspects {
        aspect.name = format!("Renamed {}", aspect.name);
        aspect.classification = AspectClass::Custom;
    }
    assert_ne!(
        baseline,
        AnalysisKey::derive(
            std::slice::from_ref(&snapshot.calc_key),
            &baseline_points,
            &semantic_change,
            &AnalysisProfile::default(),
        )
        .expect("semantic metadata key")
    );

    let mut numerical = baseline_aspects.clone();
    numerical.aspects[0].angle = Angle::from_degrees(1.0).expect("valid angle");
    assert_ne!(
        baseline,
        AnalysisKey::derive(
            std::slice::from_ref(&snapshot.calc_key),
            &baseline_points,
            &numerical,
            &AnalysisProfile::default(),
        )
        .expect("numerical key")
    );
    let mut fewer_points = baseline_points.clone();
    fewer_points.points.pop();
    assert_ne!(
        baseline,
        AnalysisKey::derive(
            std::slice::from_ref(&snapshot.calc_key),
            &fewer_points,
            &baseline_aspects,
            &AnalysisProfile::default(),
        )
        .expect("coverage key")
    );
}

#[test]
fn unresolved_point_categories_fail_at_analysis_and_layout_boundaries() {
    let (record, definition) = sample_resources();
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let unresolved = PointSet {
        points: vec![PointSelector::Category("planets".into())],
    };
    let analysis_error = AspectAnalyzer::analyze(
        &snapshot,
        &unresolved,
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect_err("category must be resolved");
    assert!(matches!(
        analysis_error,
        mirabile_engine::AnalysisError::Key(KeyError::UnresolvedPointCategory(_))
    ));

    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("resolved analysis");
    assert!(matches!(
        layout_wheel(&snapshot, &analysis, &unresolved, &wheel()),
        Err(mirabile_engine::LayoutError::UnresolvedPointCategory(_))
    ));
}

#[test]
fn aspect_change_reuses_snapshot_and_changes_only_downstream_keys() {
    let (record, definition) = sample_resources();
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");
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
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("analysis");
    let layout = layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("layout");
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

    assert_eq!(existing.calculation.houses, HouseSystem::Equal);
    assert_eq!(changed_defaults.houses, HouseSystem::WholeSign);
}

#[test]
fn disposable_cache_can_be_reconstructed_from_canonical_inputs() {
    let (record, definition) = sample_resources();
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");
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
fn cached_calculation_reuses_values_with_current_resource_context() {
    let (record, definition) = sample_resources();
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let mut cache = ComputationCache::default();
    cache.insert_snapshot(snapshot.clone());

    let mut revised_record = record
        .next_with_payload(record.payload.clone(), Timestamp::from_unix_millis(1))
        .expect("record revision");
    revised_record
        .payload
        .location
        .as_mut()
        .expect("fixture location")
        .display_name = "Current display label".into();
    let revised_definition = definition
        .next_with_payload(definition.payload.clone(), Timestamp::from_unix_millis(1))
        .expect("definition revision");
    let current_key = engine
        .calc_key(&revised_definition, &revised_record)
        .expect("current key");
    assert_eq!(current_key, snapshot.calc_key);

    let cached = cache
        .calculation(&current_key)
        .expect("cached calculation")
        .clone();
    let current = engine
        .snapshot_from_cached(&revised_definition, &revised_record, cached)
        .expect("current snapshot context");
    assert_eq!(current.calculation, snapshot.calculation);
    assert_eq!(
        current
            .context
            .definition
            .expect("canonical snapshot records its definition revision")
            .revision
            .get(),
        2
    );
    assert_eq!(current.context.records[0].revision.get(), 2);
    assert_eq!(
        current.context.location_display_name.as_deref(),
        Some("Current display label")
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn julian_day_fixtures_cover_offsets_calendars_year_zero_and_lmt_sign() {
    fn calculate_jd(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        calendar: CalendarSpec,
        zone: TimeZoneAssertion,
        longitude: f64,
    ) -> f64 {
        let (mut record, definition) = sample_resources();
        record.payload.time = TemporalAssertion {
            civil_datetime: CivilDateTime {
                date: CivilDate::new(year, month, day).expect("valid structural date"),
                time: CivilTime::new(hour, 0, 0).expect("valid time"),
            },
            calendar,
            zone,
            disambiguation: None,
        };
        record
            .payload
            .location
            .as_mut()
            .expect("fixture location")
            .longitude = Longitude::from_degrees(longitude).expect("valid longitude");
        TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1")
            .calculate(&definition, &record)
            .expect("calculation")
            .calculation
            .resolved_time
            .instant
            .julian_day()
    }

    let assert_close = |actual: f64, expected: f64| {
        assert!((actual - expected).abs() < 1.0e-9, "{actual} != {expected}");
    };
    assert_close(
        calculate_jd(
            2000,
            1,
            1,
            12,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::UniversalTime,
            0.0,
        ),
        2_451_545.0,
    );
    assert_close(
        calculate_jd(
            2000,
            1,
            1,
            14,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::FixedOffset(Offset::from_seconds(7_200).expect("valid offset")),
            0.0,
        ),
        2_451_545.0,
    );
    assert_close(
        calculate_jd(
            2000,
            1,
            1,
            7,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::FixedOffset(Offset::from_seconds(-18_000).expect("valid offset")),
            0.0,
        ),
        2_451_545.0,
    );
    assert_close(
        calculate_jd(
            2000,
            1,
            2,
            1,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::FixedOffset(Offset::from_seconds(7_200).expect("valid offset")),
            0.0,
        ),
        2_451_545.0 + 11.0 / 24.0,
    );
    assert_close(
        calculate_jd(
            1900,
            3,
            1,
            0,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::UniversalTime,
            0.0,
        ),
        2_415_079.5,
    );
    assert_close(
        calculate_jd(
            1900,
            3,
            1,
            0,
            CalendarSpec::Julian,
            TimeZoneAssertion::UniversalTime,
            0.0,
        ),
        2_415_092.5,
    );
    assert_close(
        calculate_jd(
            0,
            1,
            1,
            0,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::UniversalTime,
            0.0,
        ),
        1_721_059.5,
    );
    assert_close(
        calculate_jd(
            2000,
            1,
            1,
            12,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::LocalMeanTime,
            15.0,
        ),
        2_451_545.0 - 1.0 / 24.0,
    );
    assert_close(
        calculate_jd(
            2000,
            1,
            1,
            12,
            CalendarSpec::ProlepticGregorian,
            TimeZoneAssertion::LocalMeanTime,
            -15.0,
        ),
        2_451_545.0 + 1.0 / 24.0,
    );
}

#[test]
fn layout_produces_provider_neutral_semantic_scene() {
    let (record, definition) = sample_resources();
    let engine = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1");
    let snapshot = engine.calculate(&definition, &record).expect("snapshot");
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("analysis");
    let layout = layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("layout");
    let scene = Scene::from_wheel(&layout);

    assert!(!scene.circles.is_empty());
    assert_eq!(scene.zodiac.len(), 12);
    assert_eq!(scene.houses, layout.houses);
    assert_eq!(scene.angles, layout.angles);
    assert_eq!(scene.points, layout.points);
    assert_eq!(scene.aspects, layout.aspects);
    assert!(scene.labels.is_empty());
}

#[test]
fn layout_identity_uses_every_consumed_semantic_and_display_input() {
    let (record, definition) = sample_resources();
    let snapshot = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1")
        .calculate(&definition, &record)
        .expect("snapshot");
    let displayed = points();
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &displayed,
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("analysis");
    let baseline_wheel = wheel();
    let baseline =
        layout_wheel(&snapshot, &analysis, &displayed, &baseline_wheel).expect("baseline layout");

    let mut role_only = baseline_wheel.clone();
    role_only.rings[0].point_role = mirabile_core::PointRole::Transit;
    let mut reordered = displayed.clone();
    reordered.points.reverse();
    let equivalent =
        layout_wheel(&snapshot, &analysis, &reordered, &role_only).expect("equivalent layout");
    assert_eq!(baseline.key, equivalent.key);
    assert_eq!(baseline.points, equivalent.points);
    assert_eq!(baseline.aspects, equivalent.aspects);

    let mut display_change = baseline_wheel.clone();
    display_change.labels.show_degrees = false;
    display_change.houses.show_numbers = false;
    let changed =
        layout_wheel(&snapshot, &analysis, &displayed, &display_change).expect("display layout");
    assert_ne!(baseline.key, changed.key);
    assert_ne!(baseline.points, changed.points);

    let mut inner_change = baseline_wheel.clone();
    inner_change.rings[0].geometry.inner_radius = 120.0;
    let changed =
        layout_wheel(&snapshot, &analysis, &displayed, &inner_change).expect("inner layout");
    assert_ne!(baseline.key, changed.key);

    let mut geometry_change = baseline_wheel;
    geometry_change.rings[0].geometry.outer_radius = 160.0;
    let changed =
        layout_wheel(&snapshot, &analysis, &displayed, &geometry_change).expect("changed layout");
    assert_ne!(baseline.key, changed.key);
}

fn semantic_fixture() -> (ChartSnapshot, mirabile_engine::ChartAnalysis) {
    let (record, definition) = sample_resources();
    let snapshot = TestEngine::new(DeterministicBackend, "engine-v1", "fixture-tz-v1")
        .calculate(&definition, &record)
        .expect("snapshot");
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &points(),
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("analysis");
    (snapshot, analysis)
}

fn degrees(value: f64) -> Angle {
    Angle::from_degrees(value).expect("valid fixture angle")
}

fn assert_close_degrees(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1.0e-9,
        "expected {expected} degrees, got {actual}"
    );
}

#[test]
fn professional_orientation_uses_actual_ascendant_and_explicit_rotation() {
    let (mut snapshot, analysis) = semantic_fixture();
    snapshot.calculation.angles = AngleState {
        ascendant: Some(degrees(123.0)),
        midheaven: Some(degrees(22.0)),
    };
    let layout = layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("layout");
    assert_close_degrees(layout.rotation_degrees, 147.0);
    let asc = layout
        .angles
        .iter()
        .find(|angle| angle.id == "asc")
        .expect("ASC");
    assert_close_degrees(asc.screen_angle_degrees, 180.0);
    assert!(!asc.derived_opposite);
    let dsc = layout
        .angles
        .iter()
        .find(|angle| angle.id == "dsc")
        .expect("DSC");
    assert_close_degrees(dsc.longitude_degrees, 303.0);
    assert!(dsc.derived_opposite);
    let ic = layout
        .angles
        .iter()
        .find(|angle| angle.id == "ic")
        .expect("IC");
    assert_close_degrees(ic.longitude_degrees, 202.0);
    assert!(ic.derived_opposite);

    snapshot.calculation.angles = AngleState {
        ascendant: None,
        midheaven: None,
    };
    let unrotated = layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("layout");
    assert_close_degrees(unrotated.rotation_degrees, 0.0);
    assert_close_degrees(unrotated.zodiac[0].screen_angle_degrees, 270.0);
    assert!(unrotated.angles.is_empty());

    let explicit = layout_wheel_in_bounds(
        &snapshot,
        &analysis,
        &points(),
        &wheel(),
        Some(degrees(30.0)),
        WheelLayoutBounds::default(),
    )
    .expect("explicitly rotated layout");
    assert_close_degrees(explicit.rotation_degrees, 30.0);
    assert_close_degrees(explicit.zodiac[0].screen_angle_degrees, 300.0);
}

#[test]
fn house_layout_preserves_equal_placidus_and_no_houses_results() {
    let (mut snapshot, analysis) = semantic_fixture();
    let equal_cusps = (0_u32..12)
        .map(|index| degrees(15.0 + f64::from(index) * 30.0))
        .collect::<Vec<_>>();
    snapshot.calculation.houses = Some(HouseState {
        cusps: equal_cusps.clone(),
    });
    let equal = layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("equal layout");
    assert_eq!(equal.houses.len(), 12);
    for (house, cusp) in equal.houses.iter().zip(equal_cusps) {
        assert_close_degrees(house.cusp_longitude_degrees, cusp.degrees());
        assert!(house.show_cusp);
        assert!(house.show_number);
    }

    let placidus_cusps = [
        15.0, 37.0, 64.5, 101.0, 139.0, 171.0, 195.0, 217.0, 244.5, 281.0, 319.0, 351.0,
    ];
    snapshot.calculation.houses = Some(HouseState {
        cusps: placidus_cusps.into_iter().map(degrees).collect(),
    });
    let placidus =
        layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("Placidus layout");
    assert_eq!(placidus.houses.len(), 12);
    assert_ne!(equal.key, placidus.key);
    assert!(placidus.houses.windows(2).any(|pair| {
        (pair[1].cusp_longitude_degrees - pair[0].cusp_longitude_degrees - 30.0).abs() > 1.0
    }));

    snapshot.calculation.houses = None;
    let none = layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("no houses");
    assert!(none.houses.is_empty());
    assert_ne!(placidus.key, none.key);
}

#[test]
fn longitude_formatting_rounds_minutes_and_carries_across_boundaries() {
    let taurus = format_longitude(29.0 + 59.6 / 60.0);
    assert_eq!(taurus.sign_id, "taurus");
    assert_eq!((taurus.degree, taurus.minute), (0, 0));
    assert_eq!(taurus.text, "00°00′ ♉ Taurus");

    let aries = format_longitude(359.0 + 59.6 / 60.0);
    assert_eq!(aries.sign_id, "aries");
    assert_eq!((aries.degree, aries.minute), (0, 0));

    let wrapped = format_longitude(390.5);
    assert_eq!(wrapped.sign_id, "taurus");
    assert_eq!((wrapped.degree, wrapped.minute), (0, 30));
}

#[test]
fn supported_point_glyphs_fallback_and_retrograde_are_truthful() {
    let (mut snapshot, analysis) = semantic_fixture();
    let mercury = PointId::new("mercury").expect("point ID");
    snapshot
        .calculation
        .celestial_positions
        .get_mut(&mercury)
        .expect("Mercury state")
        .retrograde = true;
    let sun = PointId::new("sun").expect("point ID");
    let mut fallback_state = snapshot.calculation.celestial_positions[&sun].clone();
    fallback_state.longitude = degrees(44.0);
    fallback_state.retrograde = false;
    let ceres = PointId::new("dwarf_ceres").expect("point ID");
    snapshot
        .calculation
        .celestial_positions
        .insert(ceres.clone(), fallback_state);
    let mut displayed = points();
    displayed.points.push(PointSelector::Point(ceres.clone()));

    let layout =
        layout_wheel(&snapshot, &analysis, &displayed, &wheel()).expect("point metadata layout");
    let expected = [
        ("sun", "☉", "Sun"),
        ("moon", "☽", "Moon"),
        ("mercury", "☿", "Mercury"),
        ("venus", "♀", "Venus"),
        ("mars", "♂", "Mars"),
        ("jupiter", "♃", "Jupiter"),
    ];
    for (id, glyph, name) in expected {
        let point = layout
            .points
            .iter()
            .find(|point| point.point.as_str() == id)
            .expect("supported point");
        assert_eq!(point.glyph, glyph);
        assert_eq!(point.name, name);
        assert!(!point.glyph_fallback);
    }
    let mercury = layout
        .points
        .iter()
        .find(|point| point.point.as_str() == "mercury")
        .expect("Mercury marker");
    assert!(mercury.retrograde);
    assert!(mercury.show_retrograde);
    let fallback = layout
        .points
        .iter()
        .find(|point| point.point == ceres)
        .expect("fallback point");
    assert_eq!(fallback.glyph, "dwarf_ceres");
    assert_eq!(fallback.name, "Dwarf Ceres");
    assert!(fallback.glyph_fallback);
    assert!(!fallback.retrograde);
}

#[test]
fn every_aspect_hit_retains_identity_even_without_a_chord() {
    let (snapshot, mut analysis) = semantic_fixture();
    let sun = PointId::new("sun").expect("point ID");
    let moon = PointId::new("moon").expect("point ID");
    let missing = PointId::new("missing-point").expect("point ID");
    let fixtures = [
        ("conjunction", "Conjunction", AspectClass::Major, 0.0),
        ("opposition", "Opposition", AspectClass::Major, 180.0),
        ("square", "Square", AspectClass::Major, 90.0),
        ("trine", "Trine", AspectClass::Major, 120.0),
        ("sextile", "Sextile", AspectClass::Major, 60.0),
        ("quincunx", "Quincunx", AspectClass::Minor, 150.0),
        ("custom-17", "Custom Seventeen", AspectClass::Custom, 17.0),
    ];
    analysis.aspects = fixtures
        .into_iter()
        .map(|(id, name, classification, separation)| AspectHit {
            lhs: sun.clone(),
            rhs: moon.clone(),
            aspect: AspectId::new(id).expect("aspect ID"),
            name: name.into(),
            classification,
            separation: degrees(separation),
            orb: degrees(0.5),
            applying: Some(true),
        })
        .chain(std::iter::once(AspectHit {
            lhs: sun.clone(),
            rhs: missing,
            aspect: AspectId::new("missing-anchor").expect("aspect ID"),
            name: "Missing Anchor".into(),
            classification: AspectClass::Harmonic,
            separation: degrees(72.0),
            orb: degrees(1.0),
            applying: None,
        }))
        .collect();

    let layout = layout_wheel(&snapshot, &analysis, &points(), &wheel()).expect("aspect layout");
    assert_eq!(layout.aspects.len(), 8);
    let conjunction = layout
        .aspects
        .iter()
        .find(|aspect| aspect.aspect_id == "conjunction")
        .expect("conjunction");
    assert_eq!(conjunction.style, AspectVisualStyle::Conjunction);
    assert!(!conjunction.draw_chord);
    let unknown = layout
        .aspects
        .iter()
        .find(|aspect| aspect.aspect_id == "custom-17")
        .expect("custom aspect");
    assert_eq!(unknown.style, AspectVisualStyle::Neutral);
    assert_eq!(unknown.name, "Custom Seventeen");
    assert_eq!(unknown.classification, AspectClass::Custom);
    assert!(unknown.draw_chord);
    let missing = layout
        .aspects
        .iter()
        .find(|aspect| aspect.aspect_id == "missing-anchor")
        .expect("missing anchor aspect");
    assert!(!missing.draw_chord);
    assert_eq!(missing.applying, None);
}

#[test]
fn dense_circular_labels_are_bounded_order_independent_and_repeatable() {
    let (mut snapshot, _) = semantic_fixture();
    for (point, longitude) in points()
        .direct_points()
        .zip([359.4, 359.7, 0.0, 0.2, 0.45, 0.7])
    {
        snapshot
            .calculation
            .celestial_positions
            .get_mut(point)
            .expect("point state")
            .longitude = degrees(longitude);
    }
    let displayed = points();
    let analysis = AspectAnalyzer::analyze(
        &snapshot,
        &displayed,
        &aspects(8.0),
        &AnalysisProfile::default(),
    )
    .expect("cluster analysis");
    let compact_bounds = WheelLayoutBounds {
        width: 320.0,
        height: 320.0,
    };
    let compact = layout_wheel_in_bounds(
        &snapshot,
        &analysis,
        &displayed,
        &wheel(),
        None,
        compact_bounds,
    )
    .expect("compact layout");
    let repeated = layout_wheel_in_bounds(
        &snapshot,
        &analysis,
        &displayed,
        &wheel(),
        None,
        compact_bounds,
    )
    .expect("repeated compact layout");
    assert_eq!(compact, repeated);
    assert_eq!(compact.points.len(), 6);
    assert!(compact.points.iter().any(|point| point.leader.is_some()));
    for point in &compact.points {
        assert!((0.0..=compact.width).contains(&point.x));
        assert!((0.0..=compact.height).contains(&point.y));
        assert!((0.0..=compact.width).contains(&point.label_x));
        assert!((0.0..=compact.height).contains(&point.label_y));
        if point.label_lane > 0
            || (point.longitude_degrees - point.label_angle_degrees)
                .abs()
                .rem_euclid(360.0)
                > 1.5
        {
            assert!(point.leader.is_some());
        }
    }

    let mut reversed = displayed.clone();
    reversed.points.reverse();
    let reversed_layout = layout_wheel_in_bounds(
        &snapshot,
        &analysis,
        &reversed,
        &wheel(),
        None,
        compact_bounds,
    )
    .expect("reversed layout");
    assert_eq!(compact, reversed_layout);

    let regular = layout_wheel_in_bounds(
        &snapshot,
        &analysis,
        &displayed,
        &wheel(),
        None,
        WheelLayoutBounds::default(),
    )
    .expect("regular layout");
    assert_eq!(regular.points.len(), compact.points.len());
    assert_ne!(regular.key, compact.key);
}
