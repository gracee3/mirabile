//! Shared application contract between Mirabile presentation adapters and application implementations.
//!
//! Read models in this crate are UI-oriented projections. They do not replace the canonical
//! domain resources in `mirabile-core`, and an application implementation remains responsible for
//! orchestration, persistence, and derived computation.

mod application;
mod automation;
mod chart_authoring;
mod chart_draft;
mod control;
mod demo;
mod intent;
mod macro_model;
mod read_model;
mod real_application;
mod runtime;
mod startup;
#[cfg(target_arch = "wasm32")]
mod web_worker_runtime;
mod workspace_commands;
mod workspace_session;

pub use application::*;
pub use automation::*;
pub use chart_authoring::*;
pub use chart_draft::*;
pub use control::*;
pub use demo::*;
pub use intent::*;
pub use macro_model::*;
pub use read_model::*;
pub use real_application::*;
pub use runtime::*;
pub use startup::*;
#[cfg(target_arch = "wasm32")]
pub use web_worker_runtime::*;
pub use workspace_session::*;

// Stable identity/value types cross the application boundary without frontend-specific aliases.
pub use mirabile_core::{
    Angle, AspectId, ChartSlotId, CivilDate, CivilTime, CoordinateSystem, CorrectionSpec,
    EventKind, HouseSystem, InstanceId, Latitude, Longitude, Offset, PointId, ResourceId, Revision,
    ViewInstanceId, ZodiacSpec,
};
pub use mirabile_engine::ZodiacMode;

// Scene is the current stable, astrology-free presentation boundary.
pub use mirabile_engine::{Circle, FillRole, Glyph, Label, Line, Path, Scene, StrokeRole};
