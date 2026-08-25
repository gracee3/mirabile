use super::{
    ActiveChartInspector, AppAction, AppError, AppErrorKind, AppReadModel, AppResult,
    ApplicationStatus, AuthoringCapabilitiesReadModel, Availability,
    CalculationDiagnosticsReadModel, CalculationRuntime, ChartPersistence, ChartSlotAssignment,
    CommandCapability, ConfigurationLayer, DraftState, ImplementationIdentityReadModel,
    InspectorReadModel, LibraryReadModel, OpenChartSummary, RealApplication, RealState,
    ResourceEditorReadModel, ResourceRepository, ViewComputationState, ViewInstanceId,
    ViewReadModel, ViewSummary, WorkerProtocolVersion, WorkspaceReadModel,
    aspect_editor_read_model, binding_summary, capability, chart_record_subtitle, disabled,
    resolve_typed_binding, view_title,
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) fn read_model(&self) -> AppResult<AppReadModel> {
        let state = self.state.borrow();
        let mut model = state.read_model()?;
        let complete_location = state
            .chart_editor
            .as_ref()
            .is_some_and(crate::ChartAuthoringEditor::location_complete);
        model.authoring = AuthoringCapabilitiesReadModel::from_backend(
            self.engine.backend_descriptor(),
            complete_location,
        );
        model.calculation = Some(self.calculation_diagnostics(&state));
        Ok(model)
    }

    fn calculation_diagnostics(&self, state: &RealState) -> CalculationDiagnosticsReadModel {
        let descriptor = self.engine.backend_descriptor();
        let backend = &descriptor.fingerprint.backend;
        let engine = self.engine.calculation_engine_identity();
        let runtime = state
            .session
            .as_ref()
            .and_then(|session| session.active_view)
            .and_then(|view_id| state.views.get(&view_id));
        let current = runtime.and_then(|runtime| runtime.expected.as_ref());
        let latest = current.or_else(|| runtime.and_then(|runtime| runtime.last_expected.as_ref()));
        CalculationDiagnosticsReadModel {
            backend: ImplementationIdentityReadModel {
                id: backend.id.clone(),
                version: backend.version.clone(),
                revision: backend.revision.clone(),
            },
            engine: ImplementationIdentityReadModel {
                id: engine.id.clone(),
                version: engine.version.clone(),
                revision: engine.revision.clone(),
            },
            worker_protocol: u32::from(WorkerProtocolVersion::CURRENT.get()),
            active_request_id: current.map(|expected| expected.request_id.get()),
            calc_key: latest.map(|expected| expected.calc_key.to_string()),
            analysis_key: latest.map(|expected| expected.analysis_key.to_string()),
            computation: runtime.map(|runtime| {
                match runtime.computation {
                    ViewComputationState::Loading => "loading",
                    ViewComputationState::Fresh => "fresh",
                    ViewComputationState::Refreshing => "refreshing",
                    ViewComputationState::Failed(_) => "failed",
                }
                .to_owned()
            }),
            last_good_scene_present: runtime.is_some_and(|runtime| runtime.scene.is_some()),
        }
    }
}

impl RealState {
    #[allow(clippy::too_many_lines)]
    pub(super) fn read_model(&self) -> AppResult<AppReadModel> {
        if !matches!(self.status, ApplicationStatus::Ready) {
            let mut model = AppReadModel::initializing();
            model.version = self.version;
            model.status = self.status.clone();
            model.activity = self.activity_read_model();
            model.notice.clone_from(&self.notice);
            return Ok(model);
        }
        let workspace = self.workspace().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Unavailable,
                "Ready application has no workspace",
            )
        })?;
        let session = self.session()?;
        let library_charts = self.catalog.library_charts()?;
        let mut open_charts = workspace
            .chart_instances
            .iter()
            .map(|chart| self.catalog.open_chart_summary(chart))
            .collect::<AppResult<Vec<_>>>()?;
        open_charts.extend(session.draft_charts.iter().map(|chart| OpenChartSummary {
            instance_id: chart.instance_id,
            title: chart.draft.title.clone(),
            subtitle: chart_record_subtitle(&chart.draft.record),
            persistence: ChartPersistence::Ephemeral,
        }));
        let active_chart = session.active_chart.and_then(|active_id| {
            open_charts
                .iter()
                .find(|chart| chart.instance_id == active_id)
                .map(|chart| ActiveChartInspector {
                    instance_id: chart.instance_id,
                    title: chart.title.clone(),
                    subtitle: chart.subtitle.clone(),
                    persistence: chart.persistence.clone(),
                })
        });
        let view_summaries = workspace
            .views
            .iter()
            .map(|view| {
                Ok(ViewSummary {
                    view_id: view.id,
                    title: view_title(view, &self.catalog)?,
                })
            })
            .collect::<AppResult<Vec<_>>>()?;
        let active_view = session
            .active_view
            .map(|view_id| self.view_read_model(view_id))
            .transpose()?;
        let active_aspect_set = workspace.profile.aspects.id();
        let mut bindings = vec![
            binding_summary(
                "Displayed points",
                &workspace.profile.displayed_points,
                &self.catalog,
            )?,
            binding_summary(
                "Aspected points",
                &workspace.profile.aspected_points,
                &self.catalog,
            )?,
            binding_summary(
                "Transit points",
                &workspace.profile.transit_points,
                &self.catalog,
            )?,
            binding_summary("Aspect set", &workspace.profile.aspects, &self.catalog)?,
            binding_summary(
                "Analysis profile",
                &workspace.profile.analysis,
                &self.catalog,
            )?,
            binding_summary("Theme", &workspace.profile.theme, &self.catalog)?,
            binding_summary("Wheel template", &workspace.profile.wheel, &self.catalog)?,
        ];
        if let Some(view) = session
            .active_view
            .and_then(|id| workspace.views.iter().find(|view| view.id == id))
        {
            bindings.push(binding_summary(
                "View document",
                &view.document,
                &self.catalog,
            )?);
        }

        Ok(AppReadModel {
            version: self.version,
            status: self.status.clone(),
            activity: self.activity_read_model(),
            calculation: None,
            authoring: AuthoringCapabilitiesReadModel::default(),
            chart_editor: self
                .chart_editor
                .as_ref()
                .map(crate::ChartAuthoringEditor::read_model),
            library: LibraryReadModel {
                charts: library_charts,
                aspect_sets: self.catalog.aspect_set_summaries()?,
            },
            workspace: WorkspaceReadModel {
                charts: open_charts,
                active_chart: session.active_chart,
                selected_charts: session.selected_charts.clone(),
                views: view_summaries,
                active_view: session.active_view,
                document_id: self.workspace.as_ref().map(|document| document.id),
                document_revision: self.workspace.as_ref().map(|document| document.revision),
                document_dirty: session.document_dirty,
                has_temporary_display_override: session
                    .active_view
                    .is_some_and(|view_id| session.temporary_view_overrides.contains_key(&view_id)),
            },
            active_view,
            inspector: InspectorReadModel {
                active_chart,
                bindings,
                active_aspect_set,
            },
            resource_editor: ResourceEditorReadModel {
                aspect_set: self
                    .editor
                    .as_ref()
                    .map(aspect_editor_read_model)
                    .transpose()?,
            },
            capabilities: self.capabilities(),
            notice: self.notice.clone(),
        })
    }

    pub(super) fn view_read_model(&self, view_id: ViewInstanceId) -> AppResult<ViewReadModel> {
        let workspace = self.workspace().expect("read model checked workspace");
        let session = self.session()?;
        let view = workspace
            .views
            .iter()
            .find(|view| view.id == view_id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("View {view_id} was not found"),
                )
            })?;
        let document =
            resolve_typed_binding(&view.document, &self.catalog, ConfigurationLayer::View)
                .map_err(|error| AppError::new(AppErrorKind::NotFound, error.to_string()))?;
        let runtime = self.views.get(&view_id).cloned().unwrap_or_default();
        Ok(ViewReadModel {
            view_id,
            title: view_title(view, &self.catalog)?,
            scene: runtime.scene,
            computation: runtime.computation,
            slots: document
                .value
                .chart_slots
                .into_iter()
                .map(|slot| ChartSlotAssignment {
                    chart: session.effective_chart_assignment(view_id, &slot.id),
                    slot: slot.id,
                    label: slot.label,
                    required: slot.required,
                })
                .collect(),
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn capabilities(&self) -> Vec<CommandCapability> {
        let begin_new_chart = if self.chart_editor.is_none() {
            Availability::Enabled
        } else {
            disabled("Save or cancel the current chart editor first")
        };
        let save_chart_editor = self.chart_editor.as_ref().map_or_else(
            || disabled("There is no chart editor to save"),
            |editor| {
                if editor.state == crate::ChartEditorState::Saving {
                    disabled("The chart editor is already saving")
                } else if editor.validation.is_empty() {
                    Availability::Enabled
                } else {
                    disabled("Complete every invalid chart field before saving")
                }
            },
        );
        let cancel_chart_editor = self.chart_editor.as_ref().map_or_else(
            || disabled("There is no chart editor to cancel"),
            |editor| {
                if editor.state == crate::ChartEditorState::Saving {
                    disabled("Wait for the chart save to finish")
                } else {
                    Availability::Enabled
                }
            },
        );
        let (save, cancel) = match self.editor.as_ref().map(|editor| &editor.state) {
            None => (
                disabled("Begin an Aspect Set edit before saving"),
                disabled("There is no draft to cancel"),
            ),
            Some(DraftState::Clean { .. }) => (
                disabled("The draft has no changes"),
                disabled("The draft has no changes"),
            ),
            Some(DraftState::Dirty { .. }) => (Availability::Enabled, Availability::Enabled),
            Some(DraftState::Saving { .. }) => (
                disabled("The draft is currently saving"),
                disabled("Wait for the save to finish"),
            ),
            Some(DraftState::Conflict { .. }) => (
                disabled("Resolve or cancel the revision conflict before saving"),
                Availability::Enabled,
            ),
        };
        let begin = if self
            .editor
            .as_ref()
            .is_some_and(|editor| matches!(editor.state, DraftState::Saving { .. }))
        {
            disabled("Wait for the current Aspect Set save to finish")
        } else if self
            .workspace()
            .and_then(|workspace| workspace.profile.aspects.id())
            .is_some()
        {
            Availability::Enabled
        } else {
            disabled("The active Aspect Set is inline and has no canonical resource to edit")
        };
        let refresh = self
            .session
            .as_ref()
            .and_then(|session| session.active_view)
            .and_then(|id| self.views.get(&id))
            .map_or_else(|| disabled("No active view"), |_| Availability::Enabled);
        let active_chart_is_draft = self.session.as_ref().is_some_and(|session| {
            session.active_chart.is_some_and(|active| {
                session
                    .draft_charts
                    .iter()
                    .any(|chart| chart.instance_id == active)
            })
        });
        let save_chart_draft = if active_chart_is_draft {
            Availability::Enabled
        } else {
            disabled("The active chart is not an unsaved draft")
        };
        let cancel_chart_draft = save_chart_draft.clone();
        let save_workspace = self.session.as_ref().map_or_else(
            || disabled("No workspace session"),
            |session| match session.backing {
                super::WorkspaceDocumentBacking::Unsaved => Availability::Enabled,
                super::WorkspaceDocumentBacking::Saved { .. } if session.document_dirty => {
                    Availability::Enabled
                }
                super::WorkspaceDocumentBacking::Saved { .. } => {
                    disabled("The workspace has no durable changes")
                }
            },
        );
        let promote_display = self.session.as_ref().map_or_else(
            || disabled("No workspace session"),
            |session| {
                if session
                    .active_view
                    .is_some_and(|view_id| session.temporary_view_overrides.contains_key(&view_id))
                {
                    Availability::Enabled
                } else {
                    disabled("The active view has no temporary display override")
                }
            },
        );
        vec![
            capability(AppAction::BeginNewChart, begin_new_chart),
            capability(AppAction::SaveChartEditor, save_chart_editor),
            capability(AppAction::CancelChartEditor, cancel_chart_editor),
            capability(AppAction::SaveChartDraft, save_chart_draft),
            capability(AppAction::CancelChartDraft, cancel_chart_draft),
            capability(AppAction::BeginAspectSetEdit, begin),
            capability(AppAction::SaveDraft, save),
            capability(AppAction::CancelDraft, cancel),
            capability(AppAction::SaveWorkspace, save_workspace),
            capability(AppAction::PromoteWorkspaceDisplay, promote_display),
            capability(AppAction::RefreshView, refresh),
        ]
    }
}
