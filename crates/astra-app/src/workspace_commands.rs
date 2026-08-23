use astra_core::{
    Command, DomainValidate, InstanceId, ResourceBinding, ResourceId, ViewInstanceId, Workspace,
    WorkspaceChart,
};
use thiserror::Error;

#[allow(clippy::too_many_lines)]
pub(crate) fn apply_workspace_command(
    workspace_id: ResourceId,
    workspace: &mut Workspace,
    command: &Command,
) -> Result<(), WorkspaceCommandError> {
    match command {
        Command::OpenSavedChart {
            workspace: target,
            definition,
            instance_id,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(existing) = workspace
                .chart_instances
                .iter()
                .find_map(|chart| match chart {
                    WorkspaceChart::Saved {
                        instance_id,
                        definition: candidate,
                    } if candidate == definition => Some(*instance_id),
                    WorkspaceChart::Saved { .. } | WorkspaceChart::Ephemeral { .. } => None,
                })
            {
                workspace.active_chart = Some(existing);
            } else {
                workspace.chart_instances.push(WorkspaceChart::Saved {
                    instance_id: *instance_id,
                    definition: *definition,
                });
                workspace.active_chart = Some(*instance_id);
            }
        }
        Command::OpenEphemeralChart {
            workspace: target,
            definition,
            instance_id,
        } => {
            ensure_workspace(workspace_id, *target)?;
            ensure_instance_missing(workspace, *instance_id)?;
            workspace.chart_instances.push(WorkspaceChart::Ephemeral {
                instance_id: *instance_id,
                definition: definition.clone(),
            });
            workspace.active_chart = Some(*instance_id);
        }
        Command::CloseChart {
            workspace: target,
            instance_id,
        } => {
            ensure_workspace(workspace_id, *target)?;
            let index = workspace
                .chart_instances
                .iter()
                .position(|chart| chart.instance_id() == *instance_id)
                .ok_or(WorkspaceCommandError::ChartNotOpen(*instance_id))?;
            let was_active = workspace.active_chart == Some(*instance_id);
            workspace.chart_instances.remove(index);
            workspace.selected_charts.retain(|id| id != instance_id);
            if was_active {
                workspace.active_chart = workspace
                    .chart_instances
                    .get(index)
                    .or_else(|| {
                        index
                            .checked_sub(1)
                            .and_then(|prior| workspace.chart_instances.get(prior))
                    })
                    .map(WorkspaceChart::instance_id);
            }
            let replacement = workspace.active_chart;
            for view in &mut workspace.views {
                let required_slots = match &view.document {
                    ResourceBinding::Inline { value } => value
                        .chart_slots
                        .iter()
                        .filter(|slot| slot.required)
                        .map(|slot| slot.id.clone())
                        .collect::<Vec<_>>(),
                    ResourceBinding::Follow { .. } | ResourceBinding::Pinned { .. } => Vec::new(),
                };
                let affected = view
                    .charts
                    .iter()
                    .filter_map(|(slot, chart)| (*chart == *instance_id).then_some(slot.clone()))
                    .collect::<Vec<_>>();
                for slot in affected {
                    if required_slots.contains(&slot) {
                        if let Some(replacement) = replacement {
                            view.charts.insert(slot, replacement);
                        } else {
                            view.charts.remove(&slot);
                        }
                    } else {
                        view.charts.remove(&slot);
                    }
                }
            }
        }
        Command::SetActiveChart {
            workspace: target,
            instance_id,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(instance_id) = instance_id {
                ensure_chart_open(workspace, *instance_id)?;
            }
            workspace.active_chart = *instance_id;
        }
        Command::SetChartSelection {
            workspace: target,
            instance_id,
            selected,
        } => {
            ensure_workspace(workspace_id, *target)?;
            ensure_chart_open(workspace, *instance_id)?;
            if *selected && !workspace.selected_charts.contains(instance_id) {
                workspace.selected_charts.push(*instance_id);
            } else if !selected {
                workspace.selected_charts.retain(|id| id != instance_id);
            }
        }
        Command::SetActiveView {
            workspace: target,
            view,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(view) = view {
                ensure_view_exists(workspace, *view)?;
            }
            workspace.active_view = *view;
        }
        Command::AssignChartSlot {
            workspace: target,
            view,
            slot,
            chart,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(chart) = chart {
                ensure_chart_open(workspace, *chart)?;
            }
            let view = workspace
                .views
                .iter_mut()
                .find(|candidate| candidate.id == *view)
                .ok_or(WorkspaceCommandError::ViewNotFound(*view))?;
            if let ResourceBinding::Inline { value } = &view.document
                && !value
                    .chart_slots
                    .iter()
                    .any(|candidate| candidate.id == *slot)
            {
                return Err(WorkspaceCommandError::SlotNotFound(slot.to_string()));
            }
            if let Some(chart) = chart {
                view.charts.insert(slot.clone(), *chart);
            } else {
                view.charts.remove(slot);
            }
        }
        Command::SetWorkspaceAspectSet {
            workspace: target,
            aspect_set,
        } => {
            ensure_workspace(workspace_id, *target)?;
            workspace.profile.aspects = ResourceBinding::Follow { id: *aspect_set };
        }
        Command::CreateResource { .. } | Command::SaveResourceDraft { .. } => {
            return Err(WorkspaceCommandError::NotWorkspaceCommand);
        }
    }
    workspace
        .domain_validate()
        .map_err(|error| WorkspaceCommandError::InvalidWorkspace(error.to_string()))
}

fn ensure_workspace(
    actual: ResourceId,
    requested: ResourceId,
) -> Result<(), WorkspaceCommandError> {
    if actual == requested {
        Ok(())
    } else {
        Err(WorkspaceCommandError::WorkspaceMismatch {
            expected: actual,
            actual: requested,
        })
    }
}

fn ensure_chart_open(
    workspace: &Workspace,
    instance_id: InstanceId,
) -> Result<(), WorkspaceCommandError> {
    workspace
        .chart_instances
        .iter()
        .any(|chart| chart.instance_id() == instance_id)
        .then_some(())
        .ok_or(WorkspaceCommandError::ChartNotOpen(instance_id))
}

fn ensure_instance_missing(
    workspace: &Workspace,
    instance_id: InstanceId,
) -> Result<(), WorkspaceCommandError> {
    if workspace
        .chart_instances
        .iter()
        .any(|chart| chart.instance_id() == instance_id)
    {
        Err(WorkspaceCommandError::DuplicateInstance(instance_id))
    } else {
        Ok(())
    }
}

fn ensure_view_exists(
    workspace: &Workspace,
    view_id: ViewInstanceId,
) -> Result<(), WorkspaceCommandError> {
    workspace
        .views
        .iter()
        .any(|view| view.id == view_id)
        .then_some(())
        .ok_or(WorkspaceCommandError::ViewNotFound(view_id))
}

#[derive(Debug, Error)]
pub(crate) enum WorkspaceCommandError {
    #[error("command targets workspace {actual}, but the active workspace is {expected}")]
    WorkspaceMismatch {
        expected: ResourceId,
        actual: ResourceId,
    },
    #[error("chart instance {0} is not open")]
    ChartNotOpen(InstanceId),
    #[error("chart instance {0} already exists")]
    DuplicateInstance(InstanceId),
    #[error("view {0} was not found")]
    ViewNotFound(ViewInstanceId),
    #[error("chart slot {0} was not found")]
    SlotNotFound(String),
    #[error("workspace command produced invalid state: {0}")]
    InvalidWorkspace(String),
    #[error("resource persistence command was sent to the workspace handler")]
    NotWorkspaceCommand,
}

#[cfg(test)]
mod tests {
    use astra_core::{ChartSlotId, Command};

    use crate::{bootstrap_ids, bootstrap_resources};

    use super::*;

    fn workspace_fixture() -> (ResourceId, Workspace) {
        let resource = bootstrap_resources()
            .into_iter()
            .find(|resource| matches!(resource, astra_core::CanonicalResource::Workspace(_)))
            .expect("bootstrap workspace exists");
        let astra_core::CanonicalResource::Workspace(envelope) = resource else {
            unreachable!()
        };
        (envelope.id, envelope.payload)
    }

    #[test]
    fn activation_and_selection_remain_independent() {
        let (workspace_id, mut workspace) = workspace_fixture();
        let initial = workspace.active_chart.expect("active chart");
        apply_workspace_command(
            workspace_id,
            &mut workspace,
            &Command::SetChartSelection {
                workspace: workspace_id,
                instance_id: initial,
                selected: true,
            },
        )
        .expect("selection command succeeds");

        assert_eq!(workspace.active_chart, Some(initial));
        assert_eq!(workspace.selected_charts, vec![initial]);
    }

    #[test]
    fn close_repairs_required_inline_slot_to_neighbor() {
        let ids = bootstrap_ids();
        let (workspace_id, mut workspace) = workspace_fixture();
        let second = InstanceId::new();
        apply_workspace_command(
            workspace_id,
            &mut workspace,
            &Command::OpenSavedChart {
                workspace: workspace_id,
                definition: ids.chart_definition_b,
                instance_id: second,
            },
        )
        .expect("open command succeeds");
        apply_workspace_command(
            workspace_id,
            &mut workspace,
            &Command::CloseChart {
                workspace: workspace_id,
                instance_id: ids.chart_instance_a,
            },
        )
        .expect("close command succeeds");

        assert_eq!(workspace.active_chart, Some(second));
        assert_eq!(
            workspace.views[0]
                .charts
                .get(&ChartSlotId::new("radix").expect("slot ID")),
            Some(&second)
        );
    }
}
