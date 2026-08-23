use super::{
    AppError, AppErrorKind, AppIntent, AppResult, BTreeMap, CalculationRuntime, CanonicalResource,
    Command, InstanceId, RealApplication, RealState, ResourceId, ResourceRepository, Timestamp,
    ViewDocument, ViewInstanceId, WorkspaceDocument, apply_workspace_command, info, not_found,
    repository_app_error, resolve_typed_binding, success,
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) fn activate_session_chart(&self, instance_id: InstanceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        if !session.contains_chart(instance_id) {
            return Err(AppError::new(
                AppErrorKind::NotFound,
                format!("Chart instance {instance_id} is not open"),
            ));
        }
        session.active_chart = Some(instance_id);
        state.notice = Some(info("Active chart changed; selection was preserved"));
        state.advance()
    }

    pub(super) fn set_session_chart_selection(
        &self,
        instance_id: InstanceId,
        selected: bool,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        if !session.contains_chart(instance_id) {
            return Err(AppError::new(
                AppErrorKind::NotFound,
                format!("Chart instance {instance_id} is not open"),
            ));
        }
        if selected && !session.selected_charts.contains(&instance_id) {
            session.selected_charts.push(instance_id);
        } else if !selected {
            session.selected_charts.retain(|id| *id != instance_id);
        }
        state.notice = Some(info("Chart selection changed independently of activation"));
        state.advance()
    }

    pub(super) fn set_active_session_view(&self, view_id: ViewInstanceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        if !session.document.views.iter().any(|view| view.id == view_id) {
            return Err(AppError::new(
                AppErrorKind::NotFound,
                format!("View {view_id} was not found"),
            ));
        }
        session.active_view = Some(view_id);
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info("Active view changed and its projection is refreshing"));
        state.advance()
    }

    pub(super) fn dispatch_workspace_intent(&self, intent: &AppIntent) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let workspace_id = state
            .workspace
            .as_ref()
            .map(|workspace| workspace.id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Unavailable,
                    "The active session has no saved WorkspaceDocument backing",
                )
            })?;
        let document = state
            .session
            .as_ref()
            .map(|session| session.document.clone())
            .ok_or_else(|| {
                AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
            })?;
        let (command, refresh, clear_editor, notice) =
            state.command_for_intent(workspace_id, &document, intent)?;
        let view_documents = state.resolve_view_documents(&document)?;
        let session = state.session.as_mut().expect("session was checked");
        apply_workspace_command(workspace_id, session, &command, &view_documents)
            .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
        if clear_editor {
            state.editor = None;
        }
        state.ensure_view_runtimes();
        if refresh {
            self.submit_active_view_refresh(&mut state)?;
        }
        state.notice = Some(info(notice));
        state.advance()
    }

    pub(super) async fn save_workspace(&self) -> AppResult<()> {
        let (expected_revision, next) = {
            let state = self.state.borrow();
            let envelope = state.workspace.as_ref().ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Unavailable,
                    "The active session has no saved WorkspaceDocument backing",
                )
            })?;
            let session = state.session.as_ref().ok_or_else(|| {
                AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
            })?;
            if !session.document_dirty {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    "The WorkspaceDocument has no changes to save",
                ));
            }
            let next = envelope
                .next_with_payload(
                    session.document.clone(),
                    Timestamp::from_unix_millis(state.next_timestamp),
                )
                .map_err(|error| {
                    AppError::new(
                        AppErrorKind::InvalidIntent,
                        format!("WorkspaceDocument draft was invalid: {error}"),
                    )
                })?;
            (envelope.revision, next)
        };

        self.repository
            .save(
                expected_revision,
                CanonicalResource::WorkspaceDocument(next.clone()),
            )
            .await
            .map_err(|error| {
                repository_app_error("Could not save the WorkspaceDocument", &error)
            })?;

        let mut state = self.state.borrow_mut();
        state.next_timestamp = state.next_timestamp.saturating_add(1);
        state
            .catalog
            .insert_current(CanonicalResource::WorkspaceDocument(next.clone()));
        state.workspace = Some(next.clone());
        state
            .session
            .as_mut()
            .expect("ready application has a session")
            .mark_saved(next.id, next.revision);
        state.notice = Some(success("Workspace saved as a new canonical revision"));
        state.advance()
    }

    pub(super) fn set_temporary_point_hidden(
        &self,
        point_id: mirabile_core::PointId,
        hidden: bool,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let view_id = session
            .active_view
            .ok_or_else(|| AppError::new(AppErrorKind::Unavailable, "There is no active view"))?;
        let overrides = session.temporary_view_overrides.entry(view_id).or_default();
        if hidden && !overrides.hidden_points.contains(&point_id) {
            overrides.hidden_points.push(point_id);
        } else if !hidden {
            overrides.hidden_points.retain(|point| point != &point_id);
        }
        if overrides == &mirabile_core::ViewOverrides::default() {
            session.temporary_view_overrides.remove(&view_id);
        }
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Temporary display override changed for this session without dirtying the workspace",
        ));
        state.advance()
    }

    pub(super) fn promote_temporary_display(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let view_id = session
            .active_view
            .ok_or_else(|| AppError::new(AppErrorKind::Unavailable, "There is no active view"))?;
        let overrides = session
            .temporary_view_overrides
            .remove(&view_id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    "The active view has no temporary display override to promote",
                )
            })?;
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
        view.overrides = overrides;
        session.mark_document_dirty();
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Temporary display override promoted into the durable workspace projection; save the workspace to persist it",
        ));
        state.advance()
    }
}

impl RealState {
    pub(super) fn resolve_view_documents(
        &self,
        workspace: &WorkspaceDocument,
    ) -> AppResult<BTreeMap<ViewInstanceId, ViewDocument>> {
        workspace
            .views
            .iter()
            .map(|view| {
                resolve_typed_binding(&view.document, &self.catalog)
                    .map(|resolved| (view.id, resolved.value))
                    .map_err(|error| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!(
                                "ViewDocument for view {} could not be resolved: {error}",
                                view.id
                            ),
                        )
                    })
            })
            .collect()
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn command_for_intent(
        &self,
        workspace_id: ResourceId,
        workspace: &WorkspaceDocument,
        intent: &AppIntent,
    ) -> AppResult<(Command, bool, bool, &'static str)> {
        match intent {
            AppIntent::OpenChart { definition_id } => {
                if self.catalog.chart_definition(*definition_id).is_none() {
                    return Err(not_found("ChartDefinition", *definition_id));
                }
                Ok((
                    Command::OpenSavedChart {
                        workspace: workspace_id,
                        definition: *definition_id,
                        instance_id: InstanceId::new(),
                    },
                    false,
                    false,
                    "Chart opened in the working document and activated; save the workspace to persist membership",
                ))
            }
            AppIntent::CloseChart { instance_id } => Ok((
                Command::CloseChart {
                    workspace: workspace_id,
                    instance_id: *instance_id,
                },
                true,
                false,
                "Chart closed in the working document; selection and slots were repaired and the workspace is dirty",
            )),
            AppIntent::ActivateChart { instance_id } => Ok((
                Command::SetActiveChart {
                    workspace: workspace_id,
                    instance_id: Some(*instance_id),
                },
                false,
                false,
                "Active chart changed; selection was preserved",
            )),
            AppIntent::SetChartSelection {
                instance_id,
                selected,
            } => Ok((
                Command::SetChartSelection {
                    workspace: workspace_id,
                    instance_id: *instance_id,
                    selected: *selected,
                },
                false,
                false,
                "Chart selection changed independently of activation",
            )),
            AppIntent::SetActiveView { view_id } => Ok((
                Command::SetActiveView {
                    workspace: workspace_id,
                    view: Some(*view_id),
                },
                true,
                false,
                "Active view changed and its projection is refreshing",
            )),
            AppIntent::AssignChartSlot {
                view_id,
                slot,
                chart,
            } => {
                let view = workspace
                    .views
                    .iter()
                    .find(|view| view.id == *view_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("View {view_id} was not found"),
                        )
                    })?;
                let document = resolve_typed_binding(&view.document, &self.catalog)
                    .map_err(|error| AppError::new(AppErrorKind::NotFound, error.to_string()))?;
                let slot_definition = document
                    .value
                    .chart_slots
                    .iter()
                    .find(|candidate| candidate.id == *slot)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("Chart slot {slot} was not found"),
                        )
                    })?;
                if slot_definition.required && chart.is_none() {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "A required chart slot cannot be cleared",
                    ));
                }
                Ok((
                    Command::AssignChartSlot {
                        workspace: workspace_id,
                        view: *view_id,
                        slot: slot.clone(),
                        chart: *chart,
                    },
                    true,
                    false,
                    "Chart slot assignment changed in the working document; save the workspace to persist it",
                ))
            }
            AppIntent::SetWorkspaceAspectSet { resource_id } => {
                if self.catalog.aspect_set(*resource_id).is_none() {
                    return Err(not_found("Aspect Set", *resource_id));
                }
                Ok((
                    Command::SetWorkspaceAspectSet {
                        workspace: workspace_id,
                        aspect_set: *resource_id,
                    },
                    true,
                    true,
                    "Workspace Aspect Set binding changed; the workspace is dirty and analysis is refreshing",
                ))
            }
            AppIntent::StartChartDraft { .. }
            | AppIntent::SaveChartDraft { .. }
            | AppIntent::CancelChartDraft { .. }
            | AppIntent::BeginAspectSetEdit { .. }
            | AppIntent::UpdateAspectSetDraft(_)
            | AppIntent::SaveDraft
            | AppIntent::CancelDraft
            | AppIntent::SaveWorkspace
            | AppIntent::SetTemporaryPointHidden { .. }
            | AppIntent::PromoteTemporaryDisplay
            | AppIntent::RefreshActiveView => Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "The intent is not a workspace persistence command",
            )),
        }
    }
}
