use std::collections::{BTreeMap, BTreeSet};

use mirabile_core::{
    Angle, AngleState, AngularVelocity, CoordinateSystem, CorrectionSpec, HouseSystem, PointId,
    PointState,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    BackendAuthoringCapabilities, BackendCalculationProvenance, BackendCapabilities,
    BackendDescriptor, BackendFingerprint, CalculationBackendResult, CelestialBackendFingerprint,
    CelestialCalculationProvenance, CelestialCapabilities, CelestialPositionsResult,
    DerivedPointsResult, EphemerisModelIdentity, EphemerisModelKind, HouseBackendFingerprint,
    HouseCalculationProvenance, HouseCalculationResult, HouseCapabilities, ImplementationIdentity,
    ResolvedCalculationRequest, ZodiacMode,
};

pub trait CalculationBackend {
    fn descriptor(&self) -> BackendDescriptor;

    fn calculate(
        &self,
        request: &ResolvedCalculationRequest,
    ) -> Result<CalculationBackendResult, CalculationBackendError>;
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendCapability {
    CelestialPositions,
    HousesAndAngles,
    DerivedPoints,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculationBackendErrorCategory {
    InvalidInput,
    UnsupportedCapability,
    ExecutionFailure,
    Internal,
}

#[derive(Clone, Debug, Deserialize, Eq, Error, PartialEq, Serialize)]
#[error("{message}")]
pub struct CalculationBackendError {
    pub category: CalculationBackendErrorCategory,
    pub capability: Option<BackendCapability>,
    pub message: String,
}

impl CalculationBackendError {
    fn unsupported(capability: BackendCapability, message: impl Into<String>) -> Self {
        Self {
            category: CalculationBackendErrorCategory::UnsupportedCapability,
            capability: Some(capability),
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self {
            category: CalculationBackendErrorCategory::InvalidInput,
            capability: None,
            message: message.into(),
        }
    }
}

/// Executable demo/test backend. It is not an astronomical authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeterministicBackend;

impl DeterministicBackend {
    pub const ID: &'static str = "mirabile-deterministic-demo";

    fn catalog() -> [(&'static str, f64, f64); 8] {
        [
            ("sun", 0.0, 0.985_647),
            ("moon", 6.5, 1.025),
            ("mercury", 60.0, 1.12),
            ("venus", 90.0, 0.96),
            ("mars", 120.0, 0.72),
            ("jupiter", 180.0, 0.083),
            ("saturn", 240.0, 0.034),
            ("uranus", 300.0, 0.012),
        ]
    }

    fn implementation(id: &str) -> ImplementationIdentity {
        ImplementationIdentity {
            id: id.into(),
            version: env!("CARGO_PKG_VERSION").into(),
            revision: Some("deterministic-fixture-r1".into()),
        }
    }

    fn model() -> EphemerisModelIdentity {
        EphemerisModelIdentity {
            kind: EphemerisModelKind::Analytic,
            id: "mirabile-demo-linear-model".into(),
            version: Some("1".into()),
            revision: None,
            data_fingerprint: Some("deterministic-catalog-v1".into()),
        }
    }

    fn supported_points() -> BTreeSet<PointId> {
        Self::catalog()
            .into_iter()
            .map(|(id, _, _)| PointId::new(id).expect("deterministic point identifiers are valid"))
            .collect()
    }
}

impl CalculationBackend for DeterministicBackend {
    fn descriptor(&self) -> BackendDescriptor {
        let backend = Self::implementation(Self::ID);
        let celestial = Self::implementation("mirabile-deterministic-celestial");
        let houses = Self::implementation("mirabile-deterministic-houses");
        BackendDescriptor {
            fingerprint: BackendFingerprint {
                backend,
                time: None,
                celestial: Some(CelestialBackendFingerprint {
                    implementation: celestial,
                    model: Some(Self::model()),
                }),
                houses: Some(HouseBackendFingerprint {
                    implementation: houses,
                }),
                derived: None,
            },
            capabilities: BackendCapabilities {
                celestial: Some(CelestialCapabilities {
                    supported_points: Self::supported_points(),
                }),
                houses: Some(HouseCapabilities {
                    supported_systems: vec![HouseSystem::Equal],
                }),
                derived: None,
            },
            authoring: BackendAuthoringCapabilities {
                supported_zodiac_modes: vec![ZodiacMode::Tropical],
                supported_coordinate_systems: vec![CoordinateSystem::Geocentric],
                default_corrections: CorrectionSpec::default(),
            },
        }
    }

    #[allow(clippy::too_many_lines)]
    fn calculate(
        &self,
        request: &ResolvedCalculationRequest,
    ) -> Result<CalculationBackendResult, CalculationBackendError> {
        let descriptor = self.descriptor();
        let celestial_capabilities =
            descriptor.capabilities.celestial.as_ref().ok_or_else(|| {
                CalculationBackendError::unsupported(
                    BackendCapability::CelestialPositions,
                    "deterministic backend has no celestial capability",
                )
            })?;
        for point in &request.celestial.requested_points {
            if !celestial_capabilities.supported_points.contains(point) {
                return Err(CalculationBackendError::unsupported(
                    BackendCapability::CelestialPositions,
                    format!("celestial point {point} is not supported by this backend"),
                ));
            }
        }
        if request.zodiac != crate::ZodiacCalculationRequest::Tropical {
            return Err(CalculationBackendError::unsupported(
                BackendCapability::CelestialPositions,
                "deterministic backend supports only tropical celestial calculations",
            ));
        }
        if request.celestial.coordinates != CoordinateSystem::Geocentric {
            return Err(CalculationBackendError::unsupported(
                BackendCapability::CelestialPositions,
                "deterministic backend supports only geocentric coordinates",
            ));
        }
        if request.celestial.corrections != CorrectionSpec::default() {
            return Err(CalculationBackendError::unsupported(
                BackendCapability::CelestialPositions,
                "deterministic backend does not implement correction flags",
            ));
        }
        if !request.derived.points.is_empty() {
            return Err(CalculationBackendError::unsupported(
                BackendCapability::DerivedPoints,
                "deterministic backend does not implement derived formulas",
            ));
        }
        if let Some(houses) = &request.houses {
            let capabilities = descriptor.capabilities.houses.as_ref().ok_or_else(|| {
                CalculationBackendError::unsupported(
                    BackendCapability::HousesAndAngles,
                    "deterministic backend has no houses capability",
                )
            })?;
            if !capabilities.supported_systems.contains(&houses.system) {
                return Err(CalculationBackendError::unsupported(
                    BackendCapability::HousesAndAngles,
                    format!("house system {:?} is not supported", houses.system),
                ));
            }
            if houses.zodiac != crate::ZodiacCalculationRequest::Tropical {
                return Err(CalculationBackendError::unsupported(
                    BackendCapability::HousesAndAngles,
                    "deterministic backend supports only tropical house calculations",
                ));
            }
        }

        let day = request.context.time.instant.julian_day() - 2_451_545.0;
        let catalog = Self::catalog()
            .into_iter()
            .map(|(id, phase, speed)| (id, (phase, speed)))
            .collect::<BTreeMap<_, _>>();
        let mut positions = BTreeMap::new();
        for point in &request.celestial.requested_points {
            let (phase, speed) = catalog
                .get(point.as_str())
                .copied()
                .ok_or_else(|| CalculationBackendError::invalid("catalog lookup failed"))?;
            let longitude = Angle::normalized(phase + day * speed)
                .map_err(|_| CalculationBackendError::invalid("non-finite celestial input"))?;
            positions.insert(
                point.clone(),
                PointState {
                    longitude,
                    latitude: Angle::from_degrees(0.0)
                        .map_err(|_| CalculationBackendError::invalid("non-finite latitude"))?,
                    declination: Angle::from_degrees(longitude.degrees().to_radians().sin() * 23.4)
                        .map_err(|_| CalculationBackendError::invalid("non-finite declination"))?,
                    right_ascension: longitude,
                    speed_longitude: AngularVelocity::degrees_per_day(speed).map_err(|_| {
                        CalculationBackendError::invalid("non-finite angular velocity")
                    })?,
                    retrograde: false,
                    azimuth: None,
                    altitude: None,
                },
            );
        }

        let house_result = request
            .houses
            .as_ref()
            .map(|_| {
                let location = request.context.location.as_ref().ok_or_else(|| {
                    CalculationBackendError::invalid(
                        "house calculation requires an observer location",
                    )
                })?;
                let ascendant = Angle::normalized(
                    request.context.time.instant.julian_day().fract() * 360.0
                        + location.longitude.degrees(),
                )
                .map_err(|_| CalculationBackendError::invalid("non-finite house input"))?;
                let midheaven = Angle::normalized(ascendant.degrees() + 90.0)
                    .map_err(|_| CalculationBackendError::invalid("non-finite house input"))?;
                let cusps = (0..12)
                    .map(|index| {
                        Angle::normalized(ascendant.degrees() + f64::from(index) * 30.0)
                            .map_err(|_| CalculationBackendError::invalid("non-finite cusp"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(HouseCalculationResult {
                    cusps,
                    angles: AngleState {
                        ascendant: Some(ascendant),
                        midheaven: Some(midheaven),
                    },
                })
            })
            .transpose()?;

        let fingerprint = descriptor.fingerprint;
        let celestial_fingerprint = fingerprint.celestial.ok_or_else(|| {
            CalculationBackendError::invalid("celestial fingerprint is unavailable")
        })?;
        let house_provenance = match (&request.houses, fingerprint.houses) {
            (Some(houses), Some(component)) => Some(HouseCalculationProvenance {
                implementation: component.implementation,
                system: houses.system,
                zodiac: houses.zodiac.clone(),
            }),
            (Some(_), None) => {
                return Err(CalculationBackendError::invalid(
                    "house fingerprint is unavailable",
                ));
            }
            (None, _) => None,
        };

        Ok(CalculationBackendResult {
            celestial: CelestialPositionsResult { positions },
            houses: house_result,
            derived: DerivedPointsResult::default(),
            provenance: BackendCalculationProvenance {
                backend: fingerprint.backend,
                time: None,
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
