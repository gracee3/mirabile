use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use thiserror::Error;

use crate::{
    DomainValidate, DomainValidationError, DomainValidationIssue, Offset, validation::nonempty,
};

/// A civil date using astronomical year numbering; year 0 is 1 BCE.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CivilDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CivilDate {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, CivilTimeError> {
        if !(1..=12).contains(&month) {
            return Err(CivilTimeError::InvalidMonth(month));
        }
        // Calendar-independent construction deliberately permits February 29.
        // The enclosing TemporalAssertion applies the selected calendar rules.
        let maximum = structural_days_in_month(month);
        if day == 0 || day > maximum {
            return Err(CivilTimeError::InvalidDay { day, maximum });
        }
        Ok(Self { year, month, day })
    }

    pub const fn year(self) -> i32 {
        self.year
    }

    pub const fn month(self) -> u8 {
        self.month
    }

    pub const fn day(self) -> u8 {
        self.day
    }
}

#[derive(Deserialize)]
struct CivilDateWire {
    year: i32,
    month: u8,
    day: u8,
}

impl<'de> Deserialize<'de> for CivilDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = CivilDateWire::deserialize(deserializer)?;
        Self::new(value.year, value.month, value.day).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct CivilTime {
    hour: u8,
    minute: u8,
    second: u8,
}

#[derive(Deserialize)]
struct CivilTimeWire {
    hour: u8,
    minute: u8,
    second: u8,
}

impl<'de> Deserialize<'de> for CivilTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = CivilTimeWire::deserialize(deserializer)?;
        Self::new(value.hour, value.minute, value.second).map_err(D::Error::custom)
    }
}

impl CivilTime {
    pub fn new(hour: u8, minute: u8, second: u8) -> Result<Self, CivilTimeError> {
        if hour > 23 {
            return Err(CivilTimeError::InvalidHour(hour));
        }
        if minute > 59 {
            return Err(CivilTimeError::InvalidMinute(minute));
        }
        if second > 59 {
            return Err(CivilTimeError::InvalidSecond(second));
        }
        Ok(Self {
            hour,
            minute,
            second,
        })
    }

    pub const fn hour(self) -> u8 {
        self.hour
    }

    pub const fn minute(self) -> u8 {
        self.minute
    }

    pub const fn second(self) -> u8 {
        self.second
    }

    pub fn seconds_since_midnight(self) -> u32 {
        u32::from(self.hour) * 3_600 + u32::from(self.minute) * 60 + u32::from(self.second)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CivilDateTime {
    pub date: CivilDate,
    pub time: CivilTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalendarSpec {
    ProlepticGregorian,
    Julian,
    HistoricalTransition { identifier: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeChoice {
    Earlier,
    Later,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum TimeZoneAssertion {
    NamedZone(String),
    FixedOffset(Offset),
    LocalMeanTime,
    LocalApparentTime,
    UniversalTime,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TemporalAssertion {
    pub civil_datetime: CivilDateTime,
    pub calendar: CalendarSpec,
    pub zone: TimeZoneAssertion,
    pub disambiguation: Option<TimeChoice>,
}

impl DomainValidate for TemporalAssertion {
    fn domain_validate(&self) -> Result<(), DomainValidationError> {
        let date = self.civil_datetime.date;
        if date.month() == 2 && date.day() == 29 {
            let valid = match &self.calendar {
                CalendarSpec::ProlepticGregorian => is_gregorian_leap_year(date.year()),
                CalendarSpec::Julian => is_julian_leap_year(date.year()),
                CalendarSpec::HistoricalTransition { .. } => true,
            };
            if !valid {
                return Err(DomainValidationError::new(
                    "civil_datetime.date",
                    DomainValidationIssue::InvalidDate {
                        calendar: match self.calendar {
                            CalendarSpec::ProlepticGregorian => "proleptic Gregorian",
                            CalendarSpec::Julian => "Julian",
                            CalendarSpec::HistoricalTransition { .. } => "historical transition",
                        }
                        .into(),
                    },
                ));
            }
        }
        if let CalendarSpec::HistoricalTransition { identifier } = &self.calendar {
            nonempty(identifier, "calendar.identifier")?;
        }
        if let TimeZoneAssertion::NamedZone(name) = &self.zone {
            nonempty(name, "zone.value")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AstroInstant(f64);

impl AstroInstant {
    pub fn from_julian_day(value: f64) -> Result<Self, CivilTimeError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(CivilTimeError::InvalidInstant)
        }
    }

    pub const fn julian_day(self) -> f64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for AstroInstant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_julian_day(f64::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Time scale carried by a resolved astronomical instant.
///
/// The scale is explicit so calculation providers cannot reinterpret a Julian
/// day as UT1 or TT merely because those scales use the same numeric encoding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeScale {
    Utc,
    Tai,
    Tt,
    Ut1,
    Tdb,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResolvedTime {
    pub instant: AstroInstant,
    pub scale: TimeScale,
    pub applied_offset: Offset,
    pub timezone_data_version: String,
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CivilTimeError {
    #[error("invalid month {0}")]
    InvalidMonth(u8),
    #[error("invalid day {day}; maximum for the month is {maximum}")]
    InvalidDay { day: u8, maximum: u8 },
    #[error("invalid hour {0}")]
    InvalidHour(u8),
    #[error("invalid minute {0}")]
    InvalidMinute(u8),
    #[error("invalid second {0}")]
    InvalidSecond(u8),
    #[error("astronomical instant must be finite")]
    InvalidInstant,
}

const fn structural_days_in_month(month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => 29,
        _ => 31,
    }
}

const fn is_gregorian_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

const fn is_julian_leap_year(year: i32) -> bool {
    year % 4 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_date_defers_leap_day_rules_to_the_calendar_assertion() {
        assert!(CivilDate::new(2000, 2, 29).is_ok());
        assert!(CivilDate::new(1900, 2, 29).is_ok());
        assert!(serde_json::from_str::<CivilDate>(r#"{"year":1900,"month":2,"day":29}"#).is_ok());
    }

    #[test]
    fn calendar_rules_distinguish_gregorian_and_julian_leap_days() {
        let assertion = |calendar| TemporalAssertion {
            civil_datetime: CivilDateTime {
                date: CivilDate::new(1900, 2, 29).expect("structurally valid date"),
                time: CivilTime::new(0, 0, 0).expect("valid time"),
            },
            calendar,
            zone: TimeZoneAssertion::UniversalTime,
            disambiguation: None,
        };

        assert!(
            assertion(CalendarSpec::ProlepticGregorian)
                .domain_validate()
                .is_err()
        );
        assert!(assertion(CalendarSpec::Julian).domain_validate().is_ok());
    }
}
