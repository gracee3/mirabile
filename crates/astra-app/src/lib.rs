//! Shared application contract between Astra presentation adapters and application implementations.
//!
//! Read models in this crate are UI-oriented projections. They do not replace the canonical
//! domain resources in `astra-core`, and an application implementation remains responsible for
//! orchestration, persistence, and derived computation.

mod application;
mod bootstrap;
mod intent;
mod read_model;
mod real_application;
mod runtime;
#[cfg(target_arch = "wasm32")]
mod web_worker_runtime;
mod workspace_commands;

pub use application::*;
pub use bootstrap::*;
pub use intent::*;
pub use read_model::*;
pub use real_application::*;
pub use runtime::*;
#[cfg(target_arch = "wasm32")]
pub use web_worker_runtime::*;

// Stable identity/value types cross the application boundary without frontend-specific aliases.
pub use astra_core::{
    Angle, AspectId, ChartSlotId, InstanceId, ResourceId, Revision, ViewInstanceId,
};

// Scene is the current stable, astrology-free presentation boundary.
pub use astra_engine::{Circle, FillRole, Glyph, Label, Line, Path, Scene, StrokeRole};
