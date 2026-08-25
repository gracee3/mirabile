use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    Angle, AppError, AspectId, CalculationDiagnosticsReadModel, ChartSlotId, InstanceId,
    ResourceId, Revision, Scene, ViewInstanceId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct AppReadModel {
    pub version: ProjectionVersion,
    pub status: ApplicationStatus,
    pub activity: ApplicationActivityReadModel,
    pub calculation: Option<CalculationDiagnosticsReadModel>,
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
    pub charts: Vec<OpenChartSummary>,
    pub active_chart: Option<InstanceId>,
    pub selected_charts: Vec<InstanceId>,
    pub views: Vec<ViewSummary>,
    pub active_view: Option<ViewInstanceId>,
    pub document_id: Option<ResourceId>,
    pub document_revision: Option<Revision>,
    pub document_dirty: bool,
    pub has_temporary_display_override: bool,
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
}
