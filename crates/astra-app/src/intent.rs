use crate::{Angle, AspectId, ChartSlotId, InstanceId, ResourceId, ViewInstanceId};

#[derive(Clone, Debug, PartialEq)]
pub enum AppIntent {
    OpenChart {
        resource_id: ResourceId,
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
