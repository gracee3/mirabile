use std::fmt;

use mirabile_core::{CoordinateSystem, CorrectionSpec, HouseSystem};
use mirabile_engine::{BackendDescriptor, ZodiacMode};
use serde::{Deserialize, Serialize};

use crate::{
    Angle, AppError, AspectId, CalculationDiagnosticsReadModel, ChartEditorReadModel, ChartSlotId,
    InstanceId, PointId, ResourceId, Revision, Scene, ViewInstanceId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct AppReadModel {
    pub version: ProjectionVersion,
    pub status: ApplicationStatus,
    pub activity: ApplicationActivityReadModel,
    pub calculation: Option<CalculationDiagnosticsReadModel>,
    pub authoring: AuthoringCapabilitiesReadModel,
    pub chart_editor: Option<ChartEditorReadModel>,
    pub library: LibraryReadModel,
    pub workspace: WorkspaceReadModel,
    pub active_view: Option<ViewReadModel>,
    pub inspector: InspectorReadModel,
    pub resource_editor: ResourceEditorReadModel,
    pub capabilities: Vec<CommandCapability>,
    pub notice: Option<AppNotice>,
}

impl AppReadModel {
    pub fn initializing() -> Self {
        Self {
            version: ProjectionVersion::INITIAL,
            status: ApplicationStatus::Initializing,
            activity: ApplicationActivityReadModel::initializing(),
            calculation: None,
            authoring: AuthoringCapabilitiesReadModel::default(),
            chart_editor: None,
            library: LibraryReadModel::default(),
            workspace: WorkspaceReadModel::default(),
            active_view: None,
            inspector: InspectorReadModel::default(),
            resource_editor: ResourceEditorReadModel::default(),
            capabilities: Vec::new(),
            notice: None,
        }
    }

    pub fn availability(&self, action: AppAction) -> Availability {
        self.capabilities
            .iter()
            .find(|capability| capability.action == action)
            .map_or(Availability::Hidden, |capability| {
                capability.availability.clone()
            })
    }

    /// The sole public predicate for authoritative application settlement.
    pub const fn is_settled(&self) -> bool {
        self.activity.settled
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuthoringOption<T> {
    pub value: T,
    pub label: String,
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}

impl<T> AuthoringOption<T> {
    fn enabled(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            enabled: true,
            disabled_reason: None,
        }
    }

    fn disabled(value: T, label: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            value,
            label: label.into(),
            enabled: false,
            disabled_reason: Some(reason.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimezoneAuthoringMode {
    UniversalTime,
    FixedOffset,
    NamedZone,
    LocalMeanTime,
    LocalApparentTime,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AuthoringCapabilitiesReadModel {
    pub zodiac_modes: Vec<AuthoringOption<ZodiacMode>>,
    pub coordinate_systems: Vec<AuthoringOption<CoordinateSystem>>,
    pub house_systems: Vec<AuthoringOption<HouseSystem>>,
    pub timezone_modes: Vec<AuthoringOption<TimezoneAuthoringMode>>,
    pub points: Vec<AuthoringOption<PointId>>,
    pub default_corrections: CorrectionSpec,
}

impl AuthoringCapabilitiesReadModel {
    #[allow(clippy::too_many_lines)]
    pub fn from_backend(descriptor: &BackendDescriptor, complete_location: bool) -> Self {
        let supports_zodiac = |mode| descriptor.authoring.supported_zodiac_modes.contains(&mode);
        let supports_coordinates = |system| {
            descriptor
                .authoring
                .supported_coordinate_systems
                .contains(&system)
        };
        let supported_houses = descriptor
            .capabilities
            .houses
            .as_ref()
            .map(|houses| houses.supported_systems.as_slice())
            .unwrap_or_default();
        let zodiac_modes = [
            (ZodiacMode::Tropical, "Tropical"),
            (ZodiacMode::Sidereal, "Sidereal"),
        ]
        .into_iter()
        .map(|(mode, label)| {
            if supports_zodiac(mode) {
                AuthoringOption::enabled(mode, label)
            } else {
                AuthoringOption::disabled(
                    mode,
                    label,
                    "The active calculation provider does not support this zodiac mode",
                )
            }
        })
        .collect();
        let coordinate_systems = [
            (CoordinateSystem::Geocentric, "Geocentric"),
            (CoordinateSystem::Topocentric, "Topocentric"),
            (CoordinateSystem::Heliocentric, "Heliocentric"),
        ]
        .into_iter()
        .map(|(system, label)| {
            if supports_coordinates(system) {
                AuthoringOption::enabled(system, label)
            } else {
                AuthoringOption::disabled(
                    system,
                    label,
                    "The active calculation provider does not support this coordinate system",
                )
            }
        })
        .collect();
        let house_systems = [
            (HouseSystem::NoHouses, "No houses"),
            (HouseSystem::Equal, "Equal"),
            (HouseSystem::Placidus, "Placidus"),
            (HouseSystem::WholeSign, "Whole Sign"),
        ]
        .into_iter()
        .map(|(system, label)| {
            if system == HouseSystem::NoHouses {
                AuthoringOption::enabled(system, label)
            } else if !supported_houses.contains(&system) {
                AuthoringOption::disabled(
                    system,
                    label,
                    "The active calculation provider does not support this house system",
                )
            } else if !complete_location {
                AuthoringOption::disabled(
                    system,
                    label,
                    "A complete manual location is required for houses",
                )
            } else {
                AuthoringOption::enabled(system, label)
            }
        })
        .collect();
        let deferred_timezone =
            "This timezone mode is deferred until a provider-backed authoring workflow exists";
        let timezone_modes = vec![
            AuthoringOption::enabled(TimezoneAuthoringMode::UniversalTime, "Universal Time"),
            AuthoringOption::enabled(TimezoneAuthoringMode::FixedOffset, "Fixed offset"),
            AuthoringOption::disabled(
                TimezoneAuthoringMode::NamedZone,
                "Named zone",
                deferred_timezone,
            ),
            AuthoringOption::disabled(
                TimezoneAuthoringMode::LocalMeanTime,
                "Local Mean Time",
                deferred_timezone,
            ),
            AuthoringOption::disabled(
                TimezoneAuthoringMode::LocalApparentTime,
                "Local Apparent Time",
                deferred_timezone,
            ),
        ];
        let points = descriptor
            .capabilities
            .celestial
            .as_ref()
            .into_iter()
            .flat_map(|celestial| celestial.supported_points.iter().cloned())
            .map(|point| {
                let label = point.as_str().replace('-', " ");
                AuthoringOption::enabled(point, label)
            })
            .collect();
        Self {
            zodiac_modes,
            coordinate_systems,
            house_systems,
            timezone_modes,
            points,
            default_corrections: descriptor.authoring.default_corrections.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApplicationActivityReadModel {
    pub settled: bool,
    pub pending_operations: Vec<PendingOperationReadModel>,
}

impl ApplicationActivityReadModel {
    pub fn settled() -> Self {
        Self {
            settled: true,
            pending_operations: Vec::new(),
        }
    }

    pub fn pending(pending_operations: Vec<PendingOperationReadModel>) -> Self {
        Self {
            settled: false,
            pending_operations,
        }
    }

    fn initializing() -> Self {
        Self::pending(vec![PendingOperationReadModel::InitializeApplication])
    }
}

impl Default for ApplicationActivityReadModel {
    fn default() -> Self {
        Self::settled()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum PendingOperationReadModel {
    InitializeApplication,
    ViewCalculation {
        view_id: ViewInstanceId,
        request_id: u64,
    },
    ChartCreate {
        instance_id: InstanceId,
    },
    ChartSave {
        definition_id: ResourceId,
    },
    WorkspaceSave {
        resource_id: Option<ResourceId>,
    },
    ResourceSave {
        resource_id: ResourceId,
    },
    DemoLoading,
}

/// Monotonic identity for authoritative application projections.
///
/// This sequence is scoped to an `Application` instance and is independent from canonical
/// resource [`Revision`]. Zero identifies the frontend's pre-initialization placeholder.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ProjectionVersion(u64);

impl ProjectionVersion {
    pub const INITIAL: Self = Self(0);

    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl fmt::Display for ProjectionVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplicationStatus {
    Initializing,
    Ready,
    Error(AppError),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LibraryReadModel {
    pub charts: Vec<LibraryChartSummary>,
    pub aspect_sets: Vec<AspectSetSummary>,
    pub workspaces: Vec<WorkspaceSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSummary {
    pub resource_id: ResourceId,
    pub title: String,
    pub revision: Revision,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LibraryChartSummary {
    /// Stable identity of the saved `ChartDefinition` represented by this library entry.
    pub definition_id: ResourceId,
    pub title: String,
    pub subtitle: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AspectSetSummary {
    pub resource_id: ResourceId,
    pub title: String,
    pub revision: Revision,
    pub conjunction_orb: Angle,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceReadModel {
    pub title: String,
    pub charts: Vec<OpenChartSummary>,
    pub active_chart: Option<InstanceId>,
    pub selected_charts: Vec<InstanceId>,
    pub views: Vec<ViewSummary>,
    pub active_view: Option<ViewInstanceId>,
    pub document_id: Option<ResourceId>,
    pub document_revision: Option<Revision>,
    pub document_dirty: bool,
    pub has_temporary_display_override: bool,
    pub switch_decision: Option<WorkspaceSwitchDecisionReadModel>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceSwitchDecisionReadModel {
    pub target: WorkspaceSwitchTarget,
    pub reasons: Vec<String>,
    pub save_and_switch_enabled: bool,
    pub save_and_switch_disabled_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum WorkspaceSwitchTarget {
    New,
    Saved { resource_id: ResourceId },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceSwitchAction {
    SaveAndSwitch,
    DiscardAndSwitch,
    Stay,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpenChartSummary {
    pub instance_id: InstanceId,
    pub title: String,
    pub subtitle: String,
    pub persistence: ChartPersistence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChartPersistence {
    Saved {
        /// Stable identity of the saved `ChartDefinition`, not its source `ChartRecord`.
        definition_id: ResourceId,
    },
    Ephemeral,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewSummary {
    pub view_id: ViewInstanceId,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewReadModel {
    pub view_id: ViewInstanceId,
    pub title: String,
    pub scene: Option<Scene>,
    pub computation: ViewComputationState,
    pub slots: Vec<ChartSlotAssignment>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewComputationState {
    Loading,
    Fresh,
    Refreshing,
    Failed(AppError),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartSlotAssignment {
    pub slot: ChartSlotId,
    pub label: String,
    pub required: bool,
    pub chart: Option<InstanceId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InspectorReadModel {
    pub active_chart: Option<ActiveChartInspector>,
    pub bindings: Vec<ResourceBindingSummary>,
    pub active_aspect_set: Option<ResourceId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveChartInspector {
    pub instance_id: InstanceId,
    pub title: String,
    pub subtitle: String,
    pub persistence: ChartPersistence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceBindingSummary {
    pub label: String,
    pub source: BindingSourceSummary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingSourceSummary {
    Follow {
        resource_id: ResourceId,
        resource_title: String,
        /// Current resolved revision of the followed resource.
        revision: Revision,
    },
    Pinned {
        resource_id: ResourceId,
        resource_title: String,
        /// Exact revision selected by the pinned binding.
        revision: Revision,
    },
    Inline,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResourceEditorReadModel {
    pub aspect_set: Option<AspectSetDraftReadModel>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AspectSetDraftReadModel {
    pub resource_id: ResourceId,
    pub title: String,
    pub state: DraftState,
    pub conjunction: AspectDraftValue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AspectDraftValue {
    pub aspect_id: AspectId,
    pub label: String,
    pub enabled: bool,
    pub maximum_orb: Angle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DraftState {
    Clean {
        revision: Revision,
    },
    Dirty {
        base_revision: Revision,
    },
    Saving {
        base_revision: Revision,
    },
    Conflict {
        base_revision: Revision,
        remote_revision: Revision,
    },
}

impl DraftState {
    pub const fn base_revision(&self) -> Revision {
        match self {
            Self::Clean { revision } => *revision,
            Self::Dirty { base_revision }
            | Self::Saving { base_revision }
            | Self::Conflict { base_revision, .. } => *base_revision,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AppAction {
    BeginNewChart,
    SaveChartEditor,
    CancelChartEditor,
    SaveChartDraft,
    CancelChartDraft,
    BeginAspectSetEdit,
    SaveDraft,
    CancelDraft,
    SaveWorkspace,
    PromoteWorkspaceDisplay,
    RefreshView,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandCapability {
    pub action: AppAction,
    pub availability: Availability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Availability {
    Enabled,
    Disabled { reason: Option<String> },
    Hidden,
}

impl Availability {
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled)
    }

    pub fn disabled_reason(&self) -> Option<&str> {
        match self {
            Self::Disabled {
                reason: Some(reason),
            } => Some(reason),
            Self::Enabled | Self::Disabled { reason: None } | Self::Hidden => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppNotice {
    pub kind: AppNoticeKind,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppNoticeKind {
    Info,
    Success,
    Warning,
    Conflict,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializing_projection_has_no_false_ready_data() {
        let model = AppReadModel::initializing();

        assert_eq!(model.status, ApplicationStatus::Initializing);
        assert_eq!(model.version, ProjectionVersion::INITIAL);
        assert!(model.active_view.is_none());
        assert!(!model.is_settled());
        assert_eq!(
            model.activity.pending_operations,
            vec![PendingOperationReadModel::InitializeApplication]
        );
        assert_eq!(
            model.availability(AppAction::SaveDraft),
            Availability::Hidden
        );
    }

    #[test]
    fn settlement_is_owned_by_the_activity_projection() {
        let mut model = AppReadModel::initializing();
        model.activity = ApplicationActivityReadModel::settled();

        assert!(model.is_settled());
    }

    #[test]
    fn inline_binding_summary_requires_no_resource_identity() {
        let binding = ResourceBindingSummary {
            label: "Theme".into(),
            source: BindingSourceSummary::Inline,
        };

        assert_eq!(binding.source, BindingSourceSummary::Inline);
    }

    #[test]
    fn chart_definition_identity_is_explicit_in_contract() {
        let definition_id = ResourceId::new();
        let summary = LibraryChartSummary {
            definition_id,
            title: "Definition fixture".into(),
            subtitle: "Saved chart definition".into(),
        };
        let persistence = ChartPersistence::Saved { definition_id };
        let intent = crate::AppIntent::OpenChart { definition_id };

        assert_eq!(summary.definition_id, definition_id);
        assert_eq!(persistence, ChartPersistence::Saved { definition_id });
        assert_eq!(intent, crate::AppIntent::OpenChart { definition_id });
    }

    #[test]
    fn disabled_capability_retains_application_reason() {
        let mut model = AppReadModel::initializing();
        model.capabilities.push(CommandCapability {
            action: AppAction::SaveDraft,
            availability: Availability::Disabled {
                reason: Some("Begin an edit before saving".into()),
            },
        });

        let availability = model.availability(AppAction::SaveDraft);
        assert_eq!(
            availability.disabled_reason(),
            Some("Begin an edit before saving")
        );
    }

    #[test]
    fn backend_authoring_projection_is_contextual_and_truthful() {
        use mirabile_engine::{CalculationBackend as _, DeterministicBackend};

        let descriptor = DeterministicBackend.descriptor();
        let locationless = AuthoringCapabilitiesReadModel::from_backend(&descriptor, false);
        assert!(
            locationless
                .zodiac_modes
                .iter()
                .find(|option| option.value == ZodiacMode::Tropical)
                .is_some_and(|option| option.enabled)
        );
        assert!(
            locationless
                .zodiac_modes
                .iter()
                .find(|option| option.value == ZodiacMode::Sidereal)
                .is_some_and(|option| !option.enabled && option.disabled_reason.is_some())
        );
        assert!(
            locationless
                .house_systems
                .iter()
                .find(|option| option.value == HouseSystem::Equal)
                .is_some_and(|option| !option.enabled)
        );
        let located = AuthoringCapabilitiesReadModel::from_backend(&descriptor, true);
        assert!(
            located
                .house_systems
                .iter()
                .find(|option| option.value == HouseSystem::Equal)
                .is_some_and(|option| option.enabled)
        );
        assert_eq!(located.default_corrections, CorrectionSpec::default());
    }

    #[cfg(feature = "xalen-backend")]
    #[test]
    fn xalen_authoring_projection_exposes_only_supported_provider_choices() {
        use mirabile_engine::{CalculationBackend as _, XalenBackend};

        let capabilities =
            AuthoringCapabilitiesReadModel::from_backend(&XalenBackend.descriptor(), true);
        assert_eq!(
            capabilities
                .points
                .iter()
                .map(|option| option.value.as_str())
                .collect::<Vec<_>>(),
            vec!["jupiter", "mars", "mercury", "moon", "sun", "venus"]
        );
        assert!(
            capabilities
                .house_systems
                .iter()
                .any(|option| { option.value == HouseSystem::Placidus && option.enabled })
        );
        assert!(capabilities.house_systems.iter().any(|option| {
            option.value == HouseSystem::WholeSign
                && !option.enabled
                && option.disabled_reason.is_some()
        }));
        assert_eq!(
            capabilities.default_corrections,
            CorrectionSpec {
                aberration: true,
                light_time: true,
                nutation: true,
            }
        );
    }
}
