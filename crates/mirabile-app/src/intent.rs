use crate::{
    Angle, AspectId, ChartDraft, ChartSlotId, InstanceId, PointId, ResourceId, ViewInstanceId,
};

#[derive(Clone, Debug, PartialEq)]
pub enum AppIntent {
    StartChartDraft {
        draft: Box<ChartDraft>,
    },
    SaveChartDraft {
        instance_id: InstanceId,
    },
    CancelChartDraft {
        instance_id: InstanceId,
    },
    OpenChart {
        /// Stable identity of the saved `ChartDefinition`, not its source `ChartRecord`.
        definition_id: ResourceId,
    },
    CloseChart {
        instance_id: InstanceId,
    },
    ActivateChart {
        instance_id: InstanceId,
    },
    /// Changes selection only. It does not implicitly activate or deactivate a chart.
    SetChartSelection {
        instance_id: InstanceId,
        selected: bool,
    },
    SetActiveView {
        view_id: ViewInstanceId,
    },
    AssignChartSlot {
        view_id: ViewInstanceId,
        slot: ChartSlotId,
        chart: Option<InstanceId>,
    },
    SetWorkspaceAspectSet {
        resource_id: ResourceId,
    },
    /// Creates revision one for an unsaved session or persists the next dirty saved revision.
    SaveWorkspace,
    /// Applies a session-only point visibility override to the active view.
    SetTemporaryPointHidden {
        point_id: PointId,
        hidden: bool,
    },
    /// Copies the active view's temporary override into the durable document and marks it dirty.
    PromoteTemporaryDisplay,
    BeginAspectSetEdit {
        resource_id: ResourceId,
    },
    UpdateAspectSetDraft(AspectSetDraftMutation),
    SaveDraft,
    CancelDraft,
    RefreshActiveView,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AspectSetDraftMutation {
    SetOrb { aspect_id: AspectId, maximum: Angle },
    SetEnabled { aspect_id: AspectId, enabled: bool },
}
