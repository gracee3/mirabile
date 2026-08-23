use mirabile_core::{
    CalculationSpec, CalendarSpec, ChartDefinition, ChartRecord, ChartSource, CivilDate,
    CivilDateTime, CivilTime, CoordinateSystem, CorrectionSpec, EventKind, HouseSystem, Latitude,
    LocationAssertion, Longitude, PointId, PointSelector, PointSet, ResourceEnvelope, ResourceId,
    SourceProvenance, SourceType, TemporalAssertion, TimeZoneAssertion, Timestamp, ZodiacSpec,
};
use mirabile_engine::{
    BackendCapability, CalcKey, CalculationBackend, CalculationEngine, CalculationOutcome,
    CalculationRequestId, CalculationWorkerFailure, CalculationWorkerFailureCategory,
    CalculationWorkerRequest, CalculationWorkerResult, DeterministicBackend,
    ImplementationIdentity, WorkerProtocolVersion, execute_calculation_request,
};

fn resources() -> (
    ResourceEnvelope<ChartRecord>,
    ResourceEnvelope<ChartDefinition>,
) {
    let record_id = ResourceId::new();
    let record = ResourceEnvelope::with_id(
        record_id,
        "Calculation fixture",
        ChartRecord {
            event_kind: EventKind::Birth,
            subject: None,
            time: TemporalAssertion {
                civil_datetime: CivilDateTime {
                    date: CivilDate::new(2000, 1, 1).expect("date"),
                    time: CivilTime::new(12, 0, 0).expect("time"),
                },
                calendar: CalendarSpec::ProlepticGregorian,
                zone: TimeZoneAssertion::UniversalTime,
                disambiguation: None,
            },
            location: Some(LocationAssertion {
                display_name: "Greenwich".into(),
                country_region: Some("GB".into()),
                latitude: Latitude::from_degrees(51.48).expect("latitude"),
                longitude: Longitude::from_degrees(0.0).expect("longitude"),
                atlas_provenance: None,
            }),
            source: SourceProvenance {
                description: "Calculation contract fixture".into(),
                source_type: SourceType::UserAssertion,
                recorded_by: None,
            },
            notes: Vec::new(),
            life_events: Vec::new(),
        },
        Timestamp::from_unix_millis(0),
    );
    let definition = ResourceEnvelope::new(
        "Fixture definition",
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

fn point_set(ids: &[&str]) -> PointSet {
    PointSet {
        points: ids
            .iter()
            .map(|id| PointSelector::Point(PointId::new(*id).expect("point ID")))
            .collect(),
    }
}

fn engine() -> CalculationEngine {
    CalculationEngine::new(
        DeterministicBackend.descriptor(),
        engine_identity(),
        "fixture-tz-v1",
    )
}

fn engine_identity() -> ImplementationIdentity {
    ImplementationIdentity {
        id: "mirabile-calculation-engine".into(),
        version: "test-v1".into(),
        revision: Some("engine-r1".into()),
    }
}

fn prepared() -> mirabile_engine::PreparedCalculation {
    let (record, definition) = resources();
    engine()
        .prepare(
            &definition,
            &record,
            &point_set(&["sun", "moon", "mercury"]),
            &point_set(&["sun", "moon"]),
        )
        .expect("prepared request")
}

fn worker_request() -> CalculationWorkerRequest {
    let prepared = prepared();
    CalculationWorkerRequest {
        protocol_version: WorkerProtocolVersion::CURRENT,
        request_id: CalculationRequestId::new(41).expect("request ID"),
        calc_key: prepared.calc_key,
        backend: DeterministicBackend.descriptor().fingerprint,
        request: prepared.request,
    }
}

#[test]
fn resolved_request_separates_celestial_houses_and_derived_responsibilities() {
    let request = prepared().request;

    assert_eq!(
        request.celestial.requested_points,
        ["mercury", "moon", "sun"]
            .into_iter()
            .map(|id| PointId::new(id).expect("point ID"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        request.houses.as_ref().map(|houses| houses.system),
        Some(HouseSystem::Equal)
    );
    assert!(request.derived.points.is_empty());

    let (record, mut definition) = resources();
    definition.payload.calculation.houses = HouseSystem::NoHouses;
    let no_houses = engine()
        .prepare(
            &definition,
            &record,
            &point_set(&["sun"]),
            &point_set(&["sun"]),
        )
        .expect("no-houses request");
    assert!(no_houses.request.houses.is_none());

    let fortune = engine()
        .prepare(
            &definition,
            &record,
            &point_set(&["part_of_fortune"]),
            &point_set(&[]),
        )
        .expect("derived request");
    assert_eq!(fortune.request.derived.points.len(), 1);
    assert_eq!(
        fortune.request.celestial.requested_points,
        ["moon", "sun"]
            .into_iter()
            .map(|id| PointId::new(id).expect("point ID"))
            .collect::<Vec<_>>()
    );
}

#[test]
fn locationless_geocentric_no_house_calculation_is_truthful_and_executable() {
    let (mut record, mut definition) = resources();
    record.payload.location = None;
    definition.payload.calculation.houses = HouseSystem::NoHouses;
    let prepared = engine()
        .prepare(
            &definition,
            &record,
            &point_set(&["sun", "moon"]),
            &point_set(&[]),
        )
        .expect("locationless geocentric positions are valid");
    assert_eq!(prepared.request.context.location, None);
    assert!(prepared.request.houses.is_none());
    let result = DeterministicBackend
        .calculate(&prepared.request)
        .expect("backend needs no invented observer for geocentric positions");
    let value = engine()
        .complete(&prepared, result)
        .expect("locationless calculation completes");
    assert_eq!(value.numeric_location, None);

    definition.payload.calculation.houses = HouseSystem::Equal;
    assert!(matches!(
        engine().prepare(&definition, &record, &point_set(&["sun"]), &point_set(&[]),),
        Err(mirabile_engine::CalculationError::LocationRequired(
            "house and angle calculation"
        ))
    ));

    definition.payload.calculation.houses = HouseSystem::NoHouses;
    definition.payload.calculation.coordinates = CoordinateSystem::Topocentric;
    assert!(matches!(
        engine().prepare(&definition, &record, &point_set(&["sun"]), &point_set(&[]),),
        Err(mirabile_engine::CalculationError::LocationRequired(
            "topocentric celestial calculation"
        ))
    ));
}

#[test]
fn payload_only_resolution_needs_no_canonical_identity() {
    let (mut record, mut definition) = resources();
    record.payload.location = None;
    definition.payload.calculation.houses = HouseSystem::NoHouses;
    let resolved = engine()
        .resolve(
            &record.payload,
            &definition.payload.calculation,
            &point_set(&["sun"]),
            &point_set(&[]),
        )
        .expect("draft payloads resolve directly");
    let prepared = resolved.with_context(mirabile_engine::SnapshotContext {
        definition: None,
        records: Vec::new(),
        location_display_name: None,
    });
    assert_eq!(prepared.snapshot_context.definition, None);
    assert!(prepared.snapshot_context.records.is_empty());
}

#[test]
fn deterministic_backend_advertises_multiple_capabilities_and_rejects_unknown_points() {
    let descriptor = DeterministicBackend.descriptor();
    assert!(descriptor.capabilities.celestial.is_some());
    assert_eq!(
        descriptor
            .capabilities
            .houses
            .as_ref()
            .expect("house capability")
            .supported_systems,
        vec![HouseSystem::Equal]
    );
    assert!(descriptor.capabilities.derived.is_none());

    let mut request = prepared().request;
    request
        .celestial
        .requested_points
        .push(PointId::new("pluto").expect("point ID"));
    let error = DeterministicBackend
        .calculate(&request)
        .expect_err("unsupported point must fail");
    assert_eq!(
        error.capability,
        Some(BackendCapability::CelestialPositions)
    );
}

#[test]
fn deterministic_backend_rejects_unimplemented_celestial_semantics() {
    let baseline = prepared().request;
    for coordinates in [
        CoordinateSystem::Topocentric,
        CoordinateSystem::Heliocentric,
    ] {
        let mut request = baseline.clone();
        request.celestial.coordinates = coordinates;
        let error = DeterministicBackend
            .calculate(&request)
            .expect_err("non-geocentric coordinates must fail");
        assert_eq!(
            error.capability,
            Some(BackendCapability::CelestialPositions)
        );
    }

    for corrections in [
        CorrectionSpec {
            aberration: true,
            light_time: false,
            nutation: false,
        },
        CorrectionSpec {
            aberration: false,
            light_time: true,
            nutation: false,
        },
        CorrectionSpec {
            aberration: false,
            light_time: false,
            nutation: true,
        },
    ] {
        let mut request = baseline.clone();
        request.celestial.corrections = corrections;
        let error = DeterministicBackend
            .calculate(&request)
            .expect_err("enabled corrections must fail");
        assert_eq!(
            error.capability,
            Some(BackendCapability::CelestialPositions)
        );
    }

    let mut sidereal = baseline.clone();
    sidereal.zodiac = mirabile_engine::ZodiacCalculationRequest::Sidereal {
        ayanamsa: mirabile_engine::AyanamsaConfiguration {
            id: "lahiri".into(),
            parameters: std::collections::BTreeMap::new(),
        },
    };
    sidereal.houses.as_mut().expect("houses").zodiac = sidereal.zodiac.clone();
    let error = DeterministicBackend
        .calculate(&sidereal)
        .expect_err("sidereal calculations must fail");
    assert_eq!(
        error.capability,
        Some(BackendCapability::CelestialPositions)
    );
}

#[test]
fn deterministic_backend_rejects_unimplemented_house_and_derived_semantics() {
    let baseline = prepared().request;
    for system in [HouseSystem::Placidus, HouseSystem::WholeSign] {
        let mut request = baseline.clone();
        request.houses.as_mut().expect("houses").system = system;
        let error = DeterministicBackend
            .calculate(&request)
            .expect_err("non-Equal houses must fail");
        assert_eq!(error.capability, Some(BackendCapability::HousesAndAngles));
    }

    let mut derived = baseline;
    derived
        .derived
        .points
        .push(mirabile_engine::DerivedPointRequest {
            point: PointId::new("custom_lot").expect("point ID"),
            formula: mirabile_engine::DerivedFormula::Named {
                id: "fixture-formula".into(),
                parameters: std::collections::BTreeMap::new(),
            },
        });
    let error = DeterministicBackend
        .calculate(&derived)
        .expect_err("derived formulas must fail");
    assert_eq!(error.capability, Some(BackendCapability::DerivedPoints));
}

#[test]
fn provenance_is_structured_and_distinguishes_every_material_component() {
    let prepared = prepared();
    let baseline = DeterministicBackend
        .calculate(&prepared.request)
        .expect("backend result")
        .provenance;

    let mut backend_revision = baseline.clone();
    backend_revision.backend.revision = Some("backend-r2".into());
    assert_ne!(baseline, backend_revision);

    let mut model = baseline.clone();
    model
        .celestial
        .model
        .as_mut()
        .expect("model")
        .data_fingerprint = Some("different-model-data".into());
    assert_ne!(baseline, model);

    let mut house_implementation = baseline.clone();
    house_implementation
        .houses
        .as_mut()
        .expect("houses")
        .implementation
        .revision = Some("houses-r2".into());
    assert_ne!(baseline, house_implementation);

    let mut house_system = baseline.clone();
    house_system.houses.as_mut().expect("houses").system = HouseSystem::WholeSign;
    assert_ne!(baseline, house_system);

    let mut coordinates = baseline.clone();
    coordinates.celestial.coordinates = CoordinateSystem::Topocentric;
    assert_ne!(baseline, coordinates);

    let mut corrections = baseline.clone();
    corrections.celestial.corrections = CorrectionSpec {
        aberration: true,
        light_time: false,
        nutation: false,
    };
    assert_ne!(baseline, corrections);

    let mut sidereal = baseline.clone();
    sidereal.celestial.zodiac = mirabile_engine::ZodiacCalculationRequest::Sidereal {
        ayanamsa: mirabile_engine::AyanamsaConfiguration {
            id: "lahiri".into(),
            parameters: std::collections::BTreeMap::new(),
        },
    };
    assert_ne!(baseline, sidereal);

    let mut ayanamsa = sidereal.clone();
    if let mirabile_engine::ZodiacCalculationRequest::Sidereal { ayanamsa } =
        &mut ayanamsa.celestial.zodiac
    {
        ayanamsa.id = "fagan_bradley".into();
    }
    assert_ne!(sidereal, ayanamsa);

    let mut lunar_node = baseline.clone();
    lunar_node.celestial.lunar_node = mirabile_core::LunarNodeType::Mean;
    assert_ne!(baseline, lunar_node);

    let mut black_moon = baseline.clone();
    black_moon.celestial.black_moon = mirabile_core::BlackMoonType::Osculating;
    assert_ne!(baseline, black_moon);

    let derived_day_night = mirabile_engine::DerivedCalculationProvenance {
        implementation: ImplementationIdentity {
            id: "mirabile-derived-fixture".into(),
            version: "1".into(),
            revision: Some("derived-r1".into()),
        },
        formulas: vec![mirabile_engine::DerivedFormulaProvenance {
            point: PointId::new("part_of_fortune").expect("point ID"),
            formula: mirabile_engine::DerivedFormula::PartOfFortune {
                formula: mirabile_core::FortuneFormula::DayNight,
            },
        }],
    };
    let mut derived_always = derived_day_night.clone();
    derived_always.formulas[0].formula = mirabile_engine::DerivedFormula::PartOfFortune {
        formula: mirabile_core::FortuneFormula::AlwaysAscendantPlusMoonMinusSun,
    };
    assert_ne!(derived_day_night, derived_always);
}

#[test]
fn calculation_value_retains_full_mirabile_and_backend_provenance() {
    let prepared = prepared();
    let backend_result = DeterministicBackend
        .calculate(&prepared.request)
        .expect("backend result");
    let value = engine()
        .complete(&prepared, backend_result)
        .expect("calculation value");

    assert_eq!(
        value.provenance.mirabile.calculation_engine,
        engine_identity()
    );
    assert_eq!(
        value.provenance.mirabile.timezone_data_version,
        "fixture-tz-v1"
    );
    assert_eq!(
        value.provenance.backend,
        DeterministicBackend.descriptor().fingerprint.backend
    );
    assert_eq!(
        value.provenance.houses.as_ref().map(|houses| houses.system),
        Some(HouseSystem::Equal)
    );
    assert_eq!(
        value.provenance.celestial.coordinates,
        CoordinateSystem::Geocentric
    );
    assert_eq!(
        value.provenance.celestial.lunar_node,
        prepared.request.celestial.lunar_node
    );
    assert_eq!(
        value.provenance.celestial.black_moon,
        prepared.request.celestial.black_moon
    );
}

#[test]
fn celestial_node_and_black_moon_provenance_is_validated() {
    let prepared = prepared();
    let baseline = DeterministicBackend
        .calculate(&prepared.request)
        .expect("backend result");

    let mut wrong_node = baseline.clone();
    wrong_node.provenance.celestial.lunar_node = match prepared.request.celestial.lunar_node {
        mirabile_core::LunarNodeType::Mean => mirabile_core::LunarNodeType::True,
        mirabile_core::LunarNodeType::True => mirabile_core::LunarNodeType::Mean,
    };
    assert!(matches!(
        engine().complete(&prepared, wrong_node),
        Err(mirabile_engine::CalculationError::BackendResultMismatch(_))
    ));

    let mut wrong_black_moon = baseline;
    wrong_black_moon.provenance.celestial.black_moon = match prepared.request.celestial.black_moon {
        mirabile_core::BlackMoonType::Mean => mirabile_core::BlackMoonType::Osculating,
        mirabile_core::BlackMoonType::Osculating => mirabile_core::BlackMoonType::Mean,
    };
    assert!(matches!(
        engine().complete(&prepared, wrong_black_moon),
        Err(mirabile_engine::CalculationError::BackendResultMismatch(_))
    ));
}

#[test]
fn calc_key_changes_for_every_execution_semantic_and_implementation_identity() {
    let prepared = prepared();
    let descriptor = DeterministicBackend.descriptor();
    let baseline = CalcKey::derive(
        &prepared.request,
        &engine_identity(),
        &descriptor.fingerprint,
    )
    .expect("baseline key");

    let assert_request_changes = |request: &mirabile_engine::ResolvedCalculationRequest| {
        assert_ne!(
            baseline,
            CalcKey::derive(request, &engine_identity(), &descriptor.fingerprint)
                .expect("changed key")
        );
    };

    let mut coordinates = prepared.request.clone();
    coordinates.celestial.coordinates = CoordinateSystem::Topocentric;
    assert_request_changes(&coordinates);

    let mut corrections = prepared.request.clone();
    corrections.celestial.corrections.aberration = true;
    assert_request_changes(&corrections);

    let mut houses = prepared.request.clone();
    houses.houses.as_mut().expect("houses").system = HouseSystem::WholeSign;
    assert_request_changes(&houses);

    let mut sidereal = prepared.request.clone();
    sidereal.zodiac = mirabile_engine::ZodiacCalculationRequest::Sidereal {
        ayanamsa: mirabile_engine::AyanamsaConfiguration {
            id: "lahiri".into(),
            parameters: std::collections::BTreeMap::new(),
        },
    };
    sidereal.houses.as_mut().expect("houses").zodiac = sidereal.zodiac.clone();
    assert_request_changes(&sidereal);

    let mut ayanamsa = sidereal.clone();
    if let mirabile_engine::ZodiacCalculationRequest::Sidereal { ayanamsa } = &mut ayanamsa.zodiac {
        ayanamsa.id = "fagan_bradley".into();
    }
    ayanamsa.houses.as_mut().expect("houses").zodiac = ayanamsa.zodiac.clone();
    assert_request_changes(&ayanamsa);

    let mut requested_points = prepared.request.clone();
    requested_points
        .celestial
        .requested_points
        .push(PointId::new("venus").expect("point ID"));
    assert_request_changes(&requested_points);

    let mut backend_revision = descriptor.fingerprint.clone();
    backend_revision.backend.revision = Some("backend-r2".into());
    assert_ne!(
        baseline,
        CalcKey::derive(&prepared.request, &engine_identity(), &backend_revision)
            .expect("backend revision key")
    );

    let mut model = descriptor.fingerprint.clone();
    model
        .celestial
        .as_mut()
        .expect("celestial")
        .model
        .as_mut()
        .expect("model")
        .data_fingerprint = Some("different-model-data".into());
    assert_ne!(
        baseline,
        CalcKey::derive(&prepared.request, &engine_identity(), &model).expect("model key")
    );

    let mut house_implementation = descriptor.fingerprint.clone();
    house_implementation
        .houses
        .as_mut()
        .expect("houses")
        .implementation
        .revision = Some("houses-r2".into());
    assert_ne!(
        baseline,
        CalcKey::derive(&prepared.request, &engine_identity(), &house_implementation)
            .expect("house implementation key")
    );
}

#[test]
fn canonical_metadata_does_not_enter_resolved_request_or_calc_key() {
    let (record, definition) = resources();
    let baseline = engine()
        .prepare(
            &definition,
            &record,
            &point_set(&["sun"]),
            &point_set(&["sun"]),
        )
        .expect("baseline");
    let mut metadata_record = record.clone();
    metadata_record.title = "Renamed resource".into();
    metadata_record
        .payload
        .location
        .as_mut()
        .expect("fixture location")
        .display_name = "Renamed place".into();
    metadata_record.payload.source.description = "Different source wording".into();
    metadata_record.payload.notes.push(mirabile_core::Note {
        text: "Non-calculation note".into(),
        created_at: Timestamp::from_unix_millis(1),
    });
    metadata_record
        .payload
        .life_events
        .push(mirabile_core::LifeEvent {
            title: "Non-calculation event".into(),
            time: metadata_record.payload.time.clone(),
            location: None,
            notes: Vec::new(),
        });
    let mut metadata_definition = definition.clone();
    metadata_definition.title = "Renamed definition".into();
    let changed = engine()
        .prepare(
            &metadata_definition,
            &metadata_record,
            &point_set(&["sun"]),
            &point_set(&["sun"]),
        )
        .expect("metadata changed");
    assert_eq!(baseline.request, changed.request);
    assert_eq!(baseline.calc_key, changed.calc_key);
}

#[test]
fn worker_request_success_and_typed_failure_round_trip() {
    let request = worker_request();
    let request_json = serde_json::to_string(&request).expect("request serialization");
    let decoded_request: CalculationWorkerRequest =
        serde_json::from_str(&request_json).expect("request deserialization");
    assert_eq!(request, decoded_request);
    assert_eq!(decoded_request.request_id.get(), 41);
    assert_eq!(decoded_request.calc_key, request.calc_key);
    for forbidden in [
        "ChartRecord",
        "ChartDefinition",
        "ResourceEnvelope",
        "WorkspaceDocument",
    ] {
        assert!(!request_json.contains(forbidden));
    }

    let success = execute_calculation_request(&DeterministicBackend, request.clone());
    assert!(matches!(success.outcome, CalculationOutcome::Success(_)));
    let success_json = serde_json::to_string(&success).expect("success serialization");
    let decoded_success: CalculationWorkerResult =
        serde_json::from_str(&success_json).expect("success deserialization");
    assert_eq!(success, decoded_success);

    let failure = CalculationWorkerResult {
        protocol_version: WorkerProtocolVersion::CURRENT,
        request_id: request.request_id,
        calc_key: request.calc_key,
        outcome: CalculationOutcome::Failure(CalculationWorkerFailure {
            category: CalculationWorkerFailureCategory::BackendFailure,
            message: "typed fixture failure".into(),
        }),
    };
    let failure_json = serde_json::to_string(&failure).expect("failure serialization");
    let decoded_failure: CalculationWorkerResult =
        serde_json::from_str(&failure_json).expect("failure deserialization");
    assert_eq!(failure, decoded_failure);
}

#[test]
fn incompatible_worker_protocol_is_rejected_explicitly() {
    assert_eq!(WorkerProtocolVersion::CURRENT.get(), 3);
    let mut request = worker_request();
    request.protocol_version = WorkerProtocolVersion::new(99);
    let result = execute_calculation_request(&DeterministicBackend, request);
    assert_eq!(result.protocol_version, WorkerProtocolVersion::CURRENT);
    assert!(matches!(
        result.outcome,
        CalculationOutcome::Failure(CalculationWorkerFailure {
            category: CalculationWorkerFailureCategory::ProtocolMismatch,
            ..
        })
    ));
}

#[test]
fn tropical_sidereal_and_ayanamsa_resolve_to_owned_semantics() {
    let (record, mut definition) = resources();
    definition.payload.calculation.zodiac = ZodiacSpec::Sidereal {
        ayanamsha: " lahiri ".into(),
    };
    let request = engine()
        .prepare(
            &definition,
            &record,
            &point_set(&["sun"]),
            &point_set(&["sun"]),
        )
        .expect("sidereal request")
        .request;
    assert_eq!(
        request.zodiac,
        mirabile_engine::ZodiacCalculationRequest::Sidereal {
            ayanamsa: mirabile_engine::AyanamsaConfiguration {
                id: "lahiri".into(),
                parameters: std::collections::BTreeMap::new(),
            }
        }
    );
    assert_eq!(request.houses.expect("houses").zodiac, request.zodiac);
}
