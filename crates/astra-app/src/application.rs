use async_trait::async_trait;
use thiserror::Error;

use crate::{AppIntent, AppReadModel};

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
/// Dispatch currently returns a complete projection. Implementations may expose an accepted
/// intermediate state (for example, `Refreshing`) and complete queued work in a later `snapshot`
/// call. That keeps asynchronous view state visible without making events authoritative.
#[async_trait(?Send)]
pub trait Application {
    async fn initialize(&self) -> AppResult<AppReadModel>;

    async fn dispatch(&self, intent: AppIntent) -> AppResult<AppReadModel>;

    async fn snapshot(&self) -> AppResult<AppReadModel>;
}
