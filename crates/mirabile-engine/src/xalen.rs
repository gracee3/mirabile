use std::collections::{BTreeMap, BTreeSet};

use mirabile_core::{
    Angle, AngleState, AngularVelocity, CoordinateSystem, CorrectionSpec, HouseSystem, PointId,
    PointState, TimeScale,
};
use xalen_ephem::{Almanac, Body};
use xalen_houses::{GeoLocation, HouseSystem as XalenHouseSystem, compute_houses};
use xalen_time::{DeltaTModel, FIRST_LEAP_SECOND_JD, JdUTC, JulianDay as _};

use crate::{
    BackendCalculationProvenance, BackendCapabilities, BackendCapability, BackendDescriptor,
    BackendFingerprint, CalculationBackend, CalculationBackendError,
    CalculationBackendErrorCategory, CalculationBackendResult, CelestialBackendFingerprint,
    CelestialCalculationProvenance, CelestialCapabilities, CelestialPositionsResult,
    DerivedPointsResult, EphemerisModelIdentity, EphemerisModelKind, HouseBackendFingerprint,
    HouseCalculationProvenance, HouseCalculationResult, HouseCapabilities, ImplementationIdentity,
    ResolvedCalculationRequest, TimeBackendFingerprint, TimeModelIdentity,
    TimeScaleConversionProvenance, ZodiacCalculationRequest,
};

const XALEN_VERSION: &str = "0.6.0";
const XALEN_REVISION: &str = "cc6edbec1f748ebdc4950ae6198f575c5ada73fa";

/// Offline XALEN analytical calculation adapter.
///
/// XALEN types remain private to this module and are converted to Mirabile-owned
/// request/result types before the calculation boundary returns.
#[derive(Clone, Copy, Debug, Default)]
pub struct XalenBackend;

impl XalenBackend {
    pub const ID: &'static str = "xalen";
    pub const VERSION: &'static str = XALEN_VERSION;
    pub const REVISION: &'static str = XALEN_REVISION;

    fn implementation(id: &str) -> ImplementationIdentity {
        ImplementationIdentity {
            id: id.into(),
            version: XALEN_VERSION.into(),
            revision: Some(XALEN_REVISION.into()),
        }
    }

    fn model() -> EphemerisModelIdentity {
        EphemerisModelIdentity {
            kind: EphemerisModelKind::Analytic,
            id: "xalen-vsop87a-elp2000-82-apparent".into(),
            version: Some(XALEN_VERSION.into()),
            revision: Some(XALEN_REVISION.into()),
            data_fingerprint: None,
        }
    }

    fn supported_points() -> BTreeSet<PointId> {
        ["sun", "moon", "mercury", "venus", "mars", "jupiter"]
            .into_iter()
            .map(|id| PointId::new(id).expect("XALEN adapter point IDs are valid"))
            .collect()
    }

    fn apparent_corrections() -> CorrectionSpec {
        CorrectionSpec {
            aberration: true,
            light_time: true,
            nutation: true,
        }
    }

    fn body(point: &PointId) -> Option<Body> {
        match point.as_str() {
            "sun" => Some(Body::Sun),
            "moon" => Some(Body::Moon),
            "mercury" => Some(Body::Mercury),
            "venus" => Some(Body::Venus),
            "mars" => Some(Body::Mars),
            "jupiter" => Some(Body::Jupiter),
            _ => None,
        }
    }

    fn house_system(system: HouseSystem) -> Option<XalenHouseSystem> {
        match system {
            HouseSystem::Equal => Some(XalenHouseSystem::Equal),
            HouseSystem::Placidus => Some(XalenHouseSystem::Placidus),
            HouseSystem::WholeSign | HouseSystem::NoHouses => None,
        }
    }

    fn time_fingerprint() -> TimeBackendFingerprint {
        TimeBackendFingerprint {
            implementation: Self::implementation("xalen-time"),
            input_scale: TimeScale::Utc,
            celestial_scale: TimeScale::Tt,
            house_scale: Some(TimeScale::Ut1),
            leap_second_model: Some(TimeModelIdentity {
                id: "iers-leap-seconds-1972-2017".into(),
                version: Some("2017-01-01".into()),
                revision: None,
            }),
            delta_t_model: Some(TimeModelIdentity {
                id: "stephenson-morrison-hohenkerk-2016".into(),
                version: Some("table-s15".into()),
                revision: None,
            }),
        }
    }

    fn time_provenance(
        fingerprint: TimeBackendFingerprint,
        houses: bool,
    ) -> TimeScaleConversionProvenance {
        TimeScaleConversionProvenance {
            implementation: fingerprint.implementation,
            input_scale: fingerprint.input_scale,
            celestial_scale: fingerprint.celestial_scale,
            house_scale: houses.then_some(fingerprint.house_scale).flatten(),
            leap_second_model: fingerprint.leap_second_model,
            delta_t_model: houses.then_some(fingerprint.delta_t_model).flatten(),
        }
    }
}

impl CalculationBackend for XalenBackend {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            fingerprint: BackendFingerprint {
                backend: Self::implementation(Self::ID),
                time: Some(Self::time_fingerprint()),
                celestial: Some(CelestialBackendFingerprint {
                    implementation: Self::implementation("xalen-ephem"),
                    model: Some(Self::model()),
                }),
                houses: Some(HouseBackendFingerprint {
                    implementation: Self::implementation("xalen-houses"),
                }),
                derived: None,
            },
            capabilities: BackendCapabilities {
                celestial: Some(CelestialCapabilities {
                    supported_points: Self::supported_points(),
                }),
                houses: Some(HouseCapabilities {
                    supported_systems: vec![HouseSystem::Equal, HouseSystem::Placidus],
                }),
                derived: None,
            },
        }
    }

    #[allow(clippy::too_many_lines)]
    fn calculate(
        &self,
        request: &ResolvedCalculationRequest,
    ) -> Result<CalculationBackendResult, CalculationBackendError> {
        validate_request(request)?;

        let jd_utc_value = request.context.time.instant.julian_day();
        let jd_tt = JdUTC(jd_utc_value).to_tt();
        let almanac = Almanac::default_vedic();
        let mut positions = BTreeMap::new();
        for point in &request.celestial.requested_points {
            let body = Self::body(point).ok_or_else(|| {
                unsupported(
                    BackendCapability::CelestialPositions,
                    format!("celestial point {point} is not supported by the XALEN backend"),
                )
            })?;
            let ecliptic = almanac
                .geocentric_ecliptic_tt(body, jd_tt)
                .map_err(|error| execution(format!("XALEN {body} position failed: {error}")))?;
            let equatorial = almanac
                .geocentric_equatorial_tt(body, jd_tt)
                .map_err(|error| {
                    execution(format!(
                        "XALEN {body} equatorial conversion failed: {error}"
                    ))
                })?;
            let speed = almanac
                .geocentric_speed_tt(body, jd_tt)
                .map_err(|error| execution(format!("XALEN {body} speed failed: {error}")))?;
            let speed_longitude = speed.longitude_deg_per_day();
            positions.insert(
                point.clone(),
                PointState {
                    longitude: normalized_angle(ecliptic.longitude_deg(), "longitude")?,
                    latitude: angle(ecliptic.latitude_deg(), "latitude")?,
                    declination: angle(equatorial.dec_deg(), "declination")?,
                    right_ascension: normalized_angle(
                        equatorial.right_ascension.to_degrees(),
                        "right ascension",
                    )?,
                    speed_longitude: AngularVelocity::degrees_per_day(speed_longitude)
                        .map_err(|_| invalid("XALEN returned a non-finite longitude speed"))?,
                    retrograde: speed_longitude < 0.0,
                    azimuth: None,
                    altitude: None,
                },
            );
        }

        let house_result = request
            .houses
            .as_ref()
            .map(|houses| calculate_houses(request, houses.system, jd_tt))
            .transpose()?;

        let descriptor = self.descriptor();
        let fingerprint = descriptor.fingerprint;
        let time_fingerprint = fingerprint
            .time
            .ok_or_else(|| internal("XALEN time fingerprint is unavailable"))?;
        let celestial_fingerprint = fingerprint
            .celestial
            .ok_or_else(|| internal("XALEN celestial fingerprint is unavailable"))?;
        let house_provenance = match (&request.houses, fingerprint.houses) {
            (Some(houses), Some(component)) => Some(HouseCalculationProvenance {
                implementation: component.implementation,
                system: houses.system,
                zodiac: houses.zodiac.clone(),
            }),
            (Some(_), None) => return Err(internal("XALEN house fingerprint is unavailable")),
            (None, _) => None,
        };

        Ok(CalculationBackendResult {
            celestial: CelestialPositionsResult { positions },
            houses: house_result,
            derived: DerivedPointsResult::default(),
            provenance: BackendCalculationProvenance {
                backend: fingerprint.backend,
                time: Some(Self::time_provenance(
                    time_fingerprint,
                    request.houses.is_some(),
                )),
                celestial: CelestialCalculationProvenance {
                    implementation: celestial_fingerprint.implementation,
                    model: celestial_fingerprint.model,
                    coordinates: request.celestial.coordinates,
                    corrections: request.celestial.corrections.clone(),
                    zodiac: request.zodiac.clone(),
                    lunar_node: request.celestial.lunar_node,
                    black_moon: request.celestial.black_moon,
                },
                houses: house_provenance,
                derived: None,
            },
        })
    }
}

fn validate_request(request: &ResolvedCalculationRequest) -> Result<(), CalculationBackendError> {
    for point in &request.celestial.requested_points {
        if XalenBackend::body(point).is_none() {
            return Err(unsupported(
                BackendCapability::CelestialPositions,
                format!("celestial point {point} is not supported by the XALEN backend"),
            ));
        }
    }
    if request.context.time.scale != TimeScale::Utc {
        return Err(unsupported(
            BackendCapability::CelestialPositions,
            "XALEN backend currently accepts only explicitly labelled UTC instants",
        ));
    }
    if request.context.time.instant.julian_day() < FIRST_LEAP_SECOND_JD {
        return Err(unsupported(
            BackendCapability::CelestialPositions,
            "XALEN backend requires post-1972 UTC because pre-1972 UTC rubber seconds are not modelled",
        ));
    }
    if request.zodiac != ZodiacCalculationRequest::Tropical {
        return Err(unsupported(
            BackendCapability::CelestialPositions,
            "XALEN backend supports only tropical celestial calculations in this slice",
        ));
    }
    if request.celestial.coordinates != CoordinateSystem::Geocentric {
        return Err(unsupported(
            BackendCapability::CelestialPositions,
            "XALEN backend supports only geocentric coordinates in this slice",
        ));
    }
    if request.celestial.corrections != XalenBackend::apparent_corrections() {
        return Err(unsupported(
            BackendCapability::CelestialPositions,
            "XALEN analytical API maps only to apparent places with aberration, light-time, and nutation enabled",
        ));
    }
    if !request.derived.points.is_empty() {
        return Err(unsupported(
            BackendCapability::DerivedPoints,
            "XALEN backend does not implement derived formulas",
        ));
    }
    if let Some(houses) = &request.houses {
        if houses.zodiac != ZodiacCalculationRequest::Tropical {
            return Err(unsupported(
                BackendCapability::HousesAndAngles,
                "XALEN backend supports only tropical houses in this slice",
            ));
        }
        if XalenBackend::house_system(houses.system).is_none() {
            return Err(unsupported(
                BackendCapability::HousesAndAngles,
                format!(
                    "house system {:?} is not supported by the XALEN backend",
                    houses.system
                ),
            ));
        }
        if houses.system == HouseSystem::Placidus
            && request.context.location.latitude.degrees().abs() > 66.5
        {
            return Err(unsupported(
                BackendCapability::HousesAndAngles,
                "XALEN Placidus is unsupported above 66.5 degrees latitude; no silent Porphyry fallback is accepted",
            ));
        }
    }
    Ok(())
}

fn calculate_houses(
    request: &ResolvedCalculationRequest,
    system: HouseSystem,
    jd_tt: xalen_time::JdTT,
) -> Result<HouseCalculationResult, CalculationBackendError> {
    let xalen_system = XalenBackend::house_system(system).ok_or_else(|| {
        unsupported(
            BackendCapability::HousesAndAngles,
            format!("house system {system:?} is not supported by the XALEN backend"),
        )
    })?;
    let delta_t_model = DeltaTModel::StephensonMorrisonHohenkerk2016;
    let jd_ut1 = jd_tt.to_ut1(&delta_t_model);
    let location = GeoLocation::try_new(
        request.context.location.latitude.degrees(),
        request.context.location.longitude.degrees(),
    )
    .ok_or_else(|| invalid("Mirabile location could not be represented by XALEN"))?;
    let epsilon = xalen_coords::mean_obliquity(jd_tt.julian_centuries_from_j2000());
    let houses = compute_houses(jd_ut1.as_f64(), &location, epsilon, xalen_system);
    if houses.fallback_used {
        return Err(execution(
            "XALEN house calculation used an unrequested polar fallback",
        ));
    }
    let cusps = houses
        .cusps
        .into_iter()
        .map(|value| normalized_angle(value.to_degrees(), "house cusp"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HouseCalculationResult {
        cusps,
        angles: AngleState {
            ascendant: Some(normalized_angle(
                houses.ascendant.to_degrees(),
                "ascendant",
            )?),
            midheaven: Some(normalized_angle(houses.mc.to_degrees(), "midheaven")?),
        },
    })
}

fn angle(value: f64, label: &str) -> Result<Angle, CalculationBackendError> {
    Angle::from_degrees(value).map_err(|_| invalid(format!("XALEN returned a non-finite {label}")))
}

fn normalized_angle(value: f64, label: &str) -> Result<Angle, CalculationBackendError> {
    Angle::normalized(value).map_err(|_| invalid(format!("XALEN returned a non-finite {label}")))
}

fn unsupported(
    capability: BackendCapability,
    message: impl Into<String>,
) -> CalculationBackendError {
    CalculationBackendError {
        category: CalculationBackendErrorCategory::UnsupportedCapability,
        capability: Some(capability),
        message: message.into(),
    }
}

fn invalid(message: impl Into<String>) -> CalculationBackendError {
    CalculationBackendError {
        category: CalculationBackendErrorCategory::InvalidInput,
        capability: None,
        message: message.into(),
    }
}

fn execution(message: impl Into<String>) -> CalculationBackendError {
    CalculationBackendError {
        category: CalculationBackendErrorCategory::ExecutionFailure,
        capability: None,
        message: message.into(),
    }
}

fn internal(message: impl Into<String>) -> CalculationBackendError {
    CalculationBackendError {
        category: CalculationBackendErrorCategory::Internal,
        capability: None,
        message: message.into(),
    }
}
