use std::fmt;

use thiserror::Error;

/// Validation shared by canonical payloads and nested portable domain values.
pub trait DomainValidate {
    fn domain_validate(&self) -> Result<(), DomainValidationError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainValidationIssue {
    Empty,
    Duplicate,
    NonFinite,
    OutOfRange { expected: String },
    InvalidDate { calendar: String },
    InvalidReference,
    InvalidStructure { requirement: String },
    Chronology,
}

impl fmt::Display for DomainValidationIssue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("must not be empty"),
            Self::Duplicate => formatter.write_str("must not contain duplicates"),
            Self::NonFinite => formatter.write_str("must be finite"),
            Self::OutOfRange { expected } => write!(formatter, "must be {expected}"),
            Self::InvalidDate { calendar } => {
                write!(formatter, "is not a valid date in the {calendar} calendar")
            }
            Self::InvalidReference => formatter.write_str("references a missing value"),
            Self::InvalidStructure { requirement } => formatter.write_str(requirement),
            Self::Chronology => {
                formatter.write_str("must not be earlier than the corresponding creation time")
            }
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{path}: {issue}")]
pub struct DomainValidationError {
    pub path: String,
    pub issue: DomainValidationIssue,
}

impl DomainValidationError {
    pub fn new(path: impl Into<String>, issue: DomainValidationIssue) -> Self {
        Self {
            path: path.into(),
            issue,
        }
    }

    pub fn prepend(mut self, prefix: &str) -> Self {
        self.path = if self.path.is_empty() {
            prefix.to_owned()
        } else {
            format!("{prefix}.{}", self.path)
        };
        self
    }
}

pub(crate) fn nonempty(value: &str, path: &str) -> Result<(), DomainValidationError> {
    if value.trim().is_empty() {
        Err(DomainValidationError::new(
            path,
            DomainValidationIssue::Empty,
        ))
    } else {
        Ok(())
    }
}

pub(crate) fn finite(value: f64, path: &str) -> Result<(), DomainValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(DomainValidationError::new(
            path,
            DomainValidationIssue::NonFinite,
        ))
    }
}

pub(crate) fn positive(value: f64, path: &str) -> Result<(), DomainValidationError> {
    finite(value, path)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(DomainValidationError::new(
            path,
            DomainValidationIssue::OutOfRange {
                expected: "greater than zero".into(),
            },
        ))
    }
}

pub(crate) fn in_range(
    value: f64,
    minimum: f64,
    maximum: f64,
    inclusive_maximum: bool,
    path: &str,
) -> Result<(), DomainValidationError> {
    finite(value, path)?;
    let valid = value >= minimum
        && if inclusive_maximum {
            value <= maximum
        } else {
            value < maximum
        };
    if valid {
        Ok(())
    } else {
        let ending = if inclusive_maximum { "]" } else { ")" };
        Err(DomainValidationError::new(
            path,
            DomainValidationIssue::OutOfRange {
                expected: format!("in [{minimum}, {maximum}{ending}"),
            },
        ))
    }
}
