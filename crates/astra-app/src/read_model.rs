use crate::{
    Angle, AppError, AspectId, ChartSlotId, InstanceId, ResourceId, Revision, Scene, ViewInstanceId,
};

#[derive(Clone, Debug, PartialEq)]
pub struct AppReadModel {
    pub status: ApplicationStatus,
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
            status: ApplicationStatus::Initializing,
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
    pub resource_id: ResourceId,
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
    Saved { resource_id: ResourceId },
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
    pub resource_id: ResourceId,
    pub resource_title: String,
    pub revision: Revision,
    pub mode: BindingMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingMode {
    Follow,
    Pinned,
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
    BeginAspectSetEdit,
    SaveDraft,
    CancelDraft,
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
        assert!(model.active_view.is_none());
        assert_eq!(
            model.availability(AppAction::SaveDraft),
            Availability::Hidden
        );
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
