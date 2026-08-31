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
mod resource_draft;
mod runtime;
mod startup;
#[cfg(target_arch = "wasm32")]
mod web_worker_runtime;
mod workflow;
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
pub use resource_draft::*;
pub use runtime::*;
pub use startup::*;
#[cfg(target_arch = "wasm32")]
pub use web_worker_runtime::*;
pub use workflow::*;
pub use workspace_session::*;

// Stable identity/value types cross the application boundary without frontend-specific aliases.
pub use mirabile_core::{
    Angle, AspectClass, AspectDefinition, AspectId, AspectLayerKind, AspectLayerVisibility,
    AspectTableObject, AtlasRef, BlackMoonType, CalculationSpec, CalendarSpec, ChartDefinition,
    ChartDetailsObject, ChartRecord, ChartSlot, ChartSlotId, ChartSource, CivilDate, CivilDateTime,
    CivilTime, CompositeMethod, CoordinateSystem, CorrectionSpec, DerivationSpec, EventKind,
    FortuneFormula, GridObject, HouseSystem, InstanceId, Latitude, LifeEvent, LocationAssertion,
    Longitude, LunarNodeType, Note, NumericComparison, ObjectFrame, Offset, OrbPolicy, PointId,
    PointRole, PointSelector, PointTableObject, Predicate, QueryExpr, ResourceId, ResourceKind,
    Revision, RingGeometry, RingSpec, SchemaVersion, SourceType, TemporalAssertion, TextComparison,
    TextObject, Theme, TimeChoice, TimeZoneAssertion, Timestamp, ViewInstanceId, ViewObject,
    ViewOverrides, WheelObject, WheelTemplate, ZodiacSpec,
};
pub use mirabile_engine::ZodiacMode;

// Scene is the stable provider-neutral semantic presentation boundary.
pub use mirabile_engine::{
    AspectLayer, AspectSegment, AspectVisualStyle, ChartAngleMarker, Circle, FillRole, Glyph,
    HouseMarker, Label, Line, LineGeometry, Path, PointMarker, Scene, StrokeRole,
    WheelLayoutBounds, ZodiacDivision,
};
