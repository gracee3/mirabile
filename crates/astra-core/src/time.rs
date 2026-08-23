use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Offset;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
        let maximum = days_in_month(year, month);
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CivilTime {
    hour: u8,
    minute: u8,
    second: u8,
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, PartialOrd, Serialize)]
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ResolvedTime {
    pub instant: AstroInstant,
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

const fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

const fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_date_checks_leap_days() {
        assert!(CivilDate::new(2000, 2, 29).is_ok());
        assert!(CivilDate::new(1900, 2, 29).is_err());
    }
}
