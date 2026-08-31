use super::{
    ActiveChartInspector, AppAction, AppError, AppErrorKind, AppReadModel, AppResult,
    ApplicationStatus, AuthoringCapabilitiesReadModel, Availability,
    CalculationDiagnosticsReadModel, CalculationRuntime, ChartPersistence, ChartSlotAssignment,
    ChartSlotOption, CommandCapability, ConfigurationLayer, DisplayValueSource, DraftState,
    ImplementationIdentityReadModel, InspectorReadModel, LibraryReadModel, OpenChartSummary,
    PointVisibilityReadModel, RealApplication, RealState, ResourceEditorReadModel,
    ResourceRepository, SlotAssignmentSource, ViewComputationState, ViewDisplayReadModel,
    ViewInstanceId, ViewReadModel, ViewSummary, WorkerProtocolVersion, WorkspaceReadModel,
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
        let authoring = AuthoringCapabilitiesReadModel::from_backend(
            self.engine.backend_descriptor(),
            complete_location,
        );
        if let Some(view) = model.active_view.as_mut() {
            view.display = state.view_display_read_model(view.view_id, &authoring.points)?;
        }
        model.authoring = authoring;
        model.calculation = Some(self.calculation_diagnostics(&state));
        model.parameters = parameter_coverage(&model);
        model.semantic_output = semantic_output(&state);
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
                    rotation: view.overrides.rotation,
                    hidden_points: view.overrides.hidden_points.clone(),
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
                crate::WorkspaceBindingSlot::DisplayedPoints,
                "Displayed points",
                &workspace.profile.displayed_points,
                &self.catalog,
            )?,
            binding_summary(
                crate::WorkspaceBindingSlot::AspectedPoints,
                "Aspected points",
                &workspace.profile.aspected_points,
                &self.catalog,
            )?,
            binding_summary(
                crate::WorkspaceBindingSlot::TransitPoints,
                "Transit points",
                &workspace.profile.transit_points,
                &self.catalog,
            )?,
            binding_summary(
                crate::WorkspaceBindingSlot::Aspects,
                "Aspect set",
                &workspace.profile.aspects,
                &self.catalog,
            )?,
            binding_summary(
                crate::WorkspaceBindingSlot::Analysis,
                "Analysis profile",
                &workspace.profile.analysis,
                &self.catalog,
            )?,
            binding_summary(
                crate::WorkspaceBindingSlot::Theme,
                "Theme",
                &workspace.profile.theme,
                &self.catalog,
            )?,
            binding_summary(
                crate::WorkspaceBindingSlot::Wheel,
                "Wheel template",
                &workspace.profile.wheel,
                &self.catalog,
            )?,
        ];
        if let Some(view) = session
            .active_view
            .and_then(|id| workspace.views.iter().find(|view| view.id == id))
        {
            bindings.push(binding_summary(
                crate::WorkspaceBindingSlot::ViewDocument { view_id: view.id },
                "View document",
                &view.document,
                &self.catalog,
            )?);
        }

        let mut repository = self
            .catalog
            .repository_read_model(self.repository_selection.as_ref());
        repository.deletion = self.repository_deletion_read_model();
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
                workspaces: self.catalog.workspace_summaries(),
            },
            resources: self.catalog.resource_catalog_read_model(),
            repository,
            workspace: WorkspaceReadModel {
                title: session.working_title.clone(),
                description: session.working_description.clone(),
                tags: session.working_tags.clone(),
                charts: open_charts,
                active_chart: session.active_chart,
                selected_charts: session.selected_charts.clone(),
                views: view_summaries,
                active_view: session.active_view,
                document_id: self.workspace.as_ref().map(|document| document.id),
                document_revision: self.workspace.as_ref().map(|document| document.revision),
                document_schema_version: self
                    .workspace
                    .as_ref()
                    .map(|document| document.schema_version),
                document_created_at: self.workspace.as_ref().map(|document| document.created_at),
                document_modified_at: self.workspace.as_ref().map(|document| document.modified_at),
                validation: session.metadata_validation(),
                document_dirty: session.document_dirty,
                has_temporary_display_override: session
                    .active_view
                    .is_some_and(|view_id| session.temporary_view_overrides.contains_key(&view_id)),
                switch_decision: self
                    .workspace_switch
                    .as_ref()
                    .map(|decision| self.workspace_switch_decision(decision.target))
                    .transpose()?,
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
                drafts: self
                    .resource_drafts
                    .values()
                    .map(|draft| draft.read_model_with_catalog(&self.catalog.current))
                    .collect(),
            },
            parameters: Vec::new(),
            semantic_output: crate::SemanticOutputReadModel::default(),
            capabilities: self.capabilities(),
            notice: self.notice.clone(),
        })
    }

    #[allow(clippy::too_many_lines)]
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
        let slot_options = |required: bool| {
            let mut options = vec![ChartSlotOption {
                chart: None,
                label: "Unassigned".into(),
                persistence: None,
                enabled: !required,
                disabled_reason: required.then(|| "This slot requires a chart".into()),
            }];
            options.extend(workspace.chart_instances.iter().map(|chart| {
                let title = self.catalog.chart_definition(chart.definition).map_or_else(
                    || "Missing saved chart".into(),
                    |definition| definition.title.clone(),
                );
                ChartSlotOption {
                    chart: Some(chart.instance_id),
                    label: title,
                    persistence: Some(ChartPersistence::Saved {
                        definition_id: chart.definition,
                    }),
                    enabled: true,
                    disabled_reason: None,
                }
            }));
            options.extend(session.draft_charts.iter().map(|chart| ChartSlotOption {
                chart: Some(chart.instance_id),
                label: chart.draft.title.clone(),
                persistence: Some(ChartPersistence::Ephemeral),
                enabled: true,
                disabled_reason: None,
            }));
            options
        };
        Ok(ViewReadModel {
            view_id,
            title: view_title(view, &self.catalog)?,
            scene: runtime.scene,
            computation: runtime.computation,
            display: ViewDisplayReadModel {
                points: Vec::new(),
                slots: Vec::new(),
                aspect_layers: mirabile_core::AspectLayerVisibility::default(),
                wheel: crate::WheelDisplayReadModel {
                    zodiac_boundaries: true,
                    zodiac_labels: true,
                    house_cusps: true,
                    house_numbers: true,
                    degree_labels: true,
                    retrograde_markers: true,
                },
                theme: mirabile_core::Theme::mirabile_dark(),
                rotation: None,
                has_temporary_override: false,
                promotion: disabled(
                    "Display state is unavailable until capabilities are projected",
                ),
            },
            slots: document
                .value
                .chart_slots
                .into_iter()
                .map(|slot| {
                    let durable_chart = view.charts.get(&slot.id).copied();
                    let draft_chart = session
                        .draft_chart_assignments
                        .get(&view_id)
                        .and_then(|assignments| assignments.get(&slot.id))
                        .copied();
                    let chart = draft_chart.or(durable_chart);
                    let source = if let Some(instance_id) = draft_chart {
                        SlotAssignmentSource::Draft {
                            instance_id,
                            promotion: crate::DraftAssignmentPromotion::RequiresChartSave,
                        }
                    } else if let Some(instance_id) = durable_chart {
                        let definition_id = workspace
                            .chart_instances
                            .iter()
                            .find(|chart| chart.instance_id == instance_id)
                            .map(|chart| chart.definition)
                            .expect("validated saved slot assignment has a chart definition");
                        SlotAssignmentSource::Saved {
                            instance_id,
                            definition_id,
                        }
                    } else {
                        SlotAssignmentSource::Unassigned
                    };
                    ChartSlotAssignment {
                        chart,
                        durable_chart,
                        draft_chart,
                        source,
                        options: slot_options(slot.required),
                        slot: slot.id,
                        label: slot.label,
                        required: slot.required,
                    }
                })
                .collect(),
        })
    }

    fn view_display_read_model(
        &self,
        view_id: ViewInstanceId,
        supported_points: &[crate::AuthoringOption<crate::PointId>],
    ) -> AppResult<ViewDisplayReadModel> {
        let session = self.session()?;
        let view = session
            .document
            .views
            .iter()
            .find(|view| view.id == view_id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("View {view_id} was not found"),
                )
            })?;
        let temporary = session.temporary_view_overrides.get(&view_id);
        let effective = temporary.unwrap_or(&view.overrides);
        let workspace = &session.document;
        let wheel = resolve_typed_binding(
            view.wheel.as_ref().unwrap_or(&workspace.profile.wheel),
            &self.catalog,
            ConfigurationLayer::View,
        )
        .map_err(|error| AppError::new(AppErrorKind::NotFound, error.to_string()))?
        .value;
        let theme = resolve_typed_binding(
            view.theme.as_ref().unwrap_or(&workspace.profile.theme),
            &self.catalog,
            ConfigurationLayer::View,
        )
        .map_err(|error| AppError::new(AppErrorKind::NotFound, error.to_string()))?
        .value;
        let point_rows = |slot: Option<&mirabile_core::ChartSlotId>| {
            supported_points
                .iter()
                .filter(|option| option.enabled)
                .map(|option| {
                    let slot_hidden =
                        slot.and_then(|slot| effective.hidden_points_by_slot.get(slot));
                    let durable_slot_hidden =
                        slot.and_then(|slot| view.overrides.hidden_points_by_slot.get(slot));
                    PointVisibilityReadModel {
                        point_id: option.value.clone(),
                        label: option.label.clone(),
                        visible: !effective.hidden_points.contains(&option.value)
                            && !slot_hidden.is_some_and(|points| points.contains(&option.value)),
                        durable_visible: !view.overrides.hidden_points.contains(&option.value)
                            && !durable_slot_hidden
                                .is_some_and(|points| points.contains(&option.value)),
                        temporary_visible: temporary.map(|overrides| {
                            !overrides.hidden_points.contains(&option.value)
                                && !slot
                                    .and_then(|slot| overrides.hidden_points_by_slot.get(slot))
                                    .is_some_and(|points| points.contains(&option.value))
                        }),
                        source: if temporary.is_some() {
                            DisplayValueSource::Temporary
                        } else {
                            DisplayValueSource::Durable
                        },
                    }
                })
                .collect::<Vec<_>>()
        };
        Ok(ViewDisplayReadModel {
            points: point_rows(None),
            slots: resolve_typed_binding(&view.document, &self.catalog, ConfigurationLayer::View)
                .map_err(|error| AppError::new(AppErrorKind::NotFound, error.to_string()))?
                .value
                .chart_slots
                .into_iter()
                .map(|slot| crate::SlotDisplayReadModel {
                    visible: !effective.hidden_rings.contains(&slot.id),
                    points: point_rows(Some(&slot.id)),
                    slot: slot.id,
                    label: slot.label,
                })
                .collect(),
            aspect_layers: effective.aspect_layers.clone(),
            wheel: crate::WheelDisplayReadModel {
                zodiac_boundaries: wheel.zodiac.show_boundaries,
                zodiac_labels: wheel.zodiac.show_labels,
                house_cusps: wheel.houses.show_cusps,
                house_numbers: wheel.houses.show_numbers,
                degree_labels: wheel.labels.show_degrees,
                retrograde_markers: wheel.labels.show_retrograde,
            },
            theme,
            rotation: effective.rotation,
            has_temporary_override: temporary.is_some(),
            promotion: if temporary.is_some() {
                Availability::Enabled
            } else {
                disabled("The active view has no temporary display override")
            },
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
                } else if editor.state == crate::ChartEditorState::Conflict {
                    disabled("Cancel and reopen the chart to adopt the refreshed component heads")
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
            Some(DraftState::New | DraftState::Dirty { .. }) => {
                if self
                    .editor
                    .as_ref()
                    .is_some_and(|editor| !editor.metadata_validation().is_empty())
                {
                    (
                        disabled("Complete every invalid Aspect Set field before saving"),
                        Availability::Enabled,
                    )
                } else {
                    (Availability::Enabled, Availability::Enabled)
                }
            }
            Some(DraftState::Creating) => (
                disabled("The new Aspect Set is currently being created"),
                disabled("Wait for the create to finish"),
            ),
            Some(DraftState::Saving { .. }) => (
                disabled("The draft is currently saving"),
                disabled("Wait for the save to finish"),
            ),
            Some(DraftState::Conflict { .. }) => (
                disabled("Resolve or cancel the revision conflict before saving"),
                Availability::Enabled,
            ),
        };
        let replace_editor = match self.editor.as_ref().map(|editor| &editor.state) {
            None | Some(DraftState::Clean { .. }) => Availability::Enabled,
            Some(DraftState::Saving { .. } | DraftState::Creating) => {
                disabled("Wait for the current Aspect Set operation to finish")
            }
            Some(DraftState::New | DraftState::Dirty { .. } | DraftState::Conflict { .. }) => {
                disabled("Save or cancel the current Aspect Set editor first")
            }
        };
        let begin = if !replace_editor.is_enabled() {
            replace_editor.clone()
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
            |session| {
                if session.metadata_validation().is_empty() {
                    match session.backing {
                        super::WorkspaceDocumentBacking::Unsaved => Availability::Enabled,
                        super::WorkspaceDocumentBacking::Saved { .. } if session.document_dirty => {
                            Availability::Enabled
                        }
                        super::WorkspaceDocumentBacking::Saved { .. } => {
                            disabled("The workspace has no durable changes")
                        }
                    }
                } else {
                    disabled("Complete every invalid workspace field before saving")
                }
            },
        );
        let save_workspace = if self.workspace_switch.is_some() {
            disabled("Use Save and switch from the pending workspace decision")
        } else {
            save_workspace
        };
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
            capability(AppAction::BeginNewAspectSet, replace_editor.clone()),
            capability(
                AppAction::DuplicateAspectSet,
                if !replace_editor.is_enabled() {
                    replace_editor
                } else if self
                    .catalog
                    .aspect_set_summaries()
                    .is_ok_and(|sets| !sets.is_empty())
                {
                    Availability::Enabled
                } else {
                    disabled("There is no saved Aspect Set to duplicate")
                },
            ),
            capability(AppAction::SaveDraft, save),
            capability(AppAction::CancelDraft, cancel),
            capability(AppAction::SaveWorkspace, save_workspace),
            capability(AppAction::PromoteWorkspaceDisplay, promote_display),
            capability(AppAction::RefreshView, refresh),
        ]
    }
}

fn parameter_coverage(model: &AppReadModel) -> Vec<crate::ParameterCoverageReadModel> {
    use crate::{ParameterCoverageReadModel as Entry, ParameterStatus as Status};
    let chart_status = if model.chart_editor.is_some() {
        Status::Live
    } else {
        Status::Unavailable {
            reason: "Open or create a chart to edit factual and calculation parameters".into(),
        }
    };
    [
        ("chart facts", chart_status.clone()),
        ("calculation parameters", chart_status),
        ("workspace composition", Status::Live),
        ("point sets", Status::Persisted),
        ("aspect sets", Status::Live),
        ("analysis profiles", Status::Persisted),
        ("wheel templates", Status::Persisted),
        ("view documents", Status::Persisted),
        ("themes", Status::Persisted),
        ("query definitions", Status::Persisted),
        (
            "query execution",
            Status::Unavailable {
                reason: "Query execution is deferred; typed definitions remain persistable".into(),
            },
        ),
        ("calculation provenance", Status::ReadOnly),
    ]
    .into_iter()
    .map(|(parameter, status)| Entry {
        parameter: parameter.into(),
        status,
    })
    .collect()
}

#[allow(clippy::too_many_lines)]
fn semantic_output(state: &RealState) -> crate::SemanticOutputReadModel {
    use crate::{
        ProvenanceEntryReadModel, SemanticAngleReadModel, SemanticAspectReadModel,
        SemanticHouseReadModel, SemanticOutputReadModel, SemanticPointReadModel,
    };
    let Some(runtime) = state
        .session
        .as_ref()
        .and_then(|session| session.active_view)
        .and_then(|view_id| state.views.get(&view_id))
    else {
        return SemanticOutputReadModel {
            unavailable_reason: Some("No active view has calculated output".into()),
            ..Default::default()
        };
    };
    let Some(calculation) = runtime.semantic_calculation.as_ref() else {
        return SemanticOutputReadModel {
            unavailable_reason: Some("The active view has not completed a calculation".into()),
            ..Default::default()
        };
    };
    let mut points =
        calculation
            .celestial_positions
            .iter()
            .map(|(point_id, point)| SemanticPointReadModel {
                point_id: point_id.clone(),
                longitude_degrees: point.longitude.degrees(),
                latitude_degrees: point.latitude.degrees(),
                speed_degrees_per_day: point.speed_longitude.as_degrees_per_day(),
                retrograde: point.retrograde,
                derived: false,
            })
            .chain(calculation.derived_points.iter().map(|(point_id, point)| {
                SemanticPointReadModel {
                    point_id: point_id.clone(),
                    longitude_degrees: point.longitude.degrees(),
                    latitude_degrees: point.latitude.degrees(),
                    speed_degrees_per_day: point.speed_longitude.as_degrees_per_day(),
                    retrograde: point.retrograde,
                    derived: true,
                }
            }))
            .collect::<Vec<_>>();
    points.sort_by(|lhs, rhs| lhs.point_id.cmp(&rhs.point_id));
    let houses = calculation
        .houses
        .as_ref()
        .map(|houses| {
            houses
                .cusps
                .iter()
                .enumerate()
                .map(|(index, cusp)| SemanticHouseReadModel {
                    number: index + 1,
                    cusp_degrees: cusp.degrees(),
                })
                .collect()
        })
        .unwrap_or_default();
    let angles = [
        ("Ascendant", calculation.angles.ascendant),
        ("Midheaven", calculation.angles.midheaven),
    ]
    .into_iter()
    .filter_map(|(name, value)| {
        value.map(|value| SemanticAngleReadModel {
            name: name.into(),
            longitude_degrees: value.degrees(),
        })
    })
    .collect();
    let aspects = runtime
        .semantic_analysis
        .as_ref()
        .map(|analysis| {
            analysis
                .aspects
                .iter()
                .map(|aspect| SemanticAspectReadModel {
                    lhs: aspect.lhs.clone(),
                    rhs: aspect.rhs.clone(),
                    aspect: aspect.aspect.clone(),
                    separation_degrees: aspect.separation.degrees(),
                    orb_degrees: aspect.orb.degrees(),
                    applying: aspect.applying,
                })
                .collect()
        })
        .unwrap_or_default();
    let provenance = &calculation.provenance;
    SemanticOutputReadModel {
        points,
        houses,
        angles,
        aspects,
        provenance: vec![
            ProvenanceEntryReadModel {
                responsibility: "Mirabile calculation engine".into(),
                implementation: provenance.mirabile.calculation_engine.id.clone(),
                detail: provenance.mirabile.timezone_data_version.clone(),
            },
            ProvenanceEntryReadModel {
                responsibility: "Backend".into(),
                implementation: provenance.backend.id.clone(),
                detail: provenance.backend.version.clone(),
            },
            ProvenanceEntryReadModel {
                responsibility: "Celestial positions".into(),
                implementation: provenance.celestial.implementation.id.clone(),
                detail: format!("{:?}", provenance.celestial.coordinates),
            },
        ],
        unavailable_reason: None,
    }
}
