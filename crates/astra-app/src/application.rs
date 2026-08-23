use async_trait::async_trait;
use thiserror::Error;

use crate::{AppIntent, AppReadModel, ProjectionVersion};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppErrorKind {
    Initialization,
    ViewComputation,
    Conflict,
    InvalidIntent,
    NotFound,
    Unavailable,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct AppError {
    pub kind: AppErrorKind,
    pub message: String,
}

impl AppError {
    pub fn new(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Authoritative application boundary used by presentation adapters.
///
/// Dispatch returns the authoritative projection after an intent is accepted. It may expose an
/// intermediate state such as `Refreshing` or `Saving`.
#[async_trait(?Send)]
pub trait Application {
    async fn initialize(&self) -> AppResult<AppReadModel>;

    async fn dispatch(&self, intent: AppIntent) -> AppResult<AppReadModel>;

    /// Returns the current authoritative projection immediately.
    ///
    /// This method never waits for or completes pending work.
    async fn snapshot(&self) -> AppResult<AppReadModel>;

    /// Waits for an authoritative projection newer than `after`.
    ///
    /// If a newer projection already exists, it may be returned immediately. Otherwise the
    /// implementation waits for a meaningful application transition, such as repository or
    /// worker completion. A successful result always has `version > after`.
    async fn wait_for_update(&self, after: ProjectionVersion) -> AppResult<AppReadModel>;
}
