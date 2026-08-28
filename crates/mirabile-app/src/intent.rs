use crate::{
    Angle, AspectId, ChartDraft, ChartMutation, ChartSlotId, InstanceId, PointId,
    ResourceDraftKind, ResourceId, ResourceMutation, Revision, ViewInstanceId,
    WorkspaceSwitchAction,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceBindingSlot {
    DisplayedPoints,
    AspectedPoints,
    TransitPoints,
    Aspects,
    Analysis,
    Theme,
    Wheel,
    ViewDocument { view_id: ViewInstanceId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceBindingSelection {
    Follow {
        resource_id: ResourceId,
    },
    Pinned {
        resource_id: ResourceId,
        revision: Revision,
    },
    Inline {
        resource_id: ResourceId,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppIntent {
    BeginNewChart,
    BeginSavedChartEdit {
        instance_id: InstanceId,
    },
    ApplyChartMutation(ChartMutation),
    SaveChartEditor,
    CancelChartEditor,
    /// Compatibility path for non-browser fixtures. Workbench authoring uses typed mutations.
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
    SetWorkspaceBinding {
        slot: WorkspaceBindingSlot,
        selection: WorkspaceBindingSelection,
    },
    NewWorkspace,
    OpenWorkspace {
        resource_id: ResourceId,
    },
    RenameWorkspace {
        title: String,
    },
    DiscardWorkspaceChanges,
    ResolveWorkspaceSwitch {
        action: WorkspaceSwitchAction,
    },
    LoadDemoBundle,
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
    BeginNewAspectSet,
    DuplicateAspectSet {
        resource_id: ResourceId,
    },
    /// Selects one canonical identity for repository revision inspection.
    SelectRepositoryResource {
        resource_id: ResourceId,
    },
    BeginDeleteResource {
        resource_id: ResourceId,
        expected_revision: crate::Revision,
    },
    ConfirmDeleteResource {
        resource_id: ResourceId,
        expected_revision: crate::Revision,
    },
    BeginResourceEdit {
        resource_id: ResourceId,
    },
    BeginResourceCreate {
        kind: ResourceDraftKind,
    },
    ApplyResourceMutation(Box<ResourceMutation>),
    SaveResourceDraft {
        kind: ResourceDraftKind,
    },
    CancelResourceDraft {
        kind: ResourceDraftKind,
    },
    UpdateAspectSetDraft(AspectSetDraftMutation),
    SaveDraft,
    CancelDraft,
    RefreshActiveView,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AspectSetDraftMutation {
    SetTitle(String),
    SetOrb { aspect_id: AspectId, maximum: Angle },
    SetEnabled { aspect_id: AspectId, enabled: bool },
}

impl AppIntent {
    pub fn semantic_summary(&self) -> String {
        match self {
            Self::BeginNewChart | Self::StartChartDraft { .. } => "chart.begin-new".into(),
            Self::BeginSavedChartEdit { instance_id } => {
                format!("chart.begin-saved-edit[{instance_id}]")
            }
            Self::ApplyChartMutation(mutation) => mutation.semantic_summary(),
            Self::SaveChartEditor => "chart.editor.save".into(),
            Self::CancelChartEditor => "chart.editor.cancel".into(),
            Self::SaveChartDraft { instance_id } => format!("chart.save[{instance_id}]"),
            Self::CancelChartDraft { instance_id } => format!("chart.cancel[{instance_id}]"),
            Self::OpenChart { definition_id } => format!("chart.open[{definition_id}]"),
            Self::CloseChart { instance_id } => format!("chart.close[{instance_id}]"),
            Self::ActivateChart { instance_id } => format!("chart.activate[{instance_id}]"),
            Self::SetChartSelection {
                instance_id,
                selected,
            } => format!("chart.select[{instance_id}]={selected}"),
            Self::SetActiveView { view_id } => format!("view.activate[{view_id}]"),
            Self::AssignChartSlot {
                view_id,
                slot,
                chart,
            } => format!(
                "view.slot[{view_id},{}]={}",
                slot.as_str(),
                chart.map_or_else(|| "unassigned".into(), |chart| chart.to_string())
            ),
            Self::SetWorkspaceAspectSet { resource_id } => {
                format!("workspace.aspect-set[{resource_id}]")
            }
            Self::SetWorkspaceBinding { slot, selection } => {
                format!("workspace.binding[{slot:?}]={selection:?}")
            }
            Self::NewWorkspace => "workspace.new".into(),
            Self::OpenWorkspace { resource_id } => format!("workspace.open[{resource_id}]"),
            Self::RenameWorkspace { .. } => "workspace.rename".into(),
            Self::DiscardWorkspaceChanges => "workspace.discard".into(),
            Self::ResolveWorkspaceSwitch { action } => match action {
                WorkspaceSwitchAction::SaveAndSwitch => "workspace.switch.save".into(),
                WorkspaceSwitchAction::DiscardAndSwitch => "workspace.switch.discard".into(),
                WorkspaceSwitchAction::Stay => "workspace.switch.stay".into(),
            },
            Self::LoadDemoBundle => "demo.load".into(),
            Self::SaveWorkspace => "workspace.save".into(),
            Self::SetTemporaryPointHidden { point_id, hidden } => {
                format!("display.point[{}].hidden={hidden}", point_id.as_str())
            }
            Self::PromoteTemporaryDisplay => "display.promote".into(),
            Self::BeginAspectSetEdit { resource_id } => {
                format!("aspect.begin-edit[{resource_id}]")
            }
            Self::BeginNewAspectSet => "aspect.begin-new".into(),
            Self::DuplicateAspectSet { resource_id } => {
                format!("aspect.duplicate[{resource_id}]")
            }
            Self::SelectRepositoryResource { resource_id } => {
                format!("repository.select[{resource_id}]")
            }
            Self::BeginDeleteResource { resource_id, .. } => {
                format!("repository.begin-delete[{resource_id}]")
            }
            Self::ConfirmDeleteResource { resource_id, .. } => {
                format!("repository.confirm-delete[{resource_id}]")
            }
            Self::BeginResourceEdit { resource_id } => {
                format!("resource.begin-edit[{resource_id}]")
            }
            Self::BeginResourceCreate { kind } => format!("resource.begin-new[{kind:?}]"),
            Self::ApplyResourceMutation(mutation) => {
                format!("resource.mutate[{:?}]", mutation.kind())
            }
            Self::SaveResourceDraft { kind } => format!("resource.save[{kind:?}]"),
            Self::CancelResourceDraft { kind } => format!("resource.cancel[{kind:?}]"),
            Self::UpdateAspectSetDraft(AspectSetDraftMutation::SetTitle(_)) => {
                "aspect.title.set".into()
            }
            Self::UpdateAspectSetDraft(AspectSetDraftMutation::SetOrb { aspect_id, maximum }) => {
                format!(
                    "aspect.maximum-orb[{}]={}",
                    aspect_id.as_str(),
                    maximum.degrees()
                )
            }
            Self::UpdateAspectSetDraft(AspectSetDraftMutation::SetEnabled {
                aspect_id,
                enabled,
            }) => format!("aspect.enabled[{}]={enabled}", aspect_id.as_str()),
            Self::SaveDraft => "draft.save".into(),
            Self::CancelDraft => "draft.cancel".into(),
            Self::RefreshActiveView => "application.refresh".into(),
        }
    }
}

impl ChartMutation {
    fn semantic_summary(&self) -> String {
        match self {
            Self::SetTitle(_) => "chart.title.set".into(),
            Self::SetEventKind(_) => "chart.event-kind.set".into(),
            Self::SetSubjectName(Some(_)) => "chart.subject-name.set".into(),
            Self::SetSubjectName(None) => "chart.subject-name.clear".into(),
            Self::SetCivilDate(_) => "chart.civil-date.set".into(),
            Self::SetCivilTime(_) => "chart.civil-time.set".into(),
            Self::SetTimezone(_) => "chart.timezone.set".into(),
            Self::SetLocationEnabled(enabled) => {
                format!("chart.location.enabled={enabled}")
            }
            Self::SetLocationName(_) => "chart.location.name.set".into(),
            Self::SetCountryRegion(Some(_)) => "chart.location.region.set".into(),
            Self::SetCountryRegion(None) => "chart.location.region.clear".into(),
            Self::SetLatitude(Some(_)) => "chart.location.latitude.set".into(),
            Self::SetLatitude(None) => "chart.location.latitude.clear".into(),
            Self::SetLongitude(Some(_)) => "chart.location.longitude.set".into(),
            Self::SetLongitude(None) => "chart.location.longitude.clear".into(),
            Self::SetZodiac(_) => "chart.zodiac.set".into(),
            Self::SetHouseSystem(_) => "chart.houses.set".into(),
            Self::SetCoordinateSystem(_) => "chart.coordinates.set".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_summaries_contain_no_presentation_selectors() {
        let summary = AppIntent::SetTemporaryPointHidden {
            point_id: PointId::new("sun").expect("point"),
            hidden: true,
        }
        .semantic_summary();
        assert_eq!(summary, "display.point[sun].hidden=true");
        assert!(!summary.contains('#'));
    }
}
