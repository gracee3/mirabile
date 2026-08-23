use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};

use crate::{
    BackendFingerprint, CalcKey, CalculationBackend, CalculationBackendError,
    CalculationBackendErrorCategory, CalculationBackendResult, ResolvedCalculationRequest,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CalculationRequestId(u64);

impl CalculationRequestId {
    pub const FIRST: Self = Self(1);

    pub const fn new(value: u64) -> Result<Self, CalculationRequestIdError> {
        if value == 0 {
            Err(CalculationRequestIdError::Zero)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Result<Self, CalculationRequestIdError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(CalculationRequestIdError::Overflow),
        }
    }
}

impl<'de> Deserialize<'de> for CalculationRequestId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(u64::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl fmt::Display for CalculationRequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CalculationRequestIdError {
    #[error("calculation request ID zero is invalid")]
    Zero,
    #[error("calculation request ID overflow")]
    Overflow,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WorkerProtocolVersion(u16);

impl WorkerProtocolVersion {
    pub const CURRENT: Self = Self(1);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }

    pub const fn is_supported(self) -> bool {
        self.0 == Self::CURRENT.0
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalculationWorkerRequest {
    pub protocol_version: WorkerProtocolVersion,
    pub request_id: CalculationRequestId,
    pub calc_key: CalcKey,
    pub backend: BackendFingerprint,
    pub request: ResolvedCalculationRequest,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CalculationWorkerResult {
    pub protocol_version: WorkerProtocolVersion,
    pub request_id: CalculationRequestId,
    pub calc_key: CalcKey,
    pub outcome: CalculationOutcome,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
pub enum CalculationOutcome {
    Success(Box<CalculationBackendResult>),
    Failure(CalculationWorkerFailure),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CalculationWorkerFailureCategory {
    InvalidInput,
    UnsupportedCapability,
    BackendFailure,
    ProtocolMismatch,
    InternalExecutionFailure,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CalculationWorkerFailure {
    pub category: CalculationWorkerFailureCategory,
    pub message: String,
}

pub fn execute_calculation_request<B: CalculationBackend>(
    backend: &B,
    request: CalculationWorkerRequest,
) -> CalculationWorkerResult {
    let outcome = if !request.protocol_version.is_supported() {
        CalculationOutcome::Failure(CalculationWorkerFailure {
            category: CalculationWorkerFailureCategory::ProtocolMismatch,
            message: format!(
                "worker protocol {} is incompatible with supported version {}",
                request.protocol_version.get(),
                WorkerProtocolVersion::CURRENT.get()
            ),
        })
    } else if request.backend != backend.descriptor().fingerprint {
        CalculationOutcome::Failure(CalculationWorkerFailure {
            category: CalculationWorkerFailureCategory::InvalidInput,
            message: "requested backend fingerprint does not match the executing backend".into(),
        })
    } else {
        match backend.calculate(&request.request) {
            Ok(result) => CalculationOutcome::Success(Box::new(result)),
            Err(error) => CalculationOutcome::Failure(worker_failure(error)),
        }
    };
    CalculationWorkerResult {
        protocol_version: WorkerProtocolVersion::CURRENT,
        request_id: request.request_id,
        calc_key: request.calc_key,
        outcome,
    }
}

fn worker_failure(error: CalculationBackendError) -> CalculationWorkerFailure {
    let category = match error.category {
        CalculationBackendErrorCategory::InvalidInput => {
            CalculationWorkerFailureCategory::InvalidInput
        }
        CalculationBackendErrorCategory::UnsupportedCapability => {
            CalculationWorkerFailureCategory::UnsupportedCapability
        }
        CalculationBackendErrorCategory::ExecutionFailure => {
            CalculationWorkerFailureCategory::BackendFailure
        }
        CalculationBackendErrorCategory::Internal => {
            CalculationWorkerFailureCategory::InternalExecutionFailure
        }
    };
    CalculationWorkerFailure {
        category,
        message: error.message,
    }
}
