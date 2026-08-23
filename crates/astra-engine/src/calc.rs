use std::collections::{BTreeMap, BTreeSet};

use astra_core::{
    AngleState, CalendarSpec, ChartDefinition, ChartRecord, ChartSource, HouseState, Offset,
    PointId, PointSelector, PointSet, PointState, ResolvedTime, ResourceEnvelope, ResourceError,
    ResourceRevisionRef, TimeZoneAssertion, ZodiacSpec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AstraCalculationProvenance, AyanamsaConfiguration, BackendDescriptor, CalcKey,
    CalculationBackendResult, CalculationContext, CalculationProvenance, CelestialPositionsRequest,
    DerivedFormula, DerivedPointRequest, DerivedPointsRequest, HouseCalculationRequest,
    ImplementationIdentity, KeyError, NumericLocation, ResolvedCalculationRequest,
    ZodiacCalculationRequest,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalculationValue {
    pub resolved_time: ResolvedTime,
    pub numeric_location: NumericLocation,
    pub celestial_positions: BTreeMap<PointId, PointState>,
    pub houses: Option<HouseState>,
    pub angles: AngleState,
    pub derived_points: BTreeMap<PointId, PointState>,
    pub provenance: CalculationProvenance,
}

impl CalculationValue {
    pub fn point(&self, id: &PointId) -> Option<&PointState> {
        self.celestial_positions
            .get(id)
            .or_else(|| self.derived_points.get(id))
    }

    pub fn point_entry(&self, id: &PointId) -> Option<(&PointId, &PointState)> {
        self.celestial_positions
            .get_key_value(id)
            .or_else(|| self.derived_points.get_key_value(id))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotContext {
    pub definition: ResourceRevisionRef,
    pub records: Vec<ResourceRevisionRef>,
    pub location_display_name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartSnapshot {
    pub calc_key: CalcKey,
    pub context: SnapshotContext,
    pub calculation: CalculationValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedCalculation {
    pub calc_key: CalcKey,
    pub request: ResolvedCalculationRequest,
    pub snapshot_context: SnapshotContext,
}

/// Astra orchestration: canonical inputs become provider-neutral execution semantics.
pub struct CalculationEngine {
    backend: BackendDescriptor,
    engine_identity: ImplementationIdentity,
    timezone_data_version: String,
}

impl CalculationEngine {
    pub fn new(
        backend: BackendDescriptor,
        calculation_engine: ImplementationIdentity,
        timezone_data_version: impl Into<String>,
    ) -> Self {
        Self {
            backend,
            engine_identity: calculation_engine,
            timezone_data_version: timezone_data_version.into(),
        }
    }

    pub fn backend_descriptor(&self) -> &BackendDescriptor {
        &self.backend
    }

    pub fn calculation_engine_identity(&self) -> &ImplementationIdentity {
        &self.engine_identity
    }

    pub fn prepare(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
        displayed_points: &PointSet,
        aspected_points: &PointSet,
    ) -> Result<PreparedCalculation, CalculationError> {
        validate_radix_inputs(definition, record)?;
        let calculation = &definition.payload.calculation;
        let context = CalculationContext {
            time: resolve_time(&record.payload, &self.timezone_data_version)?,
            location: NumericLocation {
                latitude: record.payload.location.latitude,
                longitude: record.payload.location.longitude,
            },
        };
        let zodiac = resolve_zodiac(&calculation.zodiac)?;
        let (requested_points, derived) = resolve_requested_points(
            displayed_points,
            aspected_points,
            calculation.fortune_formula,
        )?;
        let houses = (calculation.houses != astra_core::HouseSystem::NoHouses).then(|| {
            HouseCalculationRequest {
                system: calculation.houses,
                zodiac: zodiac.clone(),
            }
        });
        let request = ResolvedCalculationRequest {
            context,
            zodiac,
            celestial: CelestialPositionsRequest {
                requested_points,
                coordinates: calculation.coordinates,
                corrections: calculation.corrections.clone(),
                // Nodes and Black Moon stay with the celestial/model capability for now.
                // A future adapter can refine support without changing canonical resources.
                lunar_node: calculation.lunar_node,
                black_moon: calculation.black_moon,
            },
            houses,
            derived,
        };
        let calc_key = CalcKey::derive(&request, &self.engine_identity, &self.backend.fingerprint)?;
        Ok(PreparedCalculation {
            calc_key,
            request,
            snapshot_context: snapshot_context(definition, record),
        })
    }

    pub fn complete(
        &self,
        prepared: &PreparedCalculation,
        result: CalculationBackendResult,
    ) -> Result<CalculationValue, CalculationError> {
        validate_backend_result(&prepared.request, &self.backend, &result)?;
        let houses = result.houses.as_ref().map(|houses| HouseState {
            cusps: houses.cusps.clone(),
        });
        let angles = result.houses.as_ref().map_or(
            AngleState {
                ascendant: None,
                midheaven: None,
            },
            |houses| houses.angles.clone(),
        );
        let backend_provenance = result.provenance;
        Ok(CalculationValue {
            resolved_time: prepared.request.context.time.clone(),
            numeric_location: prepared.request.context.location.clone(),
            celestial_positions: result.celestial.positions,
            houses,
            angles,
            derived_points: result.derived.positions,
            provenance: CalculationProvenance {
                astra: AstraCalculationProvenance {
                    calculation_engine: self.engine_identity.clone(),
                    timezone_data_version: self.timezone_data_version.clone(),
                },
                backend: backend_provenance.backend,
                celestial: backend_provenance.celestial,
                houses: backend_provenance.houses,
                derived: backend_provenance.derived,
            },
        })
    }

    pub fn snapshot(
        prepared: &PreparedCalculation,
        calculation: CalculationValue,
    ) -> ChartSnapshot {
        ChartSnapshot {
            calc_key: prepared.calc_key.clone(),
            context: prepared.snapshot_context.clone(),
            calculation,
        }
    }
}

fn resolve_requested_points(
    displayed: &PointSet,
    aspected: &PointSet,
    fortune_formula: astra_core::FortuneFormula,
) -> Result<(Vec<PointId>, DerivedPointsRequest), CalculationError> {
    let mut celestial = BTreeSet::new();
    let mut derived = BTreeMap::new();
    for selector in displayed.points.iter().chain(&aspected.points) {
        let PointSelector::Point(point) = selector else {
            let PointSelector::Category(category) = selector else {
                unreachable!();
            };
            return Err(CalculationError::UnresolvedPointCategory(category.clone()));
        };
        if point.as_str() == "part_of_fortune" {
            derived.insert(
                point.clone(),
                DerivedPointRequest {
                    point: point.clone(),
                    formula: DerivedFormula::PartOfFortune {
                        formula: fortune_formula,
                    },
                },
            );
            celestial.insert(PointId::new("sun").expect("built-in point ID is valid"));
            celestial.insert(PointId::new("moon").expect("built-in point ID is valid"));
        } else {
            celestial.insert(point.clone());
        }
    }
    Ok((
        celestial.into_iter().collect(),
        DerivedPointsRequest {
            points: derived.into_values().collect(),
        },
    ))
}

fn resolve_zodiac(value: &ZodiacSpec) -> Result<ZodiacCalculationRequest, CalculationError> {
    match value {
        ZodiacSpec::Tropical => Ok(ZodiacCalculationRequest::Tropical),
        ZodiacSpec::Sidereal { ayanamsha } => {
            let id = ayanamsha.trim();
            if id.is_empty() {
                return Err(CalculationError::InvalidAyanamsa);
            }
            Ok(ZodiacCalculationRequest::Sidereal {
                ayanamsa: AyanamsaConfiguration {
                    id: id.into(),
                    parameters: BTreeMap::new(),
                },
            })
        }
    }
}

#[allow(clippy::too_many_lines)]
fn validate_backend_result(
    request: &ResolvedCalculationRequest,
    descriptor: &BackendDescriptor,
    result: &CalculationBackendResult,
) -> Result<(), CalculationError> {
    let expected_points = request
        .celestial
        .requested_points
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual_points = result
        .celestial
        .positions
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_points != actual_points {
        return Err(CalculationError::BackendResultMismatch(
            "celestial result point set did not match the request".into(),
        ));
    }
    let expected_derived = request
        .derived
        .points
        .iter()
        .map(|point| point.point.clone())
        .collect::<BTreeSet<_>>();
    let actual_derived = result
        .derived
        .positions
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if expected_derived != actual_derived {
        return Err(CalculationError::BackendResultMismatch(
            "derived result point set did not match the request".into(),
        ));
    }
    if request.houses.is_some() != result.houses.is_some() {
        return Err(CalculationError::BackendResultMismatch(
            "house result presence did not match the request".into(),
        ));
    }
    if descriptor.fingerprint.backend != result.provenance.backend {
        return Err(CalculationError::BackendResultMismatch(
            "backend result identity did not match the selected backend".into(),
        ));
    }
    let celestial = descriptor.fingerprint.celestial.as_ref().ok_or_else(|| {
        CalculationError::BackendResultMismatch("selected backend lacks celestial identity".into())
    })?;
    if celestial.implementation != result.provenance.celestial.implementation
        || celestial.model != result.provenance.celestial.model
        || request.celestial.coordinates != result.provenance.celestial.coordinates
        || request.celestial.corrections != result.provenance.celestial.corrections
        || request.zodiac != result.provenance.celestial.zodiac
        || request.celestial.lunar_node != result.provenance.celestial.lunar_node
        || request.celestial.black_moon != result.provenance.celestial.black_moon
    {
        return Err(CalculationError::BackendResultMismatch(
            "celestial provenance did not match the request and descriptor".into(),
        ));
    }
    match (&request.houses, &result.provenance.houses) {
        (Some(expected), Some(actual)) => {
            let component = descriptor.fingerprint.houses.as_ref().ok_or_else(|| {
                CalculationError::BackendResultMismatch(
                    "selected backend lacks house identity".into(),
                )
            })?;
            if component.implementation != actual.implementation
                || expected.system != actual.system
                || expected.zodiac != actual.zodiac
            {
                return Err(CalculationError::BackendResultMismatch(
                    "house provenance did not match the request and descriptor".into(),
                ));
            }
        }
        (None, None) => {}
        _ => {
            return Err(CalculationError::BackendResultMismatch(
                "house provenance presence did not match the request".into(),
            ));
        }
    }
    match (&request.derived.points[..], &result.provenance.derived) {
        ([], None) => {}
        (expected, Some(actual)) => {
            let component = descriptor.fingerprint.derived.as_ref().ok_or_else(|| {
                CalculationError::BackendResultMismatch(
                    "selected backend lacks derived identity".into(),
                )
            })?;
            let expected_formulas = expected
                .iter()
                .map(|point| (point.point.clone(), point.formula.clone()))
                .collect::<BTreeMap<_, _>>();
            let actual_formulas = actual
                .formulas
                .iter()
                .map(|point| (point.point.clone(), point.formula.clone()))
                .collect::<BTreeMap<_, _>>();
            if component.implementation != actual.implementation
                || expected_formulas != actual_formulas
            {
                return Err(CalculationError::BackendResultMismatch(
                    "derived provenance did not match the request and descriptor".into(),
                ));
            }
        }
        _ => {
            return Err(CalculationError::BackendResultMismatch(
                "derived provenance presence did not match the request".into(),
            ));
        }
    }
    Ok(())
}

fn snapshot_context(
    definition: &ResourceEnvelope<ChartDefinition>,
    record: &ResourceEnvelope<ChartRecord>,
) -> SnapshotContext {
    SnapshotContext {
        definition: ResourceRevisionRef {
            id: definition.id,
            revision: definition.revision,
        },
        records: vec![ResourceRevisionRef {
            id: record.id,
            revision: record.revision,
        }],
        location_display_name: record.payload.location.display_name.clone(),
    }
}

fn validate_radix_inputs(
    definition: &ResourceEnvelope<ChartDefinition>,
    record: &ResourceEnvelope<ChartRecord>,
) -> Result<(), CalculationError> {
    definition.validate()?;
    record.validate()?;
    match &definition.payload.source {
        ChartSource::Radix { record: expected } if *expected == record.id => Ok(()),
        ChartSource::Radix { record: expected } => Err(CalculationError::RecordMismatch {
            expected: *expected,
            actual: record.id,
        }),
        ChartSource::Derived { .. } => Err(CalculationError::DerivedChartNotImplemented),
    }
}

fn resolve_time(
    record: &ChartRecord,
    timezone_data_version: &str,
) -> Result<ResolvedTime, TimeResolutionError> {
    let offset = match &record.time.zone {
        TimeZoneAssertion::FixedOffset(value) => *value,
        TimeZoneAssertion::UniversalTime => Offset::UTC,
        TimeZoneAssertion::LocalMeanTime => {
            #[allow(clippy::cast_possible_truncation)]
            let seconds = (record.location.longitude.degrees() * 240.0).round() as i32;
            Offset::from_seconds(seconds).map_err(|_| TimeResolutionError::InvalidOffset)?
        }
        TimeZoneAssertion::NamedZone(name) => {
            return Err(TimeResolutionError::NamedZoneUnavailable(name.clone()));
        }
        TimeZoneAssertion::LocalApparentTime => {
            return Err(TimeResolutionError::LocalApparentTimeUnavailable);
        }
        TimeZoneAssertion::Unknown => return Err(TimeResolutionError::UnknownZone),
    };

    let date = record.time.civil_datetime.date;
    let time = record.time.civil_datetime.time;
    let mut year = date.year();
    let mut month = i32::from(date.month());
    if month <= 2 {
        year -= 1;
        month += 12;
    }
    let century = (f64::from(year) / 100.0).floor();
    let calendar_correction = match &record.time.calendar {
        CalendarSpec::ProlepticGregorian => 2.0 - century + (century / 4.0).floor(),
        CalendarSpec::Julian => 0.0,
        CalendarSpec::HistoricalTransition { identifier } => {
            return Err(TimeResolutionError::HistoricalCalendarUnavailable(
                identifier.clone(),
            ));
        }
    };
    let local_day_fraction = f64::from(time.seconds_since_midnight()) / 86_400.0;
    let utc_correction = f64::from(offset.seconds()) / 86_400.0;
    let julian_day = (365.25 * (f64::from(year) + 4_716.0)).floor()
        + (30.6001 * f64::from(month + 1)).floor()
        + f64::from(date.day())
        + calendar_correction
        - 1_524.5
        + local_day_fraction
        - utc_correction;

    Ok(ResolvedTime {
        instant: astra_core::AstroInstant::from_julian_day(julian_day)
            .map_err(|_| TimeResolutionError::NonFiniteInstant)?,
        applied_offset: offset,
        timezone_data_version: timezone_data_version.into(),
    })
}

#[derive(Debug, Error)]
pub enum CalculationError {
    #[error("chart definition references record {expected}, not supplied record {actual}")]
    RecordMismatch {
        expected: astra_core::ResourceId,
        actual: astra_core::ResourceId,
    },
    #[error("derived chart recipes are established but not calculated in this milestone")]
    DerivedChartNotImplemented,
    #[error("point category {0:?} must be resolved before calculation")]
    UnresolvedPointCategory(String),
    #[error("sidereal ayanamsa identity must not be empty")]
    InvalidAyanamsa,
    #[error("backend result integrity failure: {0}")]
    BackendResultMismatch(String),
    #[error(transparent)]
    InvalidResource(#[from] ResourceError),
    #[error(transparent)]
    Time(#[from] TimeResolutionError),
    #[error(transparent)]
    Key(#[from] KeyError),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum TimeResolutionError {
    #[error("named timezone {0} requires a historical timezone provider")]
    NamedZoneUnavailable(String),
    #[error("local apparent time resolution is not implemented")]
    LocalApparentTimeUnavailable,
    #[error("an unknown timezone cannot be resolved")]
    UnknownZone,
    #[error("historical calendar transition {0} is not implemented")]
    HistoricalCalendarUnavailable(String),
    #[error("resolved UTC offset is invalid")]
    InvalidOffset,
    #[error("resolved astronomical instant is not finite")]
    NonFiniteInstant,
}
