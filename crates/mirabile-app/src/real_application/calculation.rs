use super::{
    AnalysisKey, AppError, AppErrorKind, AppNotice, AppNoticeKind, AppResult, AspectAnalyzer,
    BTreeMap, CalculationEngine, CalculationOutcome, CalculationRuntime, CalculationRuntimeError,
    CalculationWorkerRequest, CalculationWorkerResult, ChartSource, ConfigurationLayer,
    ExpectedCalculation, MultiViewRuntime, PendingCachedMulti, PendingCachedView,
    PendingViewCalculation, PendingWork, PreparedCalculation, ProjectionVersion, RealApplication,
    RealState, ResourceRepository, Scene, SnapshotContext, ViewCalculationPlan,
    ViewComputationState, ViewInstanceId, WorkerProtocolVersion, info, not_found_for_view,
    render_key, resolve_typed_binding, success, view_computation_error, view_resolution_error,
    worker_failure_error,
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) fn refresh_active_view(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let Some(view_id) = state
            .session
            .as_ref()
            .and_then(|session| session.active_view)
        else {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "There is no active view to refresh",
            ));
        };
        state.views.get(&view_id).ok_or_else(|| {
            AppError::new(
                AppErrorKind::NotFound,
                format!("Active view {view_id} was not found"),
            )
        })?;
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info("Active view refresh requested"));
        state.advance()
    }

    pub(super) async fn complete_next_pending(&self, after: ProjectionVersion) -> AppResult<()> {
        let pending = self.state.borrow_mut().pending.pop_front();
        match pending {
            Some(PendingWork::CompleteCachedView(pending)) => self.complete_cached_view(*pending),
            Some(PendingWork::CompleteCachedMulti(pending)) => self.complete_cached_multi(*pending),
            Some(PendingWork::SaveAspectSet {
                expected_revision,
                next,
            }) => self.complete_aspect_set_save(expected_revision, next).await,
            Some(PendingWork::SaveTypedResource {
                kind,
                expected_revision,
                next,
            }) => {
                self.complete_typed_resource_save(kind, expected_revision, *next)
                    .await
            }
            Some(PendingWork::CreateChart {
                instance_id,
                record,
                definition,
            }) => {
                self.complete_chart_create(instance_id, *record, *definition)
                    .await
            }
            Some(PendingWork::SaveChartEdit {
                instance_id,
                definition_id,
                batch,
            }) => {
                self.complete_saved_chart_save(instance_id, definition_id, batch)
                    .await
            }
            Some(PendingWork::SaveWorkspace {
                expected_revision,
                next,
            }) => self.complete_workspace_save(expected_revision, *next).await,
            Some(PendingWork::LoadDemoBundle { resources }) => {
                self.complete_demo_bundle_load(resources).await
            }
            None if !self.state.borrow().inflight.is_empty() => {
                // RuntimeInbox and the browser Worker runtime are intentionally
                // single-consumer queues. Serialize receive calls so concurrent
                // application observers cannot each consume a different runtime
                // message. A waiter that queued behind the active driver must
                // recheck the application projection before receiving again.
                let _receive_guard = self.runtime_receive_gate.lock().await;
                {
                    let state = self.state.borrow();
                    if state.version != after
                        || !state.pending.is_empty()
                        || state.inflight.is_empty()
                    {
                        return Ok(());
                    }
                }
                match self.runtime.receive().await {
                    Ok(result) => self.accept_worker_result(result),
                    Err(error) => self.accept_runtime_failure(&error),
                }
            }
            None => Ok(()),
        }
    }

    pub(super) fn complete_cached_view(&self, pending: PendingCachedView) -> AppResult<()> {
        let PendingCachedView {
            view_id,
            expected,
            prepared,
            plan,
            calculation,
        } = pending;
        let mut state = self.state.borrow_mut();
        if state
            .views
            .get(&view_id)
            .and_then(|runtime| runtime.expected.as_ref())
            != Some(&expected)
        {
            return Ok(());
        }
        let result = Self::finish_scene(&mut state, &prepared, &plan, calculation.clone());
        if result.is_ok() {
            let analysis = state.cache.analysis(&expected.analysis_key).cloned();
            let runtime = state.views.entry(view_id).or_default();
            runtime.semantic_calculation = Some(calculation);
            runtime.semantic_analysis = analysis;
        }
        Self::publish_view_result(&mut state, view_id, result)
    }

    pub(super) fn accept_worker_result(&self, result: CalculationWorkerResult) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let Some(pending) = state.inflight.remove(&result.request_id) else {
            return Ok(());
        };
        if pending.slot.is_some() {
            return self.accept_multi_worker_result(&mut state, pending, result);
        }
        let Some(expected) = state
            .views
            .get(&pending.view_id)
            .and_then(|runtime| runtime.expected.clone())
        else {
            return Ok(());
        };
        if expected.request_id != result.request_id {
            return Ok(());
        }
        if result.calc_key != expected.calc_key || pending.prepared.calc_key != expected.calc_key {
            return Self::publish_view_result(
                &mut state,
                pending.view_id,
                Err(AppError::new(
                    AppErrorKind::ViewComputation,
                    "Calculation runtime integrity failure: result CalcKey did not match the current request",
                )),
            );
        }
        if result.protocol_version != WorkerProtocolVersion::CURRENT {
            return Self::publish_view_result(
                &mut state,
                pending.view_id,
                Err(AppError::new(
                    AppErrorKind::ViewComputation,
                    format!(
                        "Calculation runtime protocol mismatch: received version {}",
                        result.protocol_version.get()
                    ),
                )),
            );
        }
        match result.outcome {
            CalculationOutcome::Success(backend_result) => {
                let calculation = match self.engine.complete(&pending.prepared, *backend_result) {
                    Ok(calculation) => calculation,
                    Err(error) => {
                        return Self::publish_view_result(
                            &mut state,
                            pending.view_id,
                            Err(view_computation_error(error)),
                        );
                    }
                };
                // Only authoritative successes enter the content-addressed cache. Stale
                // successes are deliberately discarded before this point.
                state
                    .cache
                    .insert_calculation(expected.calc_key.clone(), calculation.clone());
                let scene = Self::finish_scene(
                    &mut state,
                    &pending.prepared,
                    &pending.plan,
                    calculation.clone(),
                );
                if scene.is_ok() {
                    let analysis = state.cache.analysis(&expected.analysis_key).cloned();
                    let runtime = state.views.entry(pending.view_id).or_default();
                    runtime.semantic_calculation = Some(calculation);
                    runtime.semantic_analysis = analysis;
                }
                Self::publish_view_result(&mut state, pending.view_id, scene)
            }
            CalculationOutcome::Failure(failure) => Self::publish_view_result(
                &mut state,
                pending.view_id,
                Err(worker_failure_error(&failure)),
            ),
        }
    }

    pub(super) fn complete_cached_multi(&self, pending: PendingCachedMulti) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let Some(multi) = state
            .views
            .get_mut(&pending.view_id)
            .and_then(|runtime| runtime.multi.as_mut())
        else {
            return Ok(());
        };
        if multi.generation != pending.generation
            || multi.expected.get(&pending.slot) != Some(&pending.expected)
        {
            return Ok(());
        }
        multi.calculations.insert(pending.slot, pending.calculation);
        Self::finish_multi_if_ready(&mut state, pending.view_id)
    }

    fn accept_multi_worker_result(
        &self,
        state: &mut RealState,
        pending: PendingViewCalculation,
        result: CalculationWorkerResult,
    ) -> AppResult<()> {
        let slot = pending.slot.expect("multi-chart pending work has a slot");
        let current = state
            .views
            .get(&pending.view_id)
            .and_then(|runtime| runtime.multi.as_ref())
            .filter(|multi| multi.generation == pending.generation)
            .and_then(|multi| multi.expected.get(&slot))
            .cloned();
        let Some(expected) = current else {
            return Ok(());
        };
        if expected.request_id != result.request_id || expected.calc_key != result.calc_key {
            return Self::fail_multi_view(
                state,
                pending.view_id,
                "Multi-chart calculation integrity failure",
            );
        }
        if result.protocol_version != WorkerProtocolVersion::CURRENT {
            return Self::fail_multi_view(
                state,
                pending.view_id,
                "Multi-chart Worker protocol mismatch",
            );
        }
        let calculation = match result.outcome {
            CalculationOutcome::Success(value) => self
                .engine
                .complete(&pending.prepared, *value)
                .map_err(view_computation_error),
            CalculationOutcome::Failure(failure) => Err(worker_failure_error(&failure)),
        };
        match calculation {
            Ok(calculation) => {
                state
                    .cache
                    .insert_calculation(expected.calc_key, calculation.clone());
                if let Some(multi) = state
                    .views
                    .get_mut(&pending.view_id)
                    .and_then(|runtime| runtime.multi.as_mut())
                    .filter(|multi| multi.generation == pending.generation)
                {
                    multi.calculations.insert(slot, calculation);
                }
                Self::finish_multi_if_ready(state, pending.view_id)
            }
            Err(error) => Self::fail_multi_view(state, pending.view_id, &error.message),
        }
    }

    fn fail_multi_view(
        state: &mut RealState,
        view_id: ViewInstanceId,
        message: &str,
    ) -> AppResult<()> {
        let generation = state
            .views
            .get(&view_id)
            .and_then(|runtime| runtime.multi.as_ref())
            .map(|multi| multi.generation);
        if let Some(generation) = generation {
            state.inflight.retain(|_, pending| {
                pending.view_id != view_id || pending.generation != generation
            });
        }
        let runtime = state.views.entry(view_id).or_default();
        runtime.multi = None;
        runtime.computation =
            ViewComputationState::Failed(AppError::new(AppErrorKind::ViewComputation, message));
        state.notice = Some(AppNotice {
            kind: AppNoticeKind::Warning,
            message: format!(
                "Biwheel refresh failed; the complete last good Scene remains visible: {message}"
            ),
        });
        state.advance()
    }

    pub(super) fn accept_runtime_failure(&self, error: &CalculationRuntimeError) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let affected = state
            .views
            .iter()
            .filter_map(|(view_id, runtime)| {
                (runtime.expected.is_some() || runtime.multi.is_some()).then_some(*view_id)
            })
            .collect::<Vec<_>>();
        if affected.is_empty() {
            return Ok(());
        }
        for view_id in &affected {
            if let Some(request_id) = state
                .views
                .get(view_id)
                .and_then(|runtime| runtime.expected.as_ref())
                .map(|expected| expected.request_id)
            {
                state.inflight.remove(&request_id);
            }
            let runtime = state.views.entry(*view_id).or_default();
            runtime.expected = None;
            runtime.multi = None;
            runtime.computation = ViewComputationState::Failed(AppError::new(
                AppErrorKind::ViewComputation,
                format!("Calculation runtime failed: {}", error.message),
            ));
        }
        state
            .inflight
            .retain(|_, pending| !affected.contains(&pending.view_id));
        state.notice = Some(AppNotice {
            kind: AppNoticeKind::Warning,
            message: format!(
                "Calculation runtime failed; last good Scenes remain visible: {}",
                error.message
            ),
        });
        state.advance()
    }

    pub(super) fn publish_view_result(
        state: &mut RealState,
        view_id: ViewInstanceId,
        result: AppResult<Scene>,
    ) -> AppResult<()> {
        let runtime = state.views.entry(view_id).or_default();
        runtime.expected = None;
        match result {
            Ok(scene) => {
                runtime.scene = Some(scene);
                runtime.computation = ViewComputationState::Fresh;
                state.notice = Some(success("View computation completed"));
            }
            Err(error) => {
                runtime.computation = ViewComputationState::Failed(error.clone());
                state.notice = Some(AppNotice {
                    kind: AppNoticeKind::Warning,
                    message: format!(
                        "View computation failed; the last good Scene remains visible: {}",
                        error.message
                    ),
                });
            }
        }
        state.advance()
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn submit_active_view_refresh(&self, state: &mut RealState) -> AppResult<()> {
        let Some(view_id) = state
            .session
            .as_ref()
            .and_then(|session| session.active_view)
        else {
            return Ok(());
        };
        let has_chart_assignment = state
            .session
            .as_ref()
            .is_some_and(|session| !session.effective_chart_assignments(view_id).is_empty());
        if !has_chart_assignment {
            let error = AppError::new(
                AppErrorKind::ViewComputation,
                "The active view has no assigned chart",
            );
            let runtime = state.views.entry(view_id).or_default();
            runtime.expected = None;
            runtime.computation = ViewComputationState::Failed(error.clone());
            state.notice = Some(AppNotice {
                kind: AppNoticeKind::Info,
                message: error.message,
            });
            return Ok(());
        }
        let is_biwheel = state
            .session
            .as_ref()
            .and_then(|session| {
                session
                    .document
                    .views
                    .iter()
                    .find(|view| view.id == view_id)
            })
            .and_then(|view| {
                resolve_typed_binding(&view.document, &state.catalog, ConfigurationLayer::View)
                    .ok()
                    .map(|document| {
                        document
                            .value
                            .chart_slots
                            .iter()
                            .filter(|slot| slot.required)
                            .count()
                            > 1
                    })
            })
            .unwrap_or(false);
        if is_biwheel {
            return self.submit_biwheel_refresh(state, view_id);
        }
        let (prepared, plan) = self.prepare_view_calculation(state, view_id)?;
        let request_id = state.next_request_id;
        state.next_request_id = request_id.next().map_err(|error| {
            AppError::new(
                AppErrorKind::Unavailable,
                format!("Could not allocate calculation request ID: {error}"),
            )
        })?;
        let expected = ExpectedCalculation {
            request_id,
            calc_key: prepared.calc_key.clone(),
            analysis_key: AnalysisKey::derive(
                std::slice::from_ref(&prepared.calc_key),
                &plan.aspected_points,
                &plan.aspect_set,
                &plan.analysis,
            )
            .map_err(view_computation_error)?,
        };
        let cached = state.cache.calculation(&prepared.calc_key).cloned();
        if let Some(calculation) = cached {
            state
                .pending
                .push_front(PendingWork::CompleteCachedView(Box::new(
                    PendingCachedView {
                        view_id,
                        expected: expected.clone(),
                        prepared,
                        plan,
                        calculation,
                    },
                )));
        } else {
            let worker_request = CalculationWorkerRequest {
                protocol_version: WorkerProtocolVersion::CURRENT,
                request_id,
                calc_key: prepared.calc_key.clone(),
                backend: self.engine.backend_descriptor().fingerprint.clone(),
                request: prepared.request.clone(),
            };
            if let Err(error) = self.runtime.submit(worker_request) {
                let runtime = state.views.entry(view_id).or_default();
                runtime.expected = None;
                runtime.computation = ViewComputationState::Failed(AppError::new(
                    AppErrorKind::ViewComputation,
                    format!("Could not submit calculation: {}", error.message),
                ));
                state.notice = Some(AppNotice {
                    kind: AppNoticeKind::Warning,
                    message: format!(
                        "Calculation submission failed; the last good Scene remains visible: {}",
                        error.message
                    ),
                });
                return Ok(());
            }
            state.inflight.insert(
                request_id,
                PendingViewCalculation {
                    view_id,
                    slot: None,
                    generation: 0,
                    prepared,
                    plan,
                },
            );
        }
        let runtime = state.views.entry(view_id).or_default();
        runtime.expected = Some(expected.clone());
        runtime.last_expected = Some(expected);
        runtime.computation = if runtime.scene.is_some() {
            ViewComputationState::Refreshing
        } else {
            ViewComputationState::Loading
        };
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn prepare_view_calculation(
        &self,
        state: &RealState,
        view_id: ViewInstanceId,
    ) -> AppResult<(PreparedCalculation, ViewCalculationPlan)> {
        let session = state.session.as_ref().ok_or_else(|| {
            AppError::new(
                AppErrorKind::ViewComputation,
                "No workspace session is active",
            )
        })?;
        let workspace = session.document.clone();
        let chart_assignments = session.effective_chart_assignments(view_id);
        let view = workspace
            .views
            .iter()
            .find(|view| view.id == view_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::ViewComputation,
                    format!("View {view_id} was not found in the workspace"),
                )
            })?;
        let rotation = view.overrides.rotation;
        let document =
            resolve_typed_binding(&view.document, &state.catalog, ConfigurationLayer::View)
                .map_err(view_resolution_error)?;
        let chart_instance = document
            .value
            .chart_slots
            .iter()
            .filter(|slot| slot.required)
            .find_map(|slot| chart_assignments.get(&slot.id).copied())
            .or_else(|| chart_assignments.values().next().copied())
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::ViewComputation,
                    "The active view has no assigned chart",
                )
            })?;
        let (prepared, effective) = if let Some(workspace_chart) = workspace
            .chart_instances
            .iter()
            .find(|chart| chart.instance_id() == chart_instance)
        {
            if let Some(editor) = state
                .chart_editor
                .as_ref()
                .filter(|editor| editor.instance_id() == chart_instance)
            {
                let draft = &editor.last_valid;
                let effective = state.effective_configuration(&draft.calculation, &view)?;
                let prepared = self
                    .engine
                    .resolve(
                        &draft.record,
                        &effective.calculation.value,
                        &effective.displayed_points.value,
                        &effective.aspected_points.value,
                    )
                    .map_err(view_computation_error)?
                    .with_context(SnapshotContext {
                        definition: None,
                        records: Vec::new(),
                        location_display_name: draft
                            .record
                            .location
                            .as_ref()
                            .map(|location| location.display_name.clone()),
                    });
                return Ok((
                    prepared,
                    ViewCalculationPlan {
                        displayed_points: effective.displayed_points.value,
                        aspected_points: effective.aspected_points.value,
                        aspect_set: effective.aspect_set.value,
                        analysis: effective.analysis.value,
                        wheel: effective.wheel.value,
                        theme: effective.theme.value,
                        rotation,
                    },
                ));
            }
            let definition_id = workspace_chart.definition;
            let definition = state
                .catalog
                .chart_definition(definition_id)
                .cloned()
                .ok_or_else(|| not_found_for_view("ChartDefinition", definition_id))?;
            let record_id = match definition.payload.source {
                ChartSource::Radix { record } => record,
                ChartSource::Derived { .. } => {
                    return Err(AppError::new(
                        AppErrorKind::ViewComputation,
                        "Derived chart calculation remains intentionally deferred",
                    ));
                }
            };
            let record = state
                .catalog
                .chart_record(record_id)
                .cloned()
                .ok_or_else(|| not_found_for_view("ChartRecord", record_id))?;
            let effective =
                state.effective_configuration(&definition.payload.calculation, &view)?;
            let mut effective_definition = definition;
            effective_definition.payload.calculation = effective.calculation.value.clone();
            let prepared = self
                .engine
                .prepare(
                    &effective_definition,
                    &record,
                    &effective.displayed_points.value,
                    &effective.aspected_points.value,
                )
                .map_err(view_computation_error)?;
            (prepared, effective)
        } else {
            let draft = state
                .session()?
                .draft_charts
                .iter()
                .find(|chart| chart.instance_id == chart_instance)
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::ViewComputation,
                        format!("Assigned chart {chart_instance} is not open"),
                    )
                })?;
            let effective = state.effective_configuration(&draft.draft.calculation, &view)?;
            let prepared = self
                .engine
                .resolve(
                    &draft.draft.record,
                    &effective.calculation.value,
                    &effective.displayed_points.value,
                    &effective.aspected_points.value,
                )
                .map_err(view_computation_error)?
                .with_context(SnapshotContext {
                    definition: None,
                    records: Vec::new(),
                    location_display_name: draft
                        .draft
                        .record
                        .location
                        .as_ref()
                        .map(|location| location.display_name.clone()),
                });
            (prepared, effective)
        };
        Ok((
            prepared,
            ViewCalculationPlan {
                displayed_points: effective.displayed_points.value,
                aspected_points: effective.aspected_points.value,
                aspect_set: effective.aspect_set.value,
                analysis: effective.analysis.value,
                wheel: effective.wheel.value,
                theme: effective.theme.value,
                rotation,
            },
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn submit_biwheel_refresh(
        &self,
        state: &mut RealState,
        view_id: ViewInstanceId,
    ) -> AppResult<()> {
        let session = state.session.as_ref().ok_or_else(|| {
            AppError::new(
                AppErrorKind::ViewComputation,
                "No workspace session is active",
            )
        })?;
        let view = session
            .document
            .views
            .iter()
            .find(|view| view.id == view_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("View {view_id} was not found"),
                )
            })?;
        let document =
            resolve_typed_binding(&view.document, &state.catalog, ConfigurationLayer::View)
                .map_err(view_resolution_error)?;
        let assignments = session.effective_chart_assignments(view_id);
        let required = document
            .value
            .chart_slots
            .iter()
            .filter(|slot| slot.required)
            .map(|slot| {
                assignments
                    .get(&slot.id)
                    .copied()
                    .map(|chart| (slot.id.clone(), chart))
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::InvalidIntent,
                            format!("Required slot {} is unassigned", slot.id),
                        )
                    })
            })
            .collect::<AppResult<Vec<_>>>()?;
        if required.len() != 2 || required[0].1 == required[1].1 {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "A biwheel requires exactly two distinct saved chart assignments",
            ));
        }
        let mut prepared = BTreeMap::new();
        let mut plan = None;
        for (slot, instance_id) in &required {
            let workspace_chart = session
                .document
                .chart_instances
                .iter()
                .find(|chart| chart.instance_id() == *instance_id)
                .ok_or_else(|| {
                    AppError::new(AppErrorKind::InvalidIntent, "Biwheel charts must be saved")
                })?;
            let mut definition = state
                .catalog
                .chart_definition(workspace_chart.definition)
                .cloned()
                .ok_or_else(|| not_found_for_view("ChartDefinition", workspace_chart.definition))?;
            let record_id = match definition.payload.source {
                ChartSource::Radix { record } => record,
                ChartSource::Derived { .. } => {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "Derived biwheel charts are not supported",
                    ));
                }
            };
            let record = state
                .catalog
                .chart_record(record_id)
                .cloned()
                .ok_or_else(|| not_found_for_view("ChartRecord", record_id))?;
            let effective =
                state.effective_configuration(&definition.payload.calculation, &view)?;
            definition.payload.calculation = effective.calculation.value.clone();
            let slot_prepared = self
                .engine
                .prepare(
                    &definition,
                    &record,
                    &effective.displayed_points.value,
                    &effective.aspected_points.value,
                )
                .map_err(view_computation_error)?;
            if plan.is_none() {
                plan = Some(ViewCalculationPlan {
                    displayed_points: effective.displayed_points.value,
                    aspected_points: effective.aspected_points.value,
                    aspect_set: effective.aspect_set.value,
                    analysis: effective.analysis.value,
                    wheel: effective.wheel.value,
                    theme: effective.theme.value,
                    rotation: view.overrides.rotation,
                });
            }
            prepared.insert(slot.clone(), slot_prepared);
        }
        let plan = plan.expect("two required slots produce a plan");
        let generation = state
            .views
            .get(&view_id)
            .and_then(|runtime| runtime.multi.as_ref())
            .map_or(1, |multi| multi.generation.saturating_add(1));
        state
            .inflight
            .retain(|_, pending| pending.view_id != view_id);
        let mut expected_map = BTreeMap::new();
        let mut cached = Vec::new();
        for (slot, slot_prepared) in &prepared {
            let request_id = state.next_request_id;
            state.next_request_id = request_id
                .next()
                .map_err(|error| AppError::new(AppErrorKind::Unavailable, error.to_string()))?;
            let expected = ExpectedCalculation {
                request_id,
                calc_key: slot_prepared.calc_key.clone(),
                analysis_key: AnalysisKey::derive(
                    std::slice::from_ref(&slot_prepared.calc_key),
                    &plan.aspected_points,
                    &plan.aspect_set,
                    &plan.analysis,
                )
                .map_err(view_computation_error)?,
            };
            expected_map.insert(slot.clone(), expected.clone());
            if let Some(calculation) = state.cache.calculation(&slot_prepared.calc_key).cloned() {
                cached.push(PendingCachedMulti {
                    view_id,
                    slot: slot.clone(),
                    generation,
                    expected,
                    calculation,
                });
            } else {
                self.runtime
                    .submit(CalculationWorkerRequest {
                        protocol_version: WorkerProtocolVersion::CURRENT,
                        request_id,
                        calc_key: slot_prepared.calc_key.clone(),
                        backend: self.engine.backend_descriptor().fingerprint.clone(),
                        request: slot_prepared.request.clone(),
                    })
                    .map_err(|error| AppError::new(AppErrorKind::ViewComputation, error.message))?;
                state.inflight.insert(
                    request_id,
                    PendingViewCalculation {
                        view_id,
                        slot: Some(slot.clone()),
                        generation,
                        prepared: slot_prepared.clone(),
                        plan: plan.clone(),
                    },
                );
            }
        }
        let runtime = state.views.entry(view_id).or_default();
        runtime.expected = None;
        runtime.multi = Some(MultiViewRuntime {
            generation,
            expected: expected_map,
            prepared,
            calculations: BTreeMap::new(),
            plan,
        });
        runtime.computation = if runtime.scene.is_some() {
            ViewComputationState::Refreshing
        } else {
            ViewComputationState::Loading
        };
        for item in cached {
            state
                .pending
                .push_back(PendingWork::CompleteCachedMulti(Box::new(item)));
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn finish_multi_if_ready(state: &mut RealState, view_id: ViewInstanceId) -> AppResult<()> {
        let Some(multi) = state
            .views
            .get(&view_id)
            .and_then(|runtime| runtime.multi.as_ref())
        else {
            return Ok(());
        };
        if multi.calculations.len() != multi.expected.len() {
            return state.advance();
        }
        let multi = multi.clone();
        let mut slots = multi.prepared.keys().cloned().collect::<Vec<_>>();
        slots.sort_by_key(|slot| i32::from(slot.as_str() != "radix"));
        let radix_slot = slots[0].clone();
        let comparison_slot = slots[1].clone();
        let radix = CalculationEngine::snapshot(
            multi.prepared.get(&radix_slot).expect("prepared radix"),
            multi
                .calculations
                .get(&radix_slot)
                .expect("calculated radix")
                .clone(),
        );
        let comparison = CalculationEngine::snapshot(
            multi
                .prepared
                .get(&comparison_slot)
                .expect("prepared comparison"),
            multi
                .calculations
                .get(&comparison_slot)
                .expect("calculated comparison")
                .clone(),
        );
        let radix_analysis = AspectAnalyzer::analyze(
            &radix,
            &multi.plan.aspected_points,
            &multi.plan.aspect_set,
            &multi.plan.analysis,
        )
        .map_err(view_computation_error)?;
        let comparison_analysis = AspectAnalyzer::analyze(
            &comparison,
            &multi.plan.aspected_points,
            &multi.plan.aspect_set,
            &multi.plan.analysis,
        )
        .map_err(view_computation_error)?;
        let relationship = AspectAnalyzer::analyze_relationship(
            &radix_slot,
            &radix,
            &comparison_slot,
            &comparison,
            &multi.plan.aspected_points,
            &multi.plan.aspect_set,
            &multi.plan.analysis,
        )
        .map_err(view_computation_error)?;
        let mut layout = mirabile_engine::layout_biwheel_in_bounds(
            radix_slot.clone(),
            &radix,
            &radix_analysis,
            comparison_slot.clone(),
            &comparison,
            &comparison_analysis,
            &relationship,
            &multi.plan.displayed_points,
            &multi.plan.wheel,
            multi.plan.rotation,
            mirabile_engine::WheelLayoutBounds::default(),
        )
        .map_err(view_computation_error)?;
        let overrides = state
            .session
            .as_ref()
            .and_then(|session| {
                session
                    .document
                    .views
                    .iter()
                    .find(|view| view.id == view_id)
            })
            .map(|view| view.overrides.clone())
            .unwrap_or_default();
        layout.points.retain(|point| {
            point.chart_slot.as_ref().is_none_or(|slot| {
                !overrides.hidden_rings.contains(slot)
                    && !overrides
                        .hidden_points_by_slot
                        .get(slot)
                        .is_some_and(|points| points.contains(&point.point))
                    && !overrides.hidden_points.contains(&point.point)
            })
        });
        layout.aspects.retain(|aspect| match aspect.layer {
            mirabile_engine::AspectLayer::RadixIntra => overrides.aspect_layers.radix_intra,
            mirabile_engine::AspectLayer::ComparisonIntra => {
                overrides.aspect_layers.comparison_intra
            }
            mirabile_engine::AspectLayer::CrossChart => overrides.aspect_layers.cross_chart,
        });
        render_key(&layout, &multi.plan.theme).map_err(view_computation_error)?;
        let scene = Scene::from_wheel(&layout).with_theme(multi.plan.theme.clone());
        let runtime = state.views.entry(view_id).or_default();
        runtime.scene = Some(scene);
        runtime.semantic_calculation = multi.calculations.get(&radix_slot).cloned();
        runtime.semantic_analysis = Some(radix_analysis);
        runtime.multi = None;
        runtime.computation = ViewComputationState::Fresh;
        state.notice = Some(success("Biwheel computation completed"));
        state.advance()
    }

    pub(super) fn finish_scene(
        state: &mut RealState,
        prepared: &PreparedCalculation,
        plan: &ViewCalculationPlan,
        calculation: mirabile_engine::CalculationValue,
    ) -> AppResult<Scene> {
        let snapshot = CalculationEngine::snapshot(prepared, calculation);
        let analysis = AspectAnalyzer::analyze(
            &snapshot,
            &plan.aspected_points,
            &plan.aspect_set,
            &plan.analysis,
        )
        .map_err(view_computation_error)?;
        state.cache.insert_analysis(analysis.clone());
        let layout = mirabile_engine::layout_wheel_with_rotation(
            &snapshot,
            &analysis,
            &plan.displayed_points,
            &plan.wheel,
            plan.rotation,
        )
        .map_err(view_computation_error)?;
        render_key(&layout, &plan.theme).map_err(view_computation_error)?;
        Ok(Scene::from_wheel(&layout).with_theme(plan.theme.clone()))
    }
}
