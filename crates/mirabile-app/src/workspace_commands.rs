use std::collections::BTreeMap;

use mirabile_core::{
    Command, DomainValidate, InstanceId, ResourceBinding, ViewDocument, ViewInstanceId,
    WorkspaceDocumentChart,
};
use thiserror::Error;

use crate::WorkspaceSession;

#[allow(clippy::too_many_lines)]
pub(crate) fn apply_workspace_command(
    session: &mut WorkspaceSession,
    command: &Command,
    view_documents: &BTreeMap<ViewInstanceId, ViewDocument>,
) -> Result<(), WorkspaceCommandError> {
    match command {
        Command::OpenSavedChart {
            definition,
            instance_id,
        } => {
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
        Command::CloseChart { instance_id } => {
            let index = session
                .document
                .chart_instances
                .iter()
                .position(|chart| chart.instance_id() == *instance_id)
                .ok_or(WorkspaceCommandError::ChartNotOpen(*instance_id))?;
            let was_active = session.active_chart == Some(*instance_id);
            session.document.chart_instances.remove(index);
            session.selected_charts.retain(|id| id != instance_id);
            let durable_replacement = session
                .document
                .chart_instances
                .get(index)
                .or_else(|| {
                    index
                        .checked_sub(1)
                        .and_then(|prior| session.document.chart_instances.get(prior))
                })
                .map(|chart| chart.instance_id);
            if was_active {
                session.active_chart = durable_replacement
                    .or_else(|| session.draft_charts.first().map(|chart| chart.instance_id));
            }
            let draft_replacement = session
                .active_chart
                .filter(|chart| session.contains_draft_chart(*chart));
            let mut draft_repairs = Vec::new();
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
                        if let Some(replacement) = durable_replacement {
                            view.charts.insert(slot, replacement);
                        } else {
                            view.charts.remove(&slot);
                            if let Some(replacement) = draft_replacement {
                                draft_repairs.push((view.id, slot, replacement));
                            }
                        }
                    } else {
                        view.charts.remove(&slot);
                    }
                }
            }
            for (view_id, slot, replacement) in draft_repairs {
                session
                    .draft_chart_assignments
                    .entry(view_id)
                    .or_default()
                    .insert(slot, replacement);
            }
            session.remove_draft_assignments(*instance_id);
            session.mark_document_dirty();
        }
        Command::SetActiveChart { instance_id } => {
            if let Some(instance_id) = instance_id {
                ensure_chart_open(session, *instance_id)?;
            }
            session.active_chart = *instance_id;
        }
        Command::SetChartSelection {
            instance_id,
            selected,
        } => {
            ensure_chart_open(session, *instance_id)?;
            if *selected && !session.selected_charts.contains(instance_id) {
                session.selected_charts.push(*instance_id);
            } else if !selected {
                session.selected_charts.retain(|id| id != instance_id);
            }
        }
        Command::SetActiveView { view } => {
            if let Some(view) = view {
                ensure_view_exists(session, *view)?;
            }
            session.active_view = *view;
        }
        Command::AssignChartSlot { view, slot, chart } => {
            if let Some(chart) = chart {
                ensure_chart_open(session, *chart)?;
            }
            let is_draft = chart.is_some_and(|chart| session.contains_draft_chart(chart));
            let view_document = session
                .document
                .views
                .iter()
                .find(|candidate| candidate.id == *view)
                .ok_or(WorkspaceCommandError::ViewNotFound(*view))?;
            let document = view_documents.get(&view_document.id).ok_or(
                WorkspaceCommandError::ViewDocumentNotResolved(view_document.id),
            )?;
            let slot_definition = document
                .chart_slots
                .iter()
                .find(|candidate| candidate.id == *slot)
                .ok_or_else(|| WorkspaceCommandError::SlotNotFound(slot.to_string()))?;
            if slot_definition.required && chart.is_none() {
                return Err(WorkspaceCommandError::RequiredSlotCannotBeCleared(
                    slot.to_string(),
                ));
            }
            if is_draft {
                session
                    .draft_chart_assignments
                    .entry(*view)
                    .or_default()
                    .insert(slot.clone(), chart.expect("draft assignment has a chart"));
            } else {
                if let Some(assignments) = session.draft_chart_assignments.get_mut(view) {
                    assignments.remove(slot);
                    if assignments.is_empty() {
                        session.draft_chart_assignments.remove(view);
                    }
                }
                let document_view = session
                    .document
                    .views
                    .iter_mut()
                    .find(|candidate| candidate.id == *view)
                    .expect("view was checked");
                let changed = if let Some(chart) = chart {
                    document_view.charts.insert(slot.clone(), *chart) != Some(*chart)
                } else {
                    document_view.charts.remove(slot).is_some()
                };
                if changed {
                    session.mark_document_dirty();
                }
            }
        }
        Command::SetWorkspaceAspectSet { aspect_set } => {
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
    #[error("chart instance {0} is not open")]
    ChartNotOpen(InstanceId),
    #[error("view {0} was not found")]
    ViewNotFound(ViewInstanceId),
    #[error("the ViewDocument for view {0} was not resolved")]
    ViewDocumentNotResolved(ViewInstanceId),
    #[error("chart slot {0} was not found")]
    SlotNotFound(String),
    #[error("required chart slot {0} cannot be cleared")]
    RequiredSlotCannotBeCleared(String),
    #[error("workspace command produced invalid state: {0}")]
    InvalidWorkspace(String),
    #[error("resource persistence command was sent to the workspace handler")]
    NotWorkspaceCommand,
}

#[cfg(test)]
mod tests {
    use mirabile_core::{ChartSlotId, Command, ResourceBinding, WorkspaceDocument};

    use crate::{demo_ids, demo_resources};

    use super::*;

    fn workspace_fixture() -> WorkspaceSession {
        let resource = demo_resources()
            .into_iter()
            .find(|resource| {
                matches!(
                    resource,
                    mirabile_core::CanonicalResource::WorkspaceDocument(_)
                )
            })
            .expect("demo workspace exists");
        let mirabile_core::CanonicalResource::WorkspaceDocument(envelope) = resource else {
            unreachable!()
        };
        WorkspaceSession::from_saved(&envelope)
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
        let mut session = workspace_fixture();
        let initial = session.active_chart.expect("active chart");
        let view_documents = inline_view_documents(&session.document);
        apply_workspace_command(
            &mut session,
            &Command::SetChartSelection {
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
        let ids = demo_ids();
        let mut session = workspace_fixture();
        let view_documents = inline_view_documents(&session.document);
        let second = InstanceId::new();
        apply_workspace_command(
            &mut session,
            &Command::OpenSavedChart {
                definition: ids.chart_definition_b,
                instance_id: second,
            },
            &view_documents,
        )
        .expect("open command succeeds");
        apply_workspace_command(
            &mut session,
            &Command::CloseChart {
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
