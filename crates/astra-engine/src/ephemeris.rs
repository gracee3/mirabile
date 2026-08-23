use std::collections::BTreeMap;

use astra_core::{
    Angle, AngleState, AngularVelocity, CalculationSpec, HouseState, PointId, PointState,
    ResolvedLocation, ResolvedTime,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderIdentity {
    pub name: String,
    pub version: String,
    pub data_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EphemerisRequest {
    pub time: ResolvedTime,
    pub location: ResolvedLocation,
    pub calculation: CalculationSpec,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EphemerisOutput {
    pub points: BTreeMap<PointId, PointState>,
    pub houses: Option<HouseState>,
    pub angles: AngleState,
}

pub trait EphemerisProvider {
    fn identity(&self) -> ProviderIdentity;

    fn calculate(&self, request: &EphemerisRequest) -> Result<EphemerisOutput, EphemerisError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeterministicEphemeris;

impl DeterministicEphemeris {
    pub const IDENTITY: &'static str = "astra-deterministic-demo";

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
}

impl EphemerisProvider for DeterministicEphemeris {
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity {
            name: Self::IDENTITY.into(),
            version: env!("CARGO_PKG_VERSION").into(),
            data_version: Some("fixture-v1".into()),
        }
    }

    fn calculate(&self, request: &EphemerisRequest) -> Result<EphemerisOutput, EphemerisError> {
        let day = request.time.instant.julian_day() - 2_451_545.0;
        let location_shift = request.location.longitude.degrees() * 0.02;
        let mut points = BTreeMap::new();

        for (id, phase, speed) in Self::catalog() {
            let longitude = Angle::normalized(phase + day * speed + location_shift)
                .map_err(|_| EphemerisError::NonFiniteInput)?;
            points.insert(
                PointId::new(id).map_err(|_| EphemerisError::InvalidCatalog)?,
                PointState {
                    longitude,
                    latitude: Angle::from_degrees(0.0)
                        .map_err(|_| EphemerisError::NonFiniteInput)?,
                    declination: Angle::from_degrees(
                        (longitude.degrees().to_radians().sin()) * 23.4,
                    )
                    .map_err(|_| EphemerisError::NonFiniteInput)?,
                    right_ascension: longitude,
                    speed_longitude: AngularVelocity::degrees_per_day(speed)
                        .map_err(|_| EphemerisError::NonFiniteInput)?,
                    retrograde: false,
                    azimuth: None,
                    altitude: None,
                },
            );
        }

        let ascendant = Angle::normalized(
            request.time.instant.julian_day().fract() * 360.0
                + request.location.longitude.degrees(),
        )
        .map_err(|_| EphemerisError::NonFiniteInput)?;
        let midheaven = Angle::normalized(ascendant.degrees() + 90.0)
            .map_err(|_| EphemerisError::NonFiniteInput)?;
        let houses = if request.calculation.houses == astra_core::HouseSystem::NoHouses {
            None
        } else {
            Some(HouseState {
                cusps: (0..12)
                    .map(|index| Angle::normalized(ascendant.degrees() + f64::from(index) * 30.0))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| EphemerisError::NonFiniteInput)?,
            })
        };

        Ok(EphemerisOutput {
            points,
            houses,
            angles: AngleState {
                ascendant: Some(ascendant),
                midheaven: Some(midheaven),
            },
        })
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum EphemerisError {
    #[error("ephemeris input was not finite")]
    NonFiniteInput,
    #[error("deterministic provider catalog is invalid")]
    InvalidCatalog,
    #[error("provider error: {0}")]
    Provider(String),
}
