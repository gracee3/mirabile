use mirabile_core::{
    AnalysisProfile, Angle, AspectClass, AspectDefinition, AspectFieldSpec, AspectId, AspectSet,
    CalculationSpec, CalendarSpec, ChartDefinition, ChartRecord, ChartSlotId, ChartSource,
    CivilDate, CivilDateTime, CivilTime, EventKind, HouseDisplaySpec, HouseSystem, LabelSpec,
    Latitude, LocationAssertion, Longitude, Note, Offset, OrbPolicy, PointId, PointSelector,
    PointSet, ResourceEnvelope, ResourceId, RingGeometry, RingSpec, SourceProvenance, SourceType,
    SubjectInfo, TemporalAssertion, Theme, TimeZoneAssertion, Timestamp, WheelTemplate,
    ZodiacDisplaySpec,
};
use mirabile_engine::{
    AnalysisKey, AspectAnalyzer, BackendDescriptor, CalcKey, CalculationBackend,
    CalculationBackendError, CalculationBackendResult, CalculationEngine, CalculationError,
    CalculationValue, ChartSnapshot, ComputationCache, DeterministicBackend,
    ImplementationIdentity, KeyError, ResolvedCalculationRequest, Scene, layout_wheel, render_key,
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
fn analysis_identity_ignores_labels_classification_and_order() {
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

    let mut display_only = baseline_aspects.clone();
    display_only.aspects.reverse();
    for aspect in &mut display_only.aspects {
        aspect.name = format!("Renamed {}", aspect.name);
        aspect.classification = AspectClass::Custom;
    }
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
            &display_only,
            &unused_profile,
        )
        .expect("display-only key")
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
fn layout_produces_astrology_free_scene_primitives() {
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
    assert_eq!(scene.labels.len(), layout.points.len());
}

#[test]
fn layout_identity_uses_only_consumed_geometry_and_resolved_points() {
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

    let mut display_only = baseline_wheel.clone();
    display_only.rings[0].geometry.inner_radius = 120.0;
    display_only.rings[0].point_role = mirabile_core::PointRole::Transit;
    display_only.labels.show_degrees = false;
    display_only.houses.show_numbers = false;
    let mut reordered = displayed.clone();
    reordered.points.reverse();
    let equivalent =
        layout_wheel(&snapshot, &analysis, &reordered, &display_only).expect("equivalent layout");
    assert_eq!(baseline.key, equivalent.key);
    assert_eq!(baseline.points, equivalent.points);
    assert_eq!(baseline.aspects, equivalent.aspects);

    let mut geometry_change = baseline_wheel;
    geometry_change.rings[0].geometry.outer_radius = 160.0;
    let changed =
        layout_wheel(&snapshot, &analysis, &displayed, &geometry_change).expect("changed layout");
    assert_ne!(baseline.key, changed.key);
}
