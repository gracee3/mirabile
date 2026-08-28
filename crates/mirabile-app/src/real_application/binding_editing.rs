use super::{
    AnalysisProfile, AppError, AppErrorKind, AppResult, AspectSet, BoundPayload,
    CalculationRuntime, CanonicalResource, Catalog, PointSet, RealApplication, ResourceBinding,
    ResourceRepository, Theme, ViewDocument, WheelTemplate, info,
};
use crate::{WorkspaceBindingSelection, WorkspaceBindingSlot};

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
