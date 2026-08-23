#![cfg(feature = "xalen-backend")]

use std::collections::BTreeMap;

use mirabile_core::{
    BlackMoonType, CalculationSpec, CalendarSpec, ChartDefinition, ChartRecord, ChartSource,
    CivilDate, CivilDateTime, CivilTime, CoordinateSystem, CorrectionSpec, EventKind, HouseSystem,
    Latitude, LocationAssertion, Longitude, LunarNodeType, PointId, PointSelector, PointSet,
    ResourceEnvelope, ResourceId, SourceProvenance, SourceType, TemporalAssertion, TimeScale,
    TimeZoneAssertion, Timestamp, ZodiacSpec,
};
use mirabile_engine::{
    BackendCapability, CalcKey, CalculationBackend, CalculationBackendErrorCategory,
    CalculationEngine, CalculationOutcome, CalculationRequestId, CalculationWorkerRequest,
    DeterministicBackend, ImplementationIdentity, WorkerProtocolVersion, XalenBackend,
    execute_calculation_request,
};
use xalen_ephem::{Almanac, Body};
use xalen_houses::{GeoLocation, HouseSystem as XalenHouseSystem, compute_houses};
use xalen_time::{DeltaTModel, JdUTC, JulianDay as _};

const XALEN_REVISION: &str = "cc6edbec1f748ebdc4950ae6198f575c5ada73fa";
const J2000_UTC: f64 = 2_451_545.0;

fn resources() -> (
    ResourceEnvelope<ChartRecord>,
    ResourceEnvelope<ChartDefinition>,
) {
    let record_id = ResourceId::new();
    let record = ResourceEnvelope::with_id(
        record_id,
        "XALEN known-answer radix",
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
            location: LocationAssertion {
                display_name: "House oracle location".into(),
                country_region: Some("IN".into()),
                latitude: Latitude::from_degrees(28.0).expect("latitude"),
                longitude: Longitude::from_degrees(73.85).expect("longitude"),
                atlas_provenance: None,
            },
            source: SourceProvenance {
                description: "Pinned JPL Horizons and pyswisseph reference fixture".into(),
                source_type: SourceType::Research,
                recorded_by: None,
            },
            notes: Vec::new(),
            life_events: Vec::new(),
        },
        Timestamp::from_unix_millis(0),
    );
    let definition = ResourceEnvelope::new(
        "Tropical Placidus radix",
        ChartDefinition {
            source: ChartSource::Radix { record: record_id },
            calculation: CalculationSpec {
                zodiac: ZodiacSpec::Tropical,
                houses: HouseSystem::Placidus,
                coordinates: CoordinateSystem::Geocentric,
                lunar_node: LunarNodeType::True,
                black_moon: BlackMoonType::Mean,
                corrections: apparent_corrections(),
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

fn fixture_points() -> PointSet {
    point_set(&["sun", "moon", "mercury", "venus", "mars", "jupiter"])
}

fn apparent_corrections() -> CorrectionSpec {
    CorrectionSpec {
        aberration: true,
        light_time: true,
        nutation: true,
    }
}

fn engine_identity() -> ImplementationIdentity {
    ImplementationIdentity {
        id: "mirabile-calculation-engine".into(),
        version: "test-v1".into(),
        revision: Some("xalen-integration-r1".into()),
    }
}

fn engine() -> CalculationEngine {
    CalculationEngine::new(
        XalenBackend.descriptor(),
        engine_identity(),
        "fixture-tz-v1",
    )
}

fn angle_delta_degrees(actual: f64, expected: f64) -> f64 {
    let delta = (actual - expected).rem_euclid(360.0);
    delta.min(360.0 - delta)
}

#[test]
fn descriptor_is_narrow_stable_and_provider_neutral() {
    let descriptor = XalenBackend.descriptor();
    assert_eq!(descriptor.fingerprint.backend.id, "xalen");
    assert_eq!(descriptor.fingerprint.backend.version, "0.6.0");
    assert_eq!(
        descriptor.fingerprint.backend.revision.as_deref(),
        Some(XALEN_REVISION)
    );
    let time = descriptor
        .fingerprint
        .time
        .as_ref()
        .expect("time pipeline identity");
    assert_eq!(time.implementation.id, "xalen-time");
    assert_eq!(time.implementation.version, "0.6.0");
    assert_eq!(
        time.implementation.revision.as_deref(),
        Some(XALEN_REVISION)
    );
    assert_eq!(time.input_scale, TimeScale::Utc);
    assert_eq!(time.celestial_scale, TimeScale::Tt);
    assert_eq!(time.house_scale, Some(TimeScale::Ut1));
    assert_eq!(
        time.leap_second_model
            .as_ref()
            .expect("leap-second model")
            .id,
        "iers-leap-seconds-1972-2017"
    );
    assert_eq!(
        time.delta_t_model.as_ref().expect("Delta-T model").id,
        "stephenson-morrison-hohenkerk-2016"
    );
    let celestial = descriptor
        .fingerprint
        .celestial
        .as_ref()
        .expect("celestial identity");
    assert_eq!(celestial.implementation.id, "xalen-ephem");
    assert_eq!(celestial.implementation.version, "0.6.0");
    assert_eq!(
        celestial.implementation.revision.as_deref(),
        Some(XALEN_REVISION)
    );
    assert_eq!(
        celestial.model.as_ref().expect("analytical model").id,
        "xalen-vsop87a-elp2000-82-apparent"
    );
    let houses = &descriptor
        .fingerprint
        .houses
        .as_ref()
        .expect("house identity")
        .implementation;
    assert_eq!(houses.id, "xalen-houses");
    assert_eq!(houses.version, "0.6.0");
    assert_eq!(houses.revision.as_deref(), Some(XALEN_REVISION));
    assert!(descriptor.fingerprint.derived.is_none());
    assert!(descriptor.capabilities.derived.is_none());
    assert_eq!(
        descriptor
            .capabilities
            .houses
            .as_ref()
            .expect("houses")
            .supported_systems,
        vec![HouseSystem::Equal, HouseSystem::Placidus]
    );
    assert_eq!(
        descriptor
            .capabilities
            .celestial
            .as_ref()
            .expect("celestial")
            .supported_points,
        ["sun", "moon", "mercury", "venus", "mars", "jupiter"]
            .into_iter()
            .map(|id| PointId::new(id).expect("point ID"))
            .collect()
    );
}

#[test]
fn equal_houses_match_direct_pinned_xalen_call() {
    let (record, mut definition) = resources();
    definition.payload.calculation.houses = HouseSystem::Equal;
    let prepared = engine()
        .prepare(&definition, &record, &fixture_points(), &fixture_points())
        .expect("prepared Equal-house calculation");
    let actual = XalenBackend
        .calculate(&prepared.request)
        .expect("XALEN Equal-house calculation")
        .houses
        .expect("Equal houses");

    let jd_tt = JdUTC(J2000_UTC).to_tt();
    let jd_ut1 = jd_tt.to_ut1(&DeltaTModel::StephensonMorrisonHohenkerk2016);
    let epsilon = xalen_coords::mean_obliquity(jd_tt.julian_centuries_from_j2000());
    let direct = compute_houses(
        jd_ut1.as_f64(),
        &GeoLocation::new(28.0, 73.85),
        epsilon,
        XalenHouseSystem::Equal,
    );
    for (actual, expected) in actual.cusps.iter().zip(direct.cusps) {
        assert!(angle_delta_degrees(actual.degrees(), expected.to_degrees()) < 1e-12);
    }
    assert!(
        angle_delta_degrees(
            actual.angles.ascendant.expect("ascendant").degrees(),
            direct.ascendant.to_degrees()
        ) < 1e-12
    );
    assert!(
        angle_delta_degrees(
            actual.angles.midheaven.expect("midheaven").degrees(),
            direct.mc.to_degrees()
        ) < 1e-12
    );
}

#[test]
fn adapter_matches_direct_pinned_xalen_calls() {
    let (record, definition) = resources();
    let prepared = engine()
        .prepare(&definition, &record, &fixture_points(), &fixture_points())
        .expect("prepared calculation");
    let actual = XalenBackend
        .calculate(&prepared.request)
        .expect("XALEN calculation");

    let jd_tt = JdUTC(J2000_UTC).to_tt();
    let almanac = Almanac::default_vedic();
    for (id, body) in [
        ("sun", Body::Sun),
        ("moon", Body::Moon),
        ("mercury", Body::Mercury),
        ("venus", Body::Venus),
        ("mars", Body::Mars),
        ("jupiter", Body::Jupiter),
    ] {
        let point = PointId::new(id).expect("point ID");
        let state = actual.celestial.positions.get(&point).expect("position");
        let ecliptic = almanac
            .geocentric_ecliptic_tt(body, jd_tt)
            .expect("direct ecliptic position");
        let equatorial = almanac
            .geocentric_equatorial_tt(body, jd_tt)
            .expect("direct equatorial position");
        let speed = almanac
            .geocentric_speed_tt(body, jd_tt)
            .expect("direct speed");
        assert!(angle_delta_degrees(state.longitude.degrees(), ecliptic.longitude_deg()) < 1e-12);
        assert!((state.latitude.degrees() - ecliptic.latitude_deg()).abs() < 1e-12);
        assert!((state.declination.degrees() - equatorial.dec_deg()).abs() < 1e-12);
        assert!(
            angle_delta_degrees(
                state.right_ascension.degrees(),
                equatorial.right_ascension.to_degrees()
            ) < 1e-12
        );
        assert!(
            (state.speed_longitude.as_degrees_per_day() - speed.longitude_deg_per_day()).abs()
                < 1e-12
        );
        assert_eq!(state.retrograde, speed.is_retrograde());
    }

    let jd_ut1 = jd_tt.to_ut1(&DeltaTModel::StephensonMorrisonHohenkerk2016);
    let epsilon = xalen_coords::mean_obliquity(jd_tt.julian_centuries_from_j2000());
    let direct_houses = compute_houses(
        jd_ut1.as_f64(),
        &GeoLocation::new(28.0, 73.85),
        epsilon,
        XalenHouseSystem::Placidus,
    );
    let houses = actual.houses.as_ref().expect("house result");
    for (actual, expected) in houses.cusps.iter().zip(direct_houses.cusps) {
        assert!(angle_delta_degrees(actual.degrees(), expected.to_degrees()) < 1e-12);
    }
    assert!(
        angle_delta_degrees(
            houses.angles.ascendant.expect("ascendant").degrees(),
            direct_houses.ascendant.to_degrees()
        ) < 1e-12
    );
    assert!(
        angle_delta_degrees(
            houses.angles.midheaven.expect("midheaven").degrees(),
            direct_houses.mc.to_degrees()
        ) < 1e-12
    );
}

#[test]
fn independent_jpl_and_swiss_known_answer_radix() {
    let (record, definition) = resources();
    let calculation_engine = engine();
    let prepared = calculation_engine
        .prepare(&definition, &record, &fixture_points(), &fixture_points())
        .expect("prepared calculation");
    assert_eq!(prepared.request.context.time.scale, TimeScale::Utc);
    assert!((prepared.request.context.time.instant.julian_day() - J2000_UTC).abs() < 1e-12);
    let backend_result = XalenBackend
        .calculate(&prepared.request)
        .expect("backend calculation");
    let calculation = calculation_engine
        .complete(&prepared, backend_result)
        .expect("validated calculation");
    let snapshot = CalculationEngine::snapshot(&prepared, calculation);

    // NASA/JPL Horizons DE440 quantity 31, apparent geocentric
    // ecliptic-of-date longitude, 2000-01-01 12:00 UT. Values and tolerances
    // are the independently sourced oracle committed in pinned XALEN
    // `crates/xalen-ephem/tests/swiss_eph_crossval.rs`.
    for (id, expected, tolerance) in [
        ("sun", 280.3689, 0.001),
        ("moon", 223.3238, 0.006),
        ("mercury", 271.8893, 0.001),
        ("venus", 241.5658, 0.001),
        ("mars", 327.9633, 0.001),
        ("jupiter", 25.2531, 0.001),
    ] {
        let actual = snapshot
            .calculation
            .celestial_positions
            .get(&PointId::new(id).expect("point ID"))
            .expect("known-answer position")
            .longitude
            .degrees();
        assert!(
            angle_delta_degrees(actual, expected) <= tolerance,
            "{id}: expected {expected} degrees within {tolerance}, got {actual}"
        );
    }

    // pyswisseph 2.10.03 houses_armc oracle at JD(UT1) 2451545.0,
    // latitude 28.0 N, longitude 73.85 E, tropical Placidus, using XALEN's
    // RAMC and IAU 2006 mean obliquity. The 0.01 degree bound is XALEN's
    // documented tight house-algorithm tolerance and also covers the subsecond
    // modeled UT1 offset derived from this fixture's UTC instant.
    let expected_cusps = [
        96.907_359,
        119.672_032,
        144.432_212,
        173.802_738,
        208.270_696,
        244.138_944,
        276.907_359,
        299.672_032,
        324.432_212,
        353.802_738,
        28.270_696,
        64.138_944,
    ];
    let actual_cusps = &snapshot
        .calculation
        .houses
        .as_ref()
        .expect("Placidus houses")
        .cusps;
    for (index, (actual, expected)) in actual_cusps.iter().zip(expected_cusps).enumerate() {
        assert!(
            angle_delta_degrees(actual.degrees(), expected) <= 0.01,
            "cusp {}: expected {expected} degrees within 0.01, got {}",
            index + 1,
            actual.degrees()
        );
    }
    assert!(
        angle_delta_degrees(
            snapshot
                .calculation
                .angles
                .ascendant
                .expect("ascendant")
                .degrees(),
            96.907_359
        ) <= 0.01
    );
    assert!(
        angle_delta_degrees(
            snapshot
                .calculation
                .angles
                .midheaven
                .expect("midheaven")
                .degrees(),
            353.802_738
        ) <= 0.01
    );

    assert_eq!(snapshot.calculation.provenance.backend.id, "xalen");
    let time = snapshot
        .calculation
        .provenance
        .time
        .as_ref()
        .expect("time conversion provenance");
    assert_eq!(time.implementation.id, "xalen-time");
    assert_eq!(time.input_scale, TimeScale::Utc);
    assert_eq!(time.celestial_scale, TimeScale::Tt);
    assert_eq!(time.house_scale, Some(TimeScale::Ut1));
    assert_eq!(
        time.delta_t_model.as_ref().expect("Delta-T model").id,
        "stephenson-morrison-hohenkerk-2016"
    );
}

#[test]
fn unsupported_semantics_are_typed_failures() {
    let (record, definition) = resources();
    let prepared = engine()
        .prepare(&definition, &record, &fixture_points(), &fixture_points())
        .expect("prepared calculation");

    let mut unknown = prepared.request.clone();
    unknown
        .celestial
        .requested_points
        .push(PointId::new("saturn").expect("point ID"));
    let error = XalenBackend
        .calculate(&unknown)
        .expect_err("unsupported body");
    assert_eq!(
        error.category,
        CalculationBackendErrorCategory::UnsupportedCapability
    );
    assert_eq!(
        error.capability,
        Some(BackendCapability::CelestialPositions)
    );

    let mut geometric = prepared.request.clone();
    geometric.celestial.corrections = CorrectionSpec::default();
    assert_eq!(
        XalenBackend
            .calculate(&geometric)
            .expect_err("unsupported correction mode")
            .category,
        CalculationBackendErrorCategory::UnsupportedCapability
    );

    let mut topocentric = prepared.request.clone();
    topocentric.celestial.coordinates = CoordinateSystem::Topocentric;
    assert_eq!(
        XalenBackend
            .calculate(&topocentric)
            .expect_err("unsupported coordinates")
            .category,
        CalculationBackendErrorCategory::UnsupportedCapability
    );

    let mut sidereal = prepared.request.clone();
    sidereal.zodiac = mirabile_engine::ZodiacCalculationRequest::Sidereal {
        ayanamsa: mirabile_engine::AyanamsaConfiguration {
            id: "lahiri".into(),
            parameters: BTreeMap::default(),
        },
    };
    assert_eq!(
        XalenBackend
            .calculate(&sidereal)
            .expect_err("unsupported zodiac")
            .category,
        CalculationBackendErrorCategory::UnsupportedCapability
    );
}

#[test]
fn calc_keys_include_backend_revision_and_time_identity() {
    let (record, definition) = resources();
    let prepared = engine()
        .prepare(&definition, &record, &fixture_points(), &fixture_points())
        .expect("prepared calculation");
    let deterministic = CalcKey::derive(
        &prepared.request,
        &engine_identity(),
        &DeterministicBackend.descriptor().fingerprint,
    )
    .expect("deterministic key");
    assert_ne!(prepared.calc_key, deterministic);

    let mut other_revision = XalenBackend.descriptor().fingerprint;
    other_revision.backend.revision = Some("constructed-other-revision".into());
    let revised = CalcKey::derive(&prepared.request, &engine_identity(), &other_revision)
        .expect("revised XALEN key");
    assert_ne!(prepared.calc_key, revised);

    let assert_time_fingerprint_changes_key =
        |fingerprint: &mirabile_engine::BackendFingerprint| {
            assert_ne!(
                prepared.calc_key,
                CalcKey::derive(&prepared.request, &engine_identity(), fingerprint)
                    .expect("changed time fingerprint key")
            );
        };

    let mut time_implementation = XalenBackend.descriptor().fingerprint;
    time_implementation
        .time
        .as_mut()
        .expect("time fingerprint")
        .implementation
        .revision = Some("constructed-xalen-time-r2".into());
    assert_time_fingerprint_changes_key(&time_implementation);

    let mut leap_seconds = XalenBackend.descriptor().fingerprint;
    leap_seconds
        .time
        .as_mut()
        .expect("time fingerprint")
        .leap_second_model
        .as_mut()
        .expect("leap-second model")
        .version = Some("constructed-leap-table-r2".into());
    assert_time_fingerprint_changes_key(&leap_seconds);

    let mut delta_t = XalenBackend.descriptor().fingerprint;
    delta_t
        .time
        .as_mut()
        .expect("time fingerprint")
        .delta_t_model
        .as_mut()
        .expect("Delta-T model")
        .version = Some("constructed-delta-t-r2".into());
    assert_time_fingerprint_changes_key(&delta_t);
}

#[test]
fn completion_rejects_time_provenance_that_differs_from_the_fingerprint() {
    let (record, definition) = resources();
    let calculation_engine = engine();
    let prepared = calculation_engine
        .prepare(&definition, &record, &fixture_points(), &fixture_points())
        .expect("prepared calculation");
    let baseline = XalenBackend
        .calculate(&prepared.request)
        .expect("XALEN calculation");
    calculation_engine
        .complete(&prepared, baseline.clone())
        .expect("matching time provenance");

    let assert_rejected = |result| {
        assert!(matches!(
            calculation_engine.complete(&prepared, result),
            Err(mirabile_engine::CalculationError::BackendResultMismatch(_))
        ));
    };

    let mut implementation = baseline.clone();
    implementation
        .provenance
        .time
        .as_mut()
        .expect("time provenance")
        .implementation
        .revision = Some("constructed-xalen-time-r2".into());
    assert_rejected(implementation);

    let mut leap_seconds = baseline.clone();
    leap_seconds
        .provenance
        .time
        .as_mut()
        .expect("time provenance")
        .leap_second_model
        .as_mut()
        .expect("leap-second model")
        .version = Some("constructed-leap-table-r2".into());
    assert_rejected(leap_seconds);

    let mut delta_t = baseline.clone();
    delta_t
        .provenance
        .time
        .as_mut()
        .expect("time provenance")
        .delta_t_model
        .as_mut()
        .expect("Delta-T model")
        .version = Some("constructed-delta-t-r2".into());
    assert_rejected(delta_t);

    let mut absent = baseline;
    absent.provenance.time = None;
    assert_rejected(absent);

    let (record, mut no_houses_definition) = resources();
    no_houses_definition.payload.calculation.houses = HouseSystem::NoHouses;
    let no_houses = calculation_engine
        .prepare(
            &no_houses_definition,
            &record,
            &fixture_points(),
            &fixture_points(),
        )
        .expect("prepared calculation without houses");
    let no_houses_result = XalenBackend
        .calculate(&no_houses.request)
        .expect("XALEN calculation without houses");
    let time = no_houses_result
        .provenance
        .time
        .as_ref()
        .expect("celestial time provenance");
    assert_eq!(time.house_scale, None);
    assert_eq!(time.delta_t_model, None);
    calculation_engine
        .complete(&no_houses, no_houses_result)
        .expect("configured but unused house time model validates");
}

#[test]
fn worker_protocol_round_trip_contains_only_mirabile_types() {
    let (record, definition) = resources();
    let prepared = engine()
        .prepare(&definition, &record, &fixture_points(), &fixture_points())
        .expect("prepared calculation");
    let request = CalculationWorkerRequest {
        protocol_version: WorkerProtocolVersion::CURRENT,
        request_id: CalculationRequestId::new(7).expect("request ID"),
        calc_key: prepared.calc_key,
        backend: XalenBackend.descriptor().fingerprint,
        request: prepared.request,
    };
    let encoded_request = serde_json::to_string(&request).expect("worker request JSON");
    let decoded_request: CalculationWorkerRequest =
        serde_json::from_str(&encoded_request).expect("worker request round trip");
    let result = execute_calculation_request(&XalenBackend, decoded_request);
    let CalculationOutcome::Success(value) = &result.outcome else {
        panic!("XALEN worker calculation failed: {:?}", result.outcome);
    };
    assert_eq!(value.provenance.backend.id, "xalen");
    let encoded_result = serde_json::to_string(&result).expect("worker result JSON");
    assert!(!encoded_request.contains("xalen_ephem"));
    assert!(!encoded_result.contains("xalen_ephem"));
    let _: mirabile_engine::CalculationWorkerResult =
        serde_json::from_str(&encoded_result).expect("worker result round trip");
}
