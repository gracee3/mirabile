use std::collections::BTreeMap;

use astra_core::{
    AngleState, CalendarSpec, ChartDefinition, ChartRecord, ChartSource, HouseState, LocationRole,
    Offset, PointId, PointState, ResolvedLocation, ResolvedTime, ResourceEnvelope,
    ResourceRevisionRef, TimeZoneAssertion,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CalcKey, EphemerisError, EphemerisProvider, EphemerisRequest, KeyError, ProviderIdentity,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CalculationProvenance {
    pub engine_version: String,
    pub ephemeris_provider: String,
    pub ephemeris_provider_version: String,
    pub ephemeris_data_version: Option<String>,
    pub timezone_database_version: Option<String>,
    pub calculation_profile_revision: Option<astra_core::Revision>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChartSnapshot {
    pub definition: ResourceRevisionRef,
    pub calc_key: CalcKey,
    pub resolved_time: ResolvedTime,
    pub location: ResolvedLocation,
    pub points: BTreeMap<PointId, PointState>,
    pub houses: Option<HouseState>,
    pub angles: AngleState,
    pub provenance: CalculationProvenance,
}

pub struct CalculationEngine<P> {
    provider: P,
    engine_version: String,
    timezone_data_version: String,
}

impl<P: EphemerisProvider> CalculationEngine<P> {
    pub fn new(
        provider: P,
        engine_version: impl Into<String>,
        timezone_data_version: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            engine_version: engine_version.into(),
            timezone_data_version: timezone_data_version.into(),
        }
    }

    pub fn provider_identity(&self) -> ProviderIdentity {
        self.provider.identity()
    }

    pub fn calc_key(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
    ) -> Result<CalcKey, CalculationError> {
        Ok(CalcKey::derive(
            definition,
            record,
            &self.engine_version,
            &self.provider.identity(),
            &self.timezone_data_version,
        )?)
    }

    pub fn calculate(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        record: &ResourceEnvelope<ChartRecord>,
    ) -> Result<ChartSnapshot, CalculationError> {
        match &definition.payload.source {
            ChartSource::Radix { record: expected } if *expected == record.id => {}
            ChartSource::Radix { record: expected } => {
                return Err(CalculationError::RecordMismatch {
                    expected: *expected,
                    actual: record.id,
                });
            }
            ChartSource::Derived { .. } => {
                return Err(CalculationError::DerivedChartNotImplemented);
            }
        }

        let resolved_time = resolve_time(&record.payload, &self.timezone_data_version)?;
        let location = ResolvedLocation {
            display_name: record.payload.location.display_name.clone(),
            latitude: record.payload.location.latitude,
            longitude: record.payload.location.longitude,
            role: LocationRole::Asserted,
        };
        let request = EphemerisRequest {
            time: resolved_time.clone(),
            location: location.clone(),
            calculation: definition.payload.calculation.clone(),
        };
        let output = self.provider.calculate(&request)?;
        let identity = self.provider.identity();

        Ok(ChartSnapshot {
            definition: ResourceRevisionRef {
                id: definition.id,
                revision: definition.revision,
            },
            calc_key: self.calc_key(definition, record)?,
            resolved_time,
            location,
            points: output.points,
            houses: output.houses,
            angles: output.angles,
            provenance: CalculationProvenance {
                engine_version: self.engine_version.clone(),
                ephemeris_provider: identity.name,
                ephemeris_provider_version: identity.version,
                ephemeris_data_version: identity.data_version,
                timezone_database_version: Some(self.timezone_data_version.clone()),
                calculation_profile_revision: None,
            },
        })
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
            // Longitude is constrained to ±180 degrees, so this rounded value
            // is guaranteed to fit in i32 and in the supported offset range.
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
    #[error(transparent)]
    Time(#[from] TimeResolutionError),
    #[error(transparent)]
    Ephemeris(#[from] EphemerisError),
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
