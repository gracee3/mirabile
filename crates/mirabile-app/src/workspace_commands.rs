use std::collections::BTreeMap;

use mirabile_core::{
    Command, DomainValidate, InstanceId, ResourceBinding, ResourceId, ViewDocument, ViewInstanceId,
    WorkspaceDocumentChart,
};
use thiserror::Error;

use crate::WorkspaceSession;

#[allow(clippy::too_many_lines)]
pub(crate) fn apply_workspace_command(
    workspace_id: ResourceId,
    session: &mut WorkspaceSession,
    command: &Command,
    view_documents: &BTreeMap<ViewInstanceId, ViewDocument>,
) -> Result<(), WorkspaceCommandError> {
    match command {
        Command::OpenSavedChart {
            workspace: target,
            definition,
            instance_id,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(existing) = session
                .document
                .chart_instances
                .iter()
                .find(|chart| chart.definition == *definition)
                .map(|chart| chart.instance_id)
            {
                session.active_chart = Some(existing);
            } else {
                session
                    .document
                    .chart_instances
                    .push(WorkspaceDocumentChart {
                        instance_id: *instance_id,
                        definition: *definition,
                    });
                session.active_chart = Some(*instance_id);
                session.mark_document_dirty();
            }
        }
        Command::CloseChart {
            workspace: target,
            instance_id,
        } => {
            ensure_workspace(workspace_id, *target)?;
            let index = session
                .document
                .chart_instances
                .iter()
                .position(|chart| chart.instance_id() == *instance_id)
                .ok_or(WorkspaceCommandError::ChartNotOpen(*instance_id))?;
            let was_active = session.active_chart == Some(*instance_id);
            session.document.chart_instances.remove(index);
            session.selected_charts.retain(|id| id != instance_id);
            if was_active {
                session.active_chart = session
                    .document
                    .chart_instances
                    .get(index)
                    .or_else(|| {
                        index
                            .checked_sub(1)
                            .and_then(|prior| session.document.chart_instances.get(prior))
                    })
                    .map(|chart| chart.instance_id);
            }
            let replacement = session.active_chart;
            for view in &mut session.document.views {
                let document = view_documents
                    .get(&view.id)
                    .ok_or(WorkspaceCommandError::ViewDocumentNotResolved(view.id))?;
                let affected = view
                    .charts
                    .iter()
                    .filter_map(|(slot, chart)| (*chart == *instance_id).then_some(slot.clone()))
                    .collect::<Vec<_>>();
                for slot in affected {
                    if document
                        .chart_slots
                        .iter()
                        .any(|definition| definition.id == slot && definition.required)
                    {
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
            session.mark_document_dirty();
        }
        Command::SetActiveChart {
            workspace: target,
            instance_id,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(instance_id) = instance_id {
                ensure_chart_open(session, *instance_id)?;
            }
            session.active_chart = *instance_id;
        }
        Command::SetChartSelection {
            workspace: target,
            instance_id,
            selected,
        } => {
            ensure_workspace(workspace_id, *target)?;
            ensure_chart_open(session, *instance_id)?;
            if *selected && !session.selected_charts.contains(instance_id) {
                session.selected_charts.push(*instance_id);
            } else if !selected {
                session.selected_charts.retain(|id| id != instance_id);
            }
        }
        Command::SetActiveView {
            workspace: target,
            view,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(view) = view {
                ensure_view_exists(session, *view)?;
            }
            session.active_view = *view;
        }
        Command::AssignChartSlot {
            workspace: target,
            view,
            slot,
            chart,
        } => {
            ensure_workspace(workspace_id, *target)?;
            if let Some(chart) = chart {
                ensure_chart_open(session, *chart)?;
            }
            let view = session
                .document
                .views
                .iter_mut()
                .find(|candidate| candidate.id == *view)
                .ok_or(WorkspaceCommandError::ViewNotFound(*view))?;
            let document = view_documents
                .get(&view.id)
                .ok_or(WorkspaceCommandError::ViewDocumentNotResolved(view.id))?;
            if !document
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
            session.mark_document_dirty();
        }
        Command::SetWorkspaceAspectSet {
            workspace: target,
            aspect_set,
        } => {
            ensure_workspace(workspace_id, *target)?;
            session.document.profile.aspects = ResourceBinding::Follow { id: *aspect_set };
            session.mark_document_dirty();
        }
        Command::CreateResource { .. } | Command::SaveResourceDraft { .. } => {
            return Err(WorkspaceCommandError::NotWorkspaceCommand);
        }
    }
    session
        .document
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
    session: &WorkspaceSession,
    instance_id: InstanceId,
) -> Result<(), WorkspaceCommandError> {
    session
        .contains_chart(instance_id)
        .then_some(())
        .ok_or(WorkspaceCommandError::ChartNotOpen(instance_id))
}

fn ensure_view_exists(
    session: &WorkspaceSession,
    view_id: ViewInstanceId,
) -> Result<(), WorkspaceCommandError> {
    session
        .document
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
    #[error("view {0} was not found")]
    ViewNotFound(ViewInstanceId),
    #[error("the ViewDocument for view {0} was not resolved")]
    ViewDocumentNotResolved(ViewInstanceId),
    #[error("chart slot {0} was not found")]
    SlotNotFound(String),
    #[error("workspace command produced invalid state: {0}")]
    InvalidWorkspace(String),
    #[error("resource persistence command was sent to the workspace handler")]
    NotWorkspaceCommand,
}

#[cfg(test)]
mod tests {
    use mirabile_core::{ChartSlotId, Command, ResourceBinding, WorkspaceDocument};

    use crate::{bootstrap_ids, bootstrap_resources};

    use super::*;

    fn workspace_fixture() -> (ResourceId, WorkspaceSession) {
        let resource = bootstrap_resources()
            .into_iter()
            .find(|resource| {
                matches!(
                    resource,
                    mirabile_core::CanonicalResource::WorkspaceDocument(_)
                )
            })
            .expect("bootstrap workspace exists");
        let mirabile_core::CanonicalResource::WorkspaceDocument(envelope) = resource else {
            unreachable!()
        };
        let session = WorkspaceSession::from_saved(&envelope);
        (envelope.id, session)
    }

    fn inline_view_documents(
        workspace: &WorkspaceDocument,
    ) -> BTreeMap<ViewInstanceId, ViewDocument> {
        workspace
            .views
            .iter()
            .map(|view| {
                let ResourceBinding::Inline { value } = &view.document else {
                    panic!("workspace fixture uses an inline ViewDocument")
                };
                (view.id, value.clone())
            })
            .collect()
    }

    #[test]
    fn activation_and_selection_remain_independent() {
        let (workspace_id, mut session) = workspace_fixture();
        let initial = session.active_chart.expect("active chart");
        let view_documents = inline_view_documents(&session.document);
        apply_workspace_command(
            workspace_id,
            &mut session,
            &Command::SetChartSelection {
                workspace: workspace_id,
                instance_id: initial,
                selected: true,
            },
            &view_documents,
        )
        .expect("selection command succeeds");

        assert_eq!(session.active_chart, Some(initial));
        assert_eq!(session.selected_charts, vec![initial]);
        assert!(!session.document_dirty);
    }

    #[test]
    fn close_repairs_required_inline_slot_to_neighbor() {
        let ids = bootstrap_ids();
        let (workspace_id, mut session) = workspace_fixture();
        let view_documents = inline_view_documents(&session.document);
        let second = InstanceId::new();
        apply_workspace_command(
            workspace_id,
            &mut session,
            &Command::OpenSavedChart {
                workspace: workspace_id,
                definition: ids.chart_definition_b,
                instance_id: second,
            },
            &view_documents,
        )
        .expect("open command succeeds");
        apply_workspace_command(
            workspace_id,
            &mut session,
            &Command::CloseChart {
                workspace: workspace_id,
                instance_id: ids.chart_instance_a,
            },
            &view_documents,
        )
        .expect("close command succeeds");

        assert_eq!(session.active_chart, Some(second));
        assert!(session.document_dirty);
        assert_eq!(
            session.document.views[0]
                .charts
                .get(&ChartSlotId::new("radix").expect("slot ID")),
            Some(&second)
        );
    }
}
