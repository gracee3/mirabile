use super::{
    AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind, AppResult, BTreeMap,
    CalculationRuntime, CanonicalResource, Command, ConfigurationLayer, InstanceId, PendingWork,
    RealApplication, RealState, ResourceEnvelope, ResourceRepository, ResourceState, Timestamp,
    ViewDocument, ViewInstanceId, WorkspaceDocument, WorkspaceDocumentBacking, WorkspaceSession,
    WorkspaceSwitchDecisionReadModel, WorkspaceSwitchTarget, apply_workspace_command,
    current_transits_session, info, not_found, repository_app_error, resolve_typed_binding,
    success,
    validation::{validate_durable_document_references, validate_session_references},
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) fn rename_workspace(&self, title: &str) -> AppResult<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "A workspace title must not be empty",
            ));
        }
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        if session.working_title == title {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "The workspace title is unchanged",
            ));
        }
        session.working_title = title.into();
        session.mark_document_dirty();
        state.notice = Some(info(
            "Workspace title changed in the working session; save the workspace to publish it",
        ));
        state.advance()
    }

    pub(super) fn request_workspace_switch(&self, target: WorkspaceSwitchTarget) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if let WorkspaceSwitchTarget::Saved { resource_id } = target
            && state.catalog.workspace(resource_id).is_none()
        {
            return Err(not_found("WorkspaceDocument", resource_id));
        }
        let decision = state.workspace_switch_decision(target)?;
        if decision.reasons.is_empty() {
            self.activate_workspace_target(&mut state, target)?;
            state.notice = Some(info(
                "Workspace switched without discarding working changes",
            ));
        } else {
            state.workspace_switch = Some(decision);
            state.notice = Some(info(
                "Workspace switch needs an explicit Save, Discard, or Stay decision",
            ));
        }
        state.advance()
    }

    pub(super) fn resolve_workspace_switch(
        &self,
        action: crate::WorkspaceSwitchAction,
    ) -> AppResult<()> {
        let target = self
            .state
            .borrow()
            .workspace_switch
            .as_ref()
            .map(|decision| decision.target)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    "There is no pending workspace switch decision",
                )
            })?;
        let decision = self.state.borrow().workspace_switch_decision(target)?;
        match action {
            crate::WorkspaceSwitchAction::Stay => {
                let mut state = self.state.borrow_mut();
                state.workspace_switch = None;
                state.notice = Some(info("Stayed in the current workspace"));
                state.advance()
            }
            crate::WorkspaceSwitchAction::DiscardAndSwitch => {
                let mut state = self.state.borrow_mut();
                state.workspace_switch = None;
                self.activate_workspace_target(&mut state, decision.target)?;
                state.notice = Some(info(
                    "Working workspace state was explicitly discarded before switching",
                ));
                state.advance()
            }
            crate::WorkspaceSwitchAction::SaveAndSwitch => {
                if !decision.save_and_switch_enabled {
                    return Err(AppError::new(
                        AppErrorKind::Unavailable,
                        decision.save_and_switch_disabled_reason.unwrap_or_else(|| {
                            "Save and switch is unavailable until local editors are resolved".into()
                        }),
                    ));
                }
                {
                    let mut state = self.state.borrow_mut();
                    state.workspace_switch = None;
                    state.pending_workspace_switch = Some(decision.target);
                }
                if let Err(error) = self.begin_save_workspace() {
                    self.state.borrow_mut().pending_workspace_switch = None;
                    return Err(error);
                }
                Ok(())
            }
        }
    }

    pub(super) fn discard_workspace_changes(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if state.workspace_switch.is_some() {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "Use the projected workspace switch decision to Save, Discard, or Stay",
            ));
        }
        let (workspace, session) = state.workspace.clone().map_or_else(
            || {
                (
                    None,
                    current_transits_session((self.clock)(), self.startup_calculation_profile),
                )
            },
            |workspace| {
                let session = WorkspaceSession::from_saved(&workspace);
                (Some(workspace), session)
            },
        );
        state.workspace = workspace;
        state.session = Some(session);
        state.editor = None;
        state.chart_editor = None;
        state.workspace_switch = None;
        state.ensure_view_runtimes();
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Workspace changes were explicitly discarded and the saved or fresh session was restored",
        ));
        state.advance()
    }

    fn activate_workspace_target(
        &self,
        state: &mut RealState,
        target: WorkspaceSwitchTarget,
    ) -> AppResult<()> {
        let (workspace, session) = match target {
            WorkspaceSwitchTarget::New => (
                None,
                current_transits_session((self.clock)(), self.startup_calculation_profile),
            ),
            WorkspaceSwitchTarget::Saved { resource_id } => {
                let workspace = state
                    .catalog
                    .workspace(resource_id)
                    .cloned()
                    .ok_or_else(|| not_found("WorkspaceDocument", resource_id))?;
                let session = WorkspaceSession::from_saved(&workspace);
                (Some(workspace), session)
            }
        };
        state.workspace = workspace;
        state.session = Some(session);
        state.editor = None;
        state.chart_editor = None;
        state.workspace_switch = None;
        state.ensure_view_runtimes();
        self.submit_active_view_refresh(state)
    }

    pub(super) fn begin_load_demo_bundle(&self) -> AppResult<()> {
        let resources = match self.startup_calculation_profile {
            crate::StartupCalculationProfile::Baseline => crate::demo_resources(),
            #[cfg(feature = "xalen-backend")]
            crate::StartupCalculationProfile::ApparentPlace => {
                crate::apparent_place_demo_resources()
            }
        };
        let mut state = self.state.borrow_mut();
        state
            .pending
            .push_back(PendingWork::LoadDemoBundle { resources });
        state.notice = Some(info(
            "Checking stable demo identities before one idempotent atomic load",
        ));
        state.advance()
    }

    pub(super) async fn complete_demo_bundle_load(
        &self,
        resources: Vec<CanonicalResource>,
    ) -> AppResult<()> {
        let mut missing = Vec::new();
        for expected in resources {
            match self.repository.get_head(expected.id()).await {
                Ok(None) => missing.push(expected),
                Ok(Some(ResourceState::Present(existing)))
                    if existing.kind() == expected.kind() => {}
                Ok(Some(ResourceState::Present(existing))) => {
                    let mut state = self.state.borrow_mut();
                    state.notice = Some(AppNotice {
                        kind: AppNoticeKind::Warning,
                        message: format!(
                            "Demo identity {} is occupied by incompatible {:?} data",
                            expected.id(),
                            existing.kind()
                        ),
                    });
                    return state.advance();
                }
                Ok(Some(ResourceState::Deleted(_))) => {
                    let mut state = self.state.borrow_mut();
                    state.notice = Some(AppNotice {
                        kind: AppNoticeKind::Warning,
                        message: format!(
                            "Demo identity {} was deleted and cannot be reused",
                            expected.id()
                        ),
                    });
                    return state.advance();
                }
                Err(error) => {
                    let failure = repository_app_error("Could not inspect demo identities", &error);
                    let mut state = self.state.borrow_mut();
                    state.notice = Some(AppNotice {
                        kind: AppNoticeKind::Warning,
                        message: failure.message,
                    });
                    return state.advance();
                }
            }
        }
        if let Err(error) = if missing.is_empty() {
            Ok(())
        } else {
            self.repository.create_batch(missing.clone()).await
        } {
            let failure = repository_app_error("Could not atomically load the demo bundle", &error);
            let mut state = self.state.borrow_mut();
            state.notice = Some(AppNotice {
                kind: AppNoticeKind::Warning,
                message: failure.message,
            });
            return state.advance();
        }
        let created = missing.len();
        let mut state = self.state.borrow_mut();
        for resource in missing {
            state.catalog.insert_current(resource);
        }
        state.notice = Some(success(if created == 0 {
            "Demo bundle is already present; existing revisions were left untouched"
        } else {
            "Missing demo resources were created atomically; existing revisions were left untouched"
        }));
        state.advance()
    }
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
        let document = state
            .session
            .as_ref()
            .map(|session| session.document.clone())
            .ok_or_else(|| {
                AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
            })?;
        let (command, refresh, clear_editor, notice) =
            state.command_for_intent(&document, intent)?;
        let view_documents = state.resolve_view_documents(&document)?;
        let mut next_session = state.session.clone().expect("session was checked");
        apply_workspace_command(&mut next_session, &command, &view_documents)
            .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
        validate_session_references(&next_session, &state.catalog).map_err(|error| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                format!("Workspace command failed referential validation: {error}"),
            )
        })?;
        state.session = Some(next_session);
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

    pub(super) fn begin_save_workspace(&self) -> AppResult<()> {
        let (expected_revision, next) = {
            let state = self.state.borrow();
            if state.workspace_switch.is_some() {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    "Use Save and switch from the projected workspace switch decision",
                ));
            }
            let session = state.session.as_ref().ok_or_else(|| {
                AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
            })?;
            validate_durable_document_references(&session.document, &state.catalog).map_err(
                |error| {
                    AppError::new(
                        AppErrorKind::InvalidIntent,
                        format!(
                            "WorkspaceDocument cannot be saved because its durable references are invalid: {error}"
                        ),
                    )
                },
            )?;
            let timestamp = Timestamp::from_unix_millis(state.next_timestamp);
            match session.backing {
                WorkspaceDocumentBacking::Unsaved => (
                    None,
                    ResourceEnvelope::new(
                        session.working_title.clone(),
                        session.document.clone(),
                        timestamp,
                    ),
                ),
                WorkspaceDocumentBacking::Saved {
                    document_id,
                    revision,
                } => {
                    if !session.document_dirty {
                        return Err(AppError::new(
                            AppErrorKind::InvalidIntent,
                            "The WorkspaceDocument has no changes to save",
                        ));
                    }
                    let envelope = state.workspace.as_ref().filter(|workspace| {
                        workspace.id == document_id && workspace.revision == revision
                    }).ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::Unavailable,
                            "The saved WorkspaceDocument backing does not match the active session",
                        )
                    })?;
                    let mut next = envelope
                        .next_with_payload(session.document.clone(), timestamp)
                        .map_err(|error| {
                            AppError::new(
                                AppErrorKind::InvalidIntent,
                                format!("WorkspaceDocument draft was invalid: {error}"),
                            )
                        })?;
                    next.title.clone_from(&session.working_title);
                    (Some(envelope.revision), next)
                }
            }
        };

        let mut state = self.state.borrow_mut();
        state.pending.push_back(super::PendingWork::SaveWorkspace {
            expected_revision,
            next: Box::new(next),
        });
        state.notice = Some(info(
            "Saving the WorkspaceDocument as an observable revision-checked operation",
        ));
        state.advance()
    }

    pub(super) async fn complete_workspace_save(
        &self,
        expected_revision: Option<mirabile_core::Revision>,
        next: ResourceEnvelope<WorkspaceDocument>,
    ) -> AppResult<()> {
        let resource = CanonicalResource::WorkspaceDocument(next.clone());
        let result = match expected_revision {
            Some(expected_revision) => self.repository.save(expected_revision, resource).await,
            None => self.repository.create(resource).await,
        };
        if let Err(error) = result {
            let failure = repository_app_error("Could not save the WorkspaceDocument", &error);
            let mut state = self.state.borrow_mut();
            state.pending_workspace_switch = None;
            state.notice = Some(super::AppNotice {
                kind: if failure.kind == AppErrorKind::Conflict {
                    super::AppNoticeKind::Conflict
                } else {
                    super::AppNoticeKind::Warning
                },
                message: failure.message,
            });
            return state.advance();
        }

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
        let switch_target = state.pending_workspace_switch.take();
        if let Some(target) = switch_target {
            self.activate_workspace_target(&mut state, target)?;
            state.notice = Some(success(
                "Workspace saved successfully and the requested workspace switch completed",
            ));
        } else {
            state.notice = Some(success(if expected_revision.is_some() {
                "Workspace saved as a new canonical revision"
            } else {
                "Workspace saved as canonical revision one"
            }));
        }
        state.advance()
    }

    pub(super) fn set_temporary_point_hidden(
        &self,
        point_id: mirabile_core::PointId,
        hidden: bool,
    ) -> AppResult<()> {
        let supported = self
            .engine
            .backend_descriptor()
            .capabilities
            .celestial
            .as_ref()
            .is_some_and(|capabilities| capabilities.supported_points.contains(&point_id));
        if !supported {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                format!(
                    "Point {} is not supported by the active calculation provider",
                    point_id.as_str()
                ),
            ));
        }
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let view_id = session
            .active_view
            .ok_or_else(|| AppError::new(AppErrorKind::Unavailable, "There is no active view"))?;
        let durable = session
            .document
            .views
            .iter()
            .find(|view| view.id == view_id)
            .map(|view| view.overrides.clone())
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("View {view_id} was not found"),
                )
            })?;
        let current_hidden = session
            .temporary_view_overrides
            .get(&view_id)
            .unwrap_or(&durable)
            .hidden_points
            .contains(&point_id);
        if current_hidden == hidden {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                format!(
                    "Point {} visibility already has the requested value",
                    point_id.as_str()
                ),
            ));
        }
        let overrides = session
            .temporary_view_overrides
            .entry(view_id)
            .or_insert_with(|| durable.clone());
        if hidden && !overrides.hidden_points.contains(&point_id) {
            overrides.hidden_points.push(point_id);
        } else if !hidden {
            overrides.hidden_points.retain(|point| point != &point_id);
        }
        if overrides == &durable {
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
    pub(super) fn workspace_switch_decision(
        &self,
        target: WorkspaceSwitchTarget,
    ) -> AppResult<WorkspaceSwitchDecisionReadModel> {
        let session = self.session()?;
        let mut reasons = Vec::new();
        if session.document_dirty {
            reasons.push("The workspace document or title has unsaved changes".into());
        }
        if !session.draft_charts.is_empty() {
            reasons.push("Unsaved chart drafts must be saved or canceled explicitly".into());
        }
        let chart_editor_dirty = self.chart_editor.as_ref().is_some_and(|editor| {
            matches!(editor.target, crate::ChartEditorTarget::New { .. })
                || editor.state != crate::ChartEditorState::Clean
        });
        if chart_editor_dirty {
            reasons.push("The chart editor has unresolved local work".into());
        }
        let resource_editor_dirty = self
            .editor
            .as_ref()
            .is_some_and(|editor| !matches!(editor.state, crate::DraftState::Clean { .. }));
        if resource_editor_dirty {
            reasons.push("The resource editor has unresolved local work".into());
        }
        if !session.temporary_view_overrides.is_empty() {
            reasons.push("Temporary display state would be discarded".into());
        }
        let blockers = !session.draft_charts.is_empty()
            || chart_editor_dirty
            || resource_editor_dirty
            || !session.temporary_view_overrides.is_empty();
        let workspace_can_be_saved =
            session.document_dirty || matches!(session.backing, WorkspaceDocumentBacking::Unsaved);
        let save_and_switch_enabled = workspace_can_be_saved && !blockers;
        let save_and_switch_disabled_reason = (!save_and_switch_enabled).then(|| {
            if blockers {
                "Resolve chart drafts, editors, and temporary display state before Save and switch"
                    .into()
            } else {
                "The workspace has no unsaved canonical changes to save".into()
            }
        });
        Ok(WorkspaceSwitchDecisionReadModel {
            target,
            reasons,
            save_and_switch_enabled,
            save_and_switch_disabled_reason,
        })
    }

    pub(super) fn resolve_view_documents(
        &self,
        workspace: &WorkspaceDocument,
    ) -> AppResult<BTreeMap<ViewInstanceId, ViewDocument>> {
        workspace
            .views
            .iter()
            .map(|view| {
                resolve_typed_binding(&view.document, &self.catalog, ConfigurationLayer::View)
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
                    instance_id: *instance_id,
                },
                true,
                false,
                "Chart closed in the working document; selection and slots were repaired and the workspace is dirty",
            )),
            AppIntent::ActivateChart { instance_id } => Ok((
                Command::SetActiveChart {
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
                    instance_id: *instance_id,
                    selected: *selected,
                },
                false,
                false,
                "Chart selection changed independently of activation",
            )),
            AppIntent::SetActiveView { view_id } => Ok((
                Command::SetActiveView {
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
                let document =
                    resolve_typed_binding(&view.document, &self.catalog, ConfigurationLayer::View)
                        .map_err(|error| {
                            AppError::new(AppErrorKind::NotFound, error.to_string())
                        })?;
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
                let notice = if chart.is_some_and(|chart| {
                    self.session
                        .as_ref()
                        .is_some_and(|session| session.contains_draft_chart(chart))
                }) {
                    "Draft chart assigned as a session-only preview; save the chart to promote the assignment"
                } else {
                    "Saved chart slot assignment changed in the working document; save the workspace to persist it"
                };
                Ok((
                    Command::AssignChartSlot {
                        view: *view_id,
                        slot: slot.clone(),
                        chart: *chart,
                    },
                    true,
                    false,
                    notice,
                ))
            }
            AppIntent::SetWorkspaceAspectSet { resource_id } => {
                if self.catalog.aspect_set(*resource_id).is_none() {
                    return Err(not_found("Aspect Set", *resource_id));
                }
                Ok((
                    Command::SetWorkspaceAspectSet {
                        aspect_set: *resource_id,
                    },
                    true,
                    true,
                    "Workspace Aspect Set binding changed; the workspace is dirty and analysis is refreshing",
                ))
            }
            AppIntent::BeginNewChart
            | AppIntent::BeginSavedChartEdit { .. }
            | AppIntent::ApplyChartMutation(_)
            | AppIntent::SaveChartEditor
            | AppIntent::CancelChartEditor
            | AppIntent::StartChartDraft { .. }
            | AppIntent::SaveChartDraft { .. }
            | AppIntent::CancelChartDraft { .. }
            | AppIntent::BeginAspectSetEdit { .. }
            | AppIntent::BeginNewAspectSet
            | AppIntent::DuplicateAspectSet { .. }
            | AppIntent::SelectRepositoryResource { .. }
            | AppIntent::BeginResourceEdit { .. }
            | AppIntent::BeginResourceCreate { .. }
            | AppIntent::ApplyResourceMutation(_)
            | AppIntent::SaveResourceDraft { .. }
            | AppIntent::CancelResourceDraft { .. }
            | AppIntent::UpdateAspectSetDraft(_)
            | AppIntent::SaveDraft
            | AppIntent::CancelDraft
            | AppIntent::SaveWorkspace
            | AppIntent::NewWorkspace
            | AppIntent::OpenWorkspace { .. }
            | AppIntent::RenameWorkspace { .. }
            | AppIntent::DiscardWorkspaceChanges
            | AppIntent::ResolveWorkspaceSwitch { .. }
            | AppIntent::LoadDemoBundle
            | AppIntent::SetTemporaryPointHidden { .. }
            | AppIntent::PromoteTemporaryDisplay
            | AppIntent::RefreshActiveView => Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "The intent is not a workspace persistence command",
            )),
        }
    }
}
