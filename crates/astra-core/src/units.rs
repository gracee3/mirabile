use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Angle(f64);

impl Angle {
    pub fn from_degrees(value: f64) -> Result<Self, UnitError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(UnitError::NotFinite)
        }
    }

    pub fn normalized(value: f64) -> Result<Self, UnitError> {
        Self::from_degrees(value.rem_euclid(360.0))
    }

    pub const fn degrees(self) -> f64 {
        self.0
    }

    pub fn separation(self, other: Self) -> Self {
        let difference = (self.0 - other.0).rem_euclid(360.0);
        Self(difference.min(360.0 - difference))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Latitude(f64);

impl Latitude {
    pub fn from_degrees(value: f64) -> Result<Self, UnitError> {
        if !value.is_finite() {
            Err(UnitError::NotFinite)
        } else if !(-90.0..=90.0).contains(&value) {
            Err(UnitError::OutOfRange {
                minimum: -90.0,
                maximum: 90.0,
                actual: value,
            })
        } else {
            Ok(Self(value))
        }
    }

    pub const fn degrees(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Longitude(f64);

impl Longitude {
    pub fn from_degrees(value: f64) -> Result<Self, UnitError> {
        if !value.is_finite() {
            Err(UnitError::NotFinite)
        } else if !(-180.0..=180.0).contains(&value) {
            Err(UnitError::OutOfRange {
                minimum: -180.0,
                maximum: 180.0,
                actual: value,
            })
        } else {
            Ok(Self(value))
        }
    }

    pub const fn degrees(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct AngularVelocity(f64);

impl AngularVelocity {
    pub fn degrees_per_day(value: f64) -> Result<Self, UnitError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(UnitError::NotFinite)
        }
    }

    pub const fn as_degrees_per_day(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Offset(i32);

impl Offset {
    pub const UTC: Self = Self(0);

    pub const fn from_seconds(value: i32) -> Result<Self, UnitError> {
        if value < -86_400 || value > 86_400 {
            Err(UnitError::OffsetOutOfRange(value))
        } else {
            Ok(Self(value))
        }
    }

    pub const fn seconds(self) -> i32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq)]
pub enum UnitError {
    #[error("numeric value must be finite")]
    NotFinite,
    #[error("value {actual} is outside [{minimum}, {maximum}]")]
    OutOfRange {
        minimum: f64,
        maximum: f64,
        actual: f64,
    },
    #[error("UTC offset {0} seconds is outside the supported range")]
    OffsetOutOfRange(i32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_separation_takes_short_arc() {
        let lhs = Angle::from_degrees(350.0).expect("valid angle");
        let rhs = Angle::from_degrees(10.0).expect("valid angle");
        assert_eq!(lhs.separation(rhs).degrees(), 20.0);
    }

    #[test]
    fn coordinates_enforce_physical_bounds() {
        assert!(Latitude::from_degrees(90.1).is_err());
        assert!(Longitude::from_degrees(-180.0).is_ok());
    }
}
