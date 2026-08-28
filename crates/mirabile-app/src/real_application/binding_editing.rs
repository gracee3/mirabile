use super::{
    AnalysisProfile, AppError, AppErrorKind, AppResult, AspectSet, BoundPayload,
    CalculationRuntime, CanonicalResource, Catalog, ConfigurationLayer, PointSet, RealApplication,
    ResourceBinding, ResourceRepository, Theme, ViewDocument, ViewInstance, ViewInstanceId,
    WheelTemplate, info, resolve_typed_binding,
};
use crate::{
    WorkspaceBindingSelection, WorkspaceBindingSlot, WorkspaceCompositionMutation,
    real_application::validation::validate_session_references,
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) fn set_workspace_binding(
        &self,
        slot: WorkspaceBindingSlot,
        selection: WorkspaceBindingSelection,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let mut session = state.session.clone().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        match slot {
            WorkspaceBindingSlot::DisplayedPoints => {
                session.document.profile.displayed_points =
                    selected_binding::<PointSet>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::AspectedPoints => {
                session.document.profile.aspected_points =
                    selected_binding::<PointSet>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::TransitPoints => {
                session.document.profile.transit_points =
                    selected_binding::<PointSet>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::Aspects => {
                session.document.profile.aspects =
                    selected_binding::<AspectSet>(&state.catalog, selection)?;
                state.editor = None;
            }
            WorkspaceBindingSlot::Analysis => {
                session.document.profile.analysis =
                    selected_binding::<AnalysisProfile>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::Theme => {
                session.document.profile.theme =
                    selected_binding::<Theme>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::Wheel => {
                session.document.profile.wheel =
                    selected_binding::<WheelTemplate>(&state.catalog, selection)?;
            }
            WorkspaceBindingSlot::ViewDocument { view_id } => {
                let view = session
                    .document
                    .views
                    .iter_mut()
                    .find(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                view.document = selected_binding::<ViewDocument>(&state.catalog, selection)?;
            }
        }
        session.document_dirty = true;
        state.session = Some(session);
        state.ensure_view_runtimes();
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Workspace binding mode changed; save the workspace to persist it",
        ));
        state.advance()
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn apply_workspace_composition(
        &self,
        mutation: WorkspaceCompositionMutation,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let mut session = state.session.clone().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let mut refresh = false;

        match mutation {
            WorkspaceCompositionMutation::MoveChart {
                instance_id,
                before,
            } => {
                move_item(
                    &mut session.document.chart_instances,
                    |chart| chart.instance_id == instance_id,
                    |chart| before.is_some_and(|before| chart.instance_id == before),
                    "chart instance",
                    instance_id,
                )?;
            }
            WorkspaceCompositionMutation::AddView { document } => {
                let document = selected_binding::<ViewDocument>(&state.catalog, document)?;
                let resolved =
                    resolve_typed_binding(&document, &state.catalog, ConfigurationLayer::View)
                        .map_err(|error| {
                            AppError::new(
                                AppErrorKind::InvalidIntent,
                                format!("The selected ViewDocument could not be resolved: {error}"),
                            )
                        })?;
                let view_id = ViewInstanceId::new();
                let saved_chart = session
                    .document
                    .chart_instances
                    .first()
                    .map(|chart| chart.instance_id);
                let draft_chart = session.draft_charts.first().map(|chart| chart.instance_id);
                let mut charts = std::collections::BTreeMap::new();
                let mut draft_assignments = std::collections::BTreeMap::new();
                for slot in resolved
                    .value
                    .chart_slots
                    .iter()
                    .filter(|slot| slot.required)
                {
                    if let Some(instance_id) = saved_chart {
                        charts.insert(slot.id.clone(), instance_id);
                    } else if let Some(instance_id) = draft_chart {
                        draft_assignments.insert(slot.id.clone(), instance_id);
                    }
                }
                session.document.views.push(ViewInstance {
                    id: view_id,
                    document,
                    charts,
                    overrides: mirabile_core::ViewOverrides::default(),
                });
                if !draft_assignments.is_empty() {
                    session
                        .draft_chart_assignments
                        .insert(view_id, draft_assignments);
                }
                session.active_view = Some(view_id);
                refresh = true;
            }
            WorkspaceCompositionMutation::RemoveView { view_id } => {
                let index = session
                    .document
                    .views
                    .iter()
                    .position(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                session.document.views.remove(index);
                session.draft_chart_assignments.remove(&view_id);
                session.temporary_view_overrides.remove(&view_id);
                if session.active_view == Some(view_id) {
                    session.active_view = session
                        .document
                        .views
                        .get(index)
                        .or_else(|| session.document.views.last())
                        .map(|view| view.id);
                    refresh = session.active_view.is_some();
                }
            }
            WorkspaceCompositionMutation::MoveView { view_id, before } => {
                move_item(
                    &mut session.document.views,
                    |view| view.id == view_id,
                    |view| before.is_some_and(|before| view.id == before),
                    "view",
                    view_id,
                )?;
            }
            WorkspaceCompositionMutation::SetRotation { view_id, rotation } => {
                let view = session
                    .document
                    .views
                    .iter_mut()
                    .find(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                view.overrides.rotation = rotation;
                refresh = session.active_view == Some(view_id);
            }
            WorkspaceCompositionMutation::SetPointHidden {
                view_id,
                point_id,
                hidden,
            } => {
                let view = session
                    .document
                    .views
                    .iter_mut()
                    .find(|view| view.id == view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                if hidden && !view.overrides.hidden_points.contains(&point_id) {
                    view.overrides.hidden_points.push(point_id);
                    view.overrides.hidden_points.sort();
                } else if !hidden {
                    view.overrides
                        .hidden_points
                        .retain(|point| point != &point_id);
                }
                refresh = session.active_view == Some(view_id);
            }
        }

        session.document_dirty = true;
        validate_session_references(&session, &state.catalog).map_err(|error| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                format!("Workspace composition failed referential validation: {error}"),
            )
        })?;
        state.session = Some(session);
        state.ensure_view_runtimes();
        if refresh {
            self.submit_active_view_refresh(&mut state)?;
        }
        state.notice = Some(info(
            "Workspace composition changed; save the workspace to persist it",
        ));
        state.advance()
    }
}

fn move_item<T, F, B, I>(
    items: &mut Vec<T>,
    is_item: F,
    is_before: B,
    label: &str,
    identity: I,
) -> AppResult<()>
where
    F: Fn(&T) -> bool,
    B: Fn(&T) -> bool,
    I: std::fmt::Display,
{
    let source = items.iter().position(is_item).ok_or_else(|| {
        AppError::new(
            AppErrorKind::NotFound,
            format!("{label} {identity} was not found"),
        )
    })?;
    let item = items.remove(source);
    let destination = items.iter().position(is_before).unwrap_or(items.len());
    items.insert(destination, item);
    Ok(())
}

fn selected_binding<T: BoundPayload>(
    catalog: &Catalog,
    selection: WorkspaceBindingSelection,
) -> AppResult<ResourceBinding<T>> {
    match selection {
        WorkspaceBindingSelection::Follow { resource_id } => {
            require_payload::<T>(catalog.current.get(&resource_id), resource_id, None)?;
            Ok(ResourceBinding::Follow { id: resource_id })
        }
        WorkspaceBindingSelection::Pinned {
            resource_id,
            revision,
        } => {
            require_payload::<T>(
                catalog.history.get(&(resource_id, revision)),
                resource_id,
                Some(revision),
            )?;
            Ok(ResourceBinding::Pinned {
                id: resource_id,
                revision,
            })
        }
        WorkspaceBindingSelection::Inline { resource_id } => {
            let envelope =
                require_payload::<T>(catalog.current.get(&resource_id), resource_id, None)?;
            Ok(ResourceBinding::Inline {
                value: envelope.payload.clone(),
            })
        }
    }
}

fn require_payload<T: BoundPayload>(
    resource: Option<&CanonicalResource>,
    resource_id: crate::ResourceId,
    revision: Option<crate::Revision>,
) -> AppResult<&mirabile_core::ResourceEnvelope<T>> {
    resource.and_then(T::envelope).ok_or_else(|| {
        AppError::new(
            AppErrorKind::NotFound,
            revision.map_or_else(
                || format!("Compatible resource {resource_id} was not found"),
                |revision| {
                    format!("Compatible resource {resource_id} revision {revision} was not found")
                },
            ),
        )
    })
}
