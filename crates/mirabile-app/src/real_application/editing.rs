use super::{
    AppError, AppErrorKind, AppNotice, AppNoticeKind, AppResult, AspectSet, AspectSetDraftMutation,
    AspectSetEditor, AtomicSaveBatch, CalculationRuntime, CanonicalResource, ChartDefinition,
    ChartEditorState, ChartMutation, ChartSource, DomainValidate, DraftState, InstanceId,
    PendingWork, RealApplication, RepositoryError, ResourceEnvelope, ResourceId,
    ResourceRepository, Revision, RevisionExpectation, Timestamp, WorkspaceDocumentChart,
    conflict_refresh_warning, conjunction, info, not_found, repository_app_error,
    restore_dirty_editor, success,
};

fn ensure_option_enabled<T: PartialEq>(
    options: &[crate::AuthoringOption<T>],
    value: &T,
) -> AppResult<()> {
    match options.iter().find(|option| &option.value == value) {
        Some(option) if option.enabled => Ok(()),
        Some(option) => Err(AppError::new(
            AppErrorKind::InvalidIntent,
            option
                .disabled_reason
                .clone()
                .unwrap_or_else(|| "The authoring choice is disabled".into()),
        )),
        None => Err(AppError::new(
            AppErrorKind::InvalidIntent,
            "The authoring choice is not available",
        )),
    }
}

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) fn begin_new_chart(&self) -> AppResult<()> {
        let instance_id = InstanceId::new();
        let editor = crate::ChartAuthoringEditor::new(
            instance_id,
            crate::startup::utc_civil_datetime((self.clock)()),
            self.engine
                .backend_descriptor()
                .authoring
                .default_corrections
                .clone(),
        );
        let draft = editor.last_valid.clone();
        let mut state = self.state.borrow_mut();
        if state.chart_editor.is_some() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Save or cancel the current chart editor before beginning another chart",
            ));
        }
        let active_view = state.session()?.active_view;
        let required_slot = active_view
            .map(|view_id| {
                state
                    .resolve_view_documents(state.workspace().expect("ready workspace"))?
                    .remove(&view_id)
                    .and_then(|document| {
                        document
                            .chart_slots
                            .into_iter()
                            .find(|slot| slot.required)
                            .map(|slot| slot.id)
                    })
                    .map(|slot| (view_id, slot))
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            "The active view has no required chart slot for preview",
                        )
                    })
            })
            .transpose()?;
        let session = state.session.as_mut().expect("ready session");
        session
            .draft_charts
            .push(crate::WorkspaceSessionDraftChart { instance_id, draft });
        session.active_chart = Some(instance_id);
        if let Some((view_id, slot)) = required_slot {
            session
                .draft_chart_assignments
                .entry(view_id)
                .or_default()
                .insert(slot, instance_id);
        }
        state.chart_editor = Some(editor);
        if active_view.is_some() {
            self.submit_active_view_refresh(&mut state)?;
        }
        state.notice = Some(info(
            "New chart editor opened with application-owned defaults and a session-only preview",
        ));
        state.advance()
    }

    pub(super) fn begin_saved_chart_edit(&self, instance_id: InstanceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if state.chart_editor.is_some() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Save or cancel the current chart editor before editing another chart",
            ));
        }
        let definition_id = state
            .session()?
            .document
            .chart_instances
            .iter()
            .find(|chart| chart.instance_id == instance_id)
            .map(|chart| chart.definition)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("Saved chart instance {instance_id} is not open"),
                )
            })?;
        let definition = state
            .catalog
            .chart_definition(definition_id)
            .cloned()
            .ok_or_else(|| not_found("ChartDefinition", definition_id))?;
        let record_id = match definition.payload.source {
            ChartSource::Radix { record } => record,
            ChartSource::Derived { .. } => {
                return Err(AppError::new(
                    AppErrorKind::Unavailable,
                    "Derived chart editing remains intentionally deferred",
                ));
            }
        };
        let record = state
            .catalog
            .chart_record(record_id)
            .cloned()
            .ok_or_else(|| not_found("ChartRecord", record_id))?;
        let shared_record = state.catalog.chart_record_reference_count(record_id) > 1;
        let editor =
            crate::ChartAuthoringEditor::from_saved(instance_id, record, definition, shared_record)
                .map_err(|message| AppError::new(AppErrorKind::Unavailable, message))?;
        state.session.as_mut().expect("ready session").active_chart = Some(instance_id);
        state.chart_editor = Some(editor);
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(if shared_record {
            "Saved definition editor opened; factual fields are protected because its ChartRecord is shared"
        } else {
            "Saved chart editor opened from independent Record and Definition revisions"
        }));
        state.advance()
    }

    pub(super) fn apply_chart_mutation(&self, mutation: ChartMutation) -> AppResult<()> {
        let descriptor = self.engine.backend_descriptor();
        let mut state = self.state.borrow_mut();
        let complete_location = state
            .chart_editor
            .as_ref()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    "Begin a chart edit before changing chart fields",
                )
            })?
            .location_complete();
        let capabilities =
            crate::AuthoringCapabilitiesReadModel::from_backend(descriptor, complete_location);
        if crate::ChartAuthoringEditor::is_factual_mutation(&mutation)
            && !state
                .chart_editor
                .as_ref()
                .expect("editor was checked")
                .factual_mutations_enabled()
        {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "This ChartRecord is shared by multiple definitions; copy/detach is required before factual editing",
            ));
        }
        match &mutation {
            ChartMutation::SetZodiac(value) => {
                let mode = match value {
                    mirabile_core::ZodiacSpec::Tropical => mirabile_engine::ZodiacMode::Tropical,
                    mirabile_core::ZodiacSpec::Sidereal { .. } => {
                        mirabile_engine::ZodiacMode::Sidereal
                    }
                };
                ensure_option_enabled(&capabilities.zodiac_modes, &mode)?;
            }
            ChartMutation::SetCoordinateSystem(value) => {
                ensure_option_enabled(&capabilities.coordinate_systems, value)?;
            }
            ChartMutation::SetHouseSystem(value) => {
                ensure_option_enabled(&capabilities.house_systems, value)?;
            }
            ChartMutation::SetTitle(_)
            | ChartMutation::SetEventKind(_)
            | ChartMutation::SetSubjectName(_)
            | ChartMutation::SetCivilDate(_)
            | ChartMutation::SetCivilTime(_)
            | ChartMutation::SetTimezone(_)
            | ChartMutation::SetLocationEnabled(_)
            | ChartMutation::SetLocationName(_)
            | ChartMutation::SetCountryRegion(_)
            | ChartMutation::SetLatitude(_)
            | ChartMutation::SetLongitude(_) => {}
        }
        let (instance_id, is_new, materialized) = {
            let editor = state.chart_editor.as_mut().expect("editor was checked");
            if editor.state == ChartEditorState::Saving {
                return Err(AppError::new(
                    AppErrorKind::Unavailable,
                    "The chart editor cannot change while saving",
                ));
            }
            if editor.state == ChartEditorState::Conflict {
                return Err(AppError::new(
                    AppErrorKind::Conflict,
                    "Cancel and reopen this chart to adopt the refreshed component heads before editing again",
                ));
            }
            (
                editor.instance_id(),
                matches!(editor.target, crate::ChartEditorTarget::New { .. }),
                editor.apply(mutation),
            )
        };
        if let Some(materialized) = materialized {
            if is_new {
                let session = state.session.as_mut().expect("ready session");
                let draft = session
                    .draft_charts
                    .iter_mut()
                    .find(|draft| draft.instance_id == instance_id)
                    .ok_or_else(|| {
                        AppError::new(AppErrorKind::NotFound, "The chart preview is not open")
                    })?;
                draft.draft = materialized;
            }
            self.submit_active_view_refresh(&mut state)?;
            state.notice = Some(info(
                "Chart mutation accepted; the authoritative preview is refreshing",
            ));
        } else {
            state.notice = Some(info(
                "Chart field accepted but is incomplete; the last valid preview is retained",
            ));
        }
        state.advance()
    }

    pub(super) fn begin_save_chart_editor(&self) -> AppResult<()> {
        let (instance_id, is_new) = {
            let state = self.state.borrow();
            let editor = state.chart_editor.as_ref().ok_or_else(|| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    "There is no chart editor to save",
                )
            })?;
            if !editor.validation.is_empty() {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    "Complete every invalid chart field before saving",
                ));
            }
            (
                editor.instance_id(),
                matches!(editor.target, crate::ChartEditorTarget::New { .. }),
            )
        };
        if is_new {
            self.begin_save_chart_draft(instance_id)?;
        } else {
            self.begin_save_saved_chart_editor()?;
        }
        if let Some(editor) = self.state.borrow_mut().chart_editor.as_mut() {
            editor.state = ChartEditorState::Saving;
        }
        Ok(())
    }

    pub(super) fn cancel_chart_editor(&self) -> AppResult<()> {
        let (instance_id, is_new) = self
            .state
            .borrow()
            .chart_editor
            .as_ref()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    "There is no chart editor to cancel",
                )
            })
            .map(|editor| {
                (
                    editor.instance_id(),
                    matches!(editor.target, crate::ChartEditorTarget::New { .. }),
                )
            })?;
        if is_new {
            self.cancel_chart_draft(instance_id)
        } else {
            let mut state = self.state.borrow_mut();
            state.chart_editor = None;
            self.submit_active_view_refresh(&mut state)?;
            state.notice = Some(info(
                "Saved chart edit canceled; canonical Record and Definition remain unchanged",
            ));
            state.advance()
        }
    }

    fn begin_save_saved_chart_editor(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let (instance_id, bases, draft) = {
            let editor = state.chart_editor.as_ref().expect("editor was checked");
            (
                editor.instance_id(),
                editor
                    .saved_bases()
                    .expect("saved chart target retains saved bases")
                    .clone(),
                editor.last_valid.clone(),
            )
        };
        let timestamp = Timestamp::from_unix_millis(state.next_timestamp);
        let mut changes = Vec::new();
        if draft.record != bases.record.payload {
            let next = bases
                .record
                .next_with_payload(draft.record, timestamp)
                .map_err(|error| AppError::new(AppErrorKind::Unavailable, error.to_string()))?;
            changes.push(CanonicalResource::ChartRecord(next));
        }
        let next_definition_payload = ChartDefinition {
            source: bases.definition.payload.source.clone(),
            calculation: draft.calculation,
        };
        if next_definition_payload != bases.definition.payload
            || draft.title != bases.definition.title
        {
            let mut next = bases
                .definition
                .next_with_payload(next_definition_payload, timestamp)
                .map_err(|error| AppError::new(AppErrorKind::Unavailable, error.to_string()))?;
            next.title = draft.title;
            changes.push(CanonicalResource::ChartDefinition(next));
        }
        let batch = AtomicSaveBatch {
            expectations: vec![
                RevisionExpectation {
                    id: bases.record.id,
                    expected_revision: bases.record.revision,
                },
                RevisionExpectation {
                    id: bases.definition.id,
                    expected_revision: bases.definition.revision,
                },
            ],
            changes,
        };
        state.pending.push_back(PendingWork::SaveChartEdit {
            instance_id,
            definition_id: bases.definition.id,
            batch,
        });
        state.notice = Some(info(
            "Comparing both chart component revisions before one atomic saved-chart publication",
        ));
        state.advance()
    }

    pub(super) async fn complete_saved_chart_save(
        &self,
        instance_id: InstanceId,
        _definition_id: ResourceId,
        batch: AtomicSaveBatch,
    ) -> AppResult<()> {
        let result = self.repository.save_batch(batch.clone()).await;
        match result {
            Ok(()) => {
                let mut state = self.state.borrow_mut();
                let changed = !batch.changes.is_empty();
                for resource in batch.changes {
                    state.catalog.insert_current(resource);
                }
                if changed {
                    state.next_timestamp = state.next_timestamp.saturating_add(1);
                }
                if state
                    .chart_editor
                    .as_ref()
                    .is_some_and(|editor| editor.instance_id() == instance_id)
                {
                    state.chart_editor = None;
                }
                self.submit_active_view_refresh(&mut state)?;
                state.notice = Some(success(if changed {
                    "Saved chart changes published atomically with both component revisions checked"
                } else {
                    "Saved chart was unchanged; both component revisions were verified"
                }));
                state.advance()
            }
            Err(RepositoryError::BatchConflict { conflicts }) => {
                let mut refreshed = Vec::new();
                let mut refresh_failed = false;
                for conflict in &conflicts {
                    match self.repository.get(conflict.id).await {
                        Ok(Some(resource)) => refreshed.push(resource),
                        Ok(None) | Err(_) => refresh_failed = true,
                    }
                }
                let mut state = self.state.borrow_mut();
                for resource in refreshed {
                    state.catalog.insert_current(resource);
                }
                if let Some(editor) = state
                    .chart_editor
                    .as_mut()
                    .filter(|editor| editor.instance_id() == instance_id)
                {
                    let bases = editor
                        .saved_bases()
                        .expect("saved editor retains component bases");
                    let record_id = bases.record.id;
                    editor.state = ChartEditorState::Conflict;
                    editor.conflicts = conflicts
                        .iter()
                        .map(|conflict| crate::ChartEditorConflict {
                            component: if conflict.id == record_id {
                                crate::ChartConflictComponent::Record
                            } else {
                                crate::ChartConflictComponent::Definition
                            },
                            resource_id: conflict.id,
                            expected_revision: conflict.expected,
                            actual_revision: conflict.actual,
                        })
                        .collect();
                }
                state.notice = Some(AppNotice {
                    kind: AppNoticeKind::Conflict,
                    message: if refresh_failed {
                        "Saved chart conflict detected; at least one current component head could not be refreshed"
                            .into()
                    } else {
                        "Saved chart conflict detected; current component heads were refreshed while the local editor was retained"
                            .into()
                    },
                });
                state.advance()
            }
            Err(error) => {
                let failure = repository_app_error("Could not atomically save the chart", &error);
                let mut state = self.state.borrow_mut();
                if let Some(editor) = state
                    .chart_editor
                    .as_mut()
                    .filter(|editor| editor.instance_id() == instance_id)
                {
                    editor.state = ChartEditorState::Dirty;
                }
                state.notice = Some(AppNotice {
                    kind: AppNoticeKind::Warning,
                    message: failure.message,
                });
                state.advance()
            }
        }
    }

    pub(super) fn start_chart_draft(&self, draft: crate::ChartDraft) -> AppResult<()> {
        if draft.title.trim().is_empty() {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "A chart draft title must not be empty",
            ));
        }
        draft.record.domain_validate().map_err(|error| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                format!("ChartDraft record is invalid: {error}"),
            )
        })?;
        draft.calculation.domain_validate().map_err(|error| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                format!("ChartDraft calculation is invalid: {error}"),
            )
        })?;
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let instance_id = InstanceId::new();
        session
            .draft_charts
            .push(crate::WorkspaceSessionDraftChart { instance_id, draft });
        session.active_chart = Some(instance_id);
        state.notice = Some(info(
            "Chart draft started without creating canonical resources",
        ));
        state.advance()
    }

    pub(super) fn begin_save_chart_draft(&self, instance_id: InstanceId) -> AppResult<()> {
        let (record, definition) = {
            let mut state = self.state.borrow_mut();
            let draft = state
                .session()?
                .draft_charts
                .iter()
                .find(|chart| chart.instance_id == instance_id)
                .map(|chart| chart.draft.clone())
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorKind::NotFound,
                        format!("Chart draft {instance_id} is not open"),
                    )
                })?;
            let timestamp = Timestamp::from_unix_millis(state.next_timestamp);
            let record_id = ResourceId::new();
            let record = ResourceEnvelope::with_id(
                record_id,
                format!("{} source", draft.title),
                draft.record,
                timestamp,
            );
            let definition = ResourceEnvelope::with_id(
                ResourceId::new(),
                draft.title,
                ChartDefinition {
                    source: ChartSource::Radix { record: record_id },
                    calculation: draft.calculation,
                },
                timestamp,
            );
            state.saving_chart_drafts.insert(instance_id);
            (record, definition)
        };

        let mut state = self.state.borrow_mut();
        state.pending.push_back(PendingWork::CreateChart {
            instance_id,
            record: Box::new(record),
            definition: Box::new(definition),
        });
        state.notice = Some(info(
            "Creating the ChartRecord and ChartDefinition as one observable atomic operation",
        ));
        state.advance()
    }

    pub(super) async fn complete_chart_create(
        &self,
        instance_id: InstanceId,
        record: ResourceEnvelope<mirabile_core::ChartRecord>,
        definition: ResourceEnvelope<ChartDefinition>,
    ) -> AppResult<()> {
        let result = self
            .repository
            .create_batch(vec![
                CanonicalResource::ChartRecord(record.clone()),
                CanonicalResource::ChartDefinition(definition.clone()),
            ])
            .await;
        if let Err(error) = result {
            let failure = repository_app_error("Could not atomically save the ChartDraft", &error);
            let mut state = self.state.borrow_mut();
            state.saving_chart_drafts.remove(&instance_id);
            if let Some(editor) = state
                .chart_editor
                .as_mut()
                .filter(|editor| editor.instance_id() == instance_id)
            {
                editor.state = ChartEditorState::Dirty;
            }
            state.notice = Some(AppNotice {
                kind: if failure.kind == AppErrorKind::Conflict {
                    AppNoticeKind::Conflict
                } else {
                    AppNoticeKind::Warning
                },
                message: failure.message,
            });
            return state.advance();
        }

        let mut state = self.state.borrow_mut();
        state.saving_chart_drafts.remove(&instance_id);
        if state
            .chart_editor
            .as_ref()
            .is_some_and(|editor| editor.instance_id() == instance_id)
        {
            state.chart_editor = None;
        }
        state.next_timestamp = state.next_timestamp.saturating_add(1);
        state
            .catalog
            .insert_current(CanonicalResource::ChartRecord(record));
        state
            .catalog
            .insert_current(CanonicalResource::ChartDefinition(definition.clone()));
        let session = state
            .session
            .as_mut()
            .expect("a ready application has a workspace session");
        session
            .draft_charts
            .retain(|chart| chart.instance_id != instance_id);
        session
            .document
            .chart_instances
            .push(WorkspaceDocumentChart {
                instance_id,
                definition: definition.id,
            });
        session.promote_draft_assignments(instance_id);
        session.mark_document_dirty();
        state.notice = Some(success(
            "ChartRecord and ChartDefinition were created atomically; save the workspace to persist membership",
        ));
        state.advance()
    }

    pub(super) fn cancel_chart_draft(&self, instance_id: InstanceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let index = session
            .draft_charts
            .iter()
            .position(|chart| chart.instance_id == instance_id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::NotFound,
                    format!("Chart draft {instance_id} is not open"),
                )
            })?;
        let refresh_active_view = session.active_view.is_some_and(|view_id| {
            session
                .draft_chart_assignments
                .get(&view_id)
                .is_some_and(|assignments| assignments.values().any(|chart| *chart == instance_id))
        });
        session.draft_charts.remove(index);
        session.selected_charts.retain(|id| *id != instance_id);
        session.remove_draft_assignments(instance_id);
        if session.active_chart == Some(instance_id) {
            session.active_chart = session
                .document
                .chart_instances
                .first()
                .map(|chart| chart.instance_id)
                .or_else(|| session.draft_charts.first().map(|chart| chart.instance_id));
        }
        if refresh_active_view {
            self.submit_active_view_refresh(&mut state)?;
        }
        if state
            .chart_editor
            .as_ref()
            .is_some_and(|editor| editor.instance_id() == instance_id)
        {
            state.chart_editor = None;
        }
        state.notice = Some(info(
            "Chart draft canceled; no canonical resources were created",
        ));
        state.advance()
    }

    pub(super) fn begin_aspect_set_edit(&self, resource_id: ResourceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        ensure_aspect_editor_can_be_replaced(state.editor.as_ref())?;
        let envelope = state
            .catalog
            .aspect_set(resource_id)
            .cloned()
            .ok_or_else(|| not_found("Aspect Set", resource_id))?;
        conjunction(&envelope.payload)?;
        state.editor = Some(AspectSetEditor {
            base: Some(envelope.clone()),
            title: envelope.title.clone(),
            draft: envelope.payload,
            state: DraftState::Clean {
                revision: envelope.revision,
            },
        });
        state.notice = Some(info("Aspect Set draft opened from the canonical revision"));
        state.advance()
    }

    pub(super) fn begin_new_aspect_set(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        ensure_aspect_editor_can_be_replaced(state.editor.as_ref())?;
        state.editor = Some(AspectSetEditor {
            base: None,
            title: "Untitled Aspect Set".into(),
            draft: authoring_aspect_set(),
            state: DraftState::New,
        });
        state.notice = Some(info(
            "New Aspect Set opened with the supported Conjunction and Square vocabulary",
        ));
        state.advance()
    }

    pub(super) fn duplicate_aspect_set(&self, resource_id: ResourceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        ensure_aspect_editor_can_be_replaced(state.editor.as_ref())?;
        let source = state
            .catalog
            .aspect_set(resource_id)
            .cloned()
            .ok_or_else(|| not_found("Aspect Set", resource_id))?;
        state.editor = Some(AspectSetEditor {
            base: None,
            title: format!("{} Copy", source.title),
            draft: source.payload,
            state: DraftState::New,
        });
        state.notice = Some(info(
            "Aspect Set duplicated as a new unsaved resource with every row preserved",
        ));
        state.advance()
    }

    pub(super) fn update_aspect_set_draft(
        &self,
        mutation: AspectSetDraftMutation,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let editor = state.editor.as_mut().ok_or_else(|| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                "Begin an Aspect Set edit before updating the draft",
            )
        })?;
        if matches!(
            editor.state,
            DraftState::Saving { .. } | DraftState::Creating
        ) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The Aspect Set draft cannot change while it is saving",
            ));
        }
        let base_revision = editor.state.base_revision();
        let affects_analysis = match mutation {
            AspectSetDraftMutation::SetTitle(title) => {
                let title = title.trim();
                if title.is_empty() {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        "An Aspect Set title must not be empty",
                    ));
                }
                editor.title = title.into();
                false
            }
            AspectSetDraftMutation::SetOrb { aspect_id, maximum } => {
                let aspect = editor
                    .draft
                    .aspects
                    .iter_mut()
                    .find(|aspect| aspect.id == aspect_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("Aspect {aspect_id} was not found in the draft"),
                        )
                    })?;
                aspect.orbs.maximum = maximum;
                true
            }
            AspectSetDraftMutation::SetEnabled { aspect_id, enabled } => {
                let aspect = editor
                    .draft
                    .aspects
                    .iter_mut()
                    .find(|aspect| aspect.id == aspect_id)
                    .ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::NotFound,
                            format!("Aspect {aspect_id} was not found in the draft"),
                        )
                    })?;
                aspect.enabled = enabled;
                true
            }
        };
        if !matches!(editor.state, DraftState::New | DraftState::Conflict { .. }) {
            editor.state = DraftState::Dirty {
                base_revision: base_revision.expect("saved draft has a base revision"),
            };
        }
        if affects_analysis && editor.base.is_some() {
            self.submit_active_view_refresh(&mut state)?;
            state.notice = Some(info(
                "Draft preview accepted; analysis is refreshing with the last good Scene retained",
            ));
        } else {
            state.notice = Some(info(
                "Aspect Set metadata or an unbound new resource changed without invalidating the current analysis",
            ));
        }
        state.advance()
    }

    pub(super) fn begin_save_draft(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let timestamp = state.next_timestamp;
        let editor = state.editor.as_mut().ok_or_else(|| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                "There is no Aspect Set draft to save",
            )
        })?;
        let (expected_revision, mut next) = match editor.state {
            DraftState::New => (
                None,
                ResourceEnvelope::new(
                    editor.title.clone(),
                    editor.draft.clone(),
                    Timestamp::from_unix_millis(timestamp),
                ),
            ),
            DraftState::Dirty { base_revision } => {
                let next = editor
                    .base
                    .as_ref()
                    .expect("saved editor has a base")
                    .next_with_payload(editor.draft.clone(), Timestamp::from_unix_millis(timestamp))
                    .map_err(|error| {
                        AppError::new(
                            AppErrorKind::InvalidIntent,
                            format!("Aspect Set draft was invalid: {error}"),
                        )
                    })?;
                (Some(base_revision), next)
            }
            _ => {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    "Only a new or dirty Aspect Set draft can be saved",
                ));
            }
        };
        next.title.clone_from(&editor.title);
        next.validate().map_err(|error| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                format!("Aspect Set draft was invalid: {error}"),
            )
        })?;
        editor.state = expected_revision.map_or(DraftState::Creating, |base_revision| {
            DraftState::Saving { base_revision }
        });
        state.pending.push_back(PendingWork::SaveAspectSet {
            expected_revision,
            next,
        });
        state.notice = Some(info(
            "Publishing the Aspect Set draft with the applicable revision checks",
        ));
        state.advance()
    }

    pub(super) fn cancel_draft(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if state.editor.as_ref().is_some_and(|editor| {
            matches!(
                editor.state,
                DraftState::Saving { .. } | DraftState::Creating
            )
        }) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Wait for the Aspect Set save to finish before canceling",
            ));
        }
        let editor = state.editor.as_ref().ok_or_else(|| {
            AppError::new(AppErrorKind::InvalidIntent, "There is no draft to cancel")
        })?;
        let Some(resource_id) = editor.base.as_ref().map(|base| base.id) else {
            state.editor = None;
            state.notice = Some(info(
                "New Aspect Set canceled without creating a canonical resource",
            ));
            return state.advance();
        };
        let canonical = state
            .catalog
            .aspect_set(resource_id)
            .cloned()
            .ok_or_else(|| not_found("Aspect Set", resource_id))?;
        let editor = state.editor.as_mut().expect("editor was checked");
        editor.base = Some(canonical.clone());
        editor.title.clone_from(&canonical.title);
        editor.draft = canonical.payload;
        editor.state = DraftState::Clean {
            revision: canonical.revision,
        };
        state.pending.retain(|pending| {
            !matches!(pending, PendingWork::SaveAspectSet { next, .. } if next.id == resource_id)
        });
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Draft canceled; canonical Aspect Set semantics restored without a repository write",
        ));
        state.advance()
    }
    #[allow(clippy::too_many_lines)]
    pub(super) async fn complete_aspect_set_save(
        &self,
        expected_revision: Option<Revision>,
        next: ResourceEnvelope<AspectSet>,
    ) -> AppResult<()> {
        let resource_id = next.id;
        let result = if let Some(expected_revision) = expected_revision {
            self.repository
                .save(
                    expected_revision,
                    CanonicalResource::AspectSet(next.clone()),
                )
                .await
        } else {
            self.repository
                .create(CanonicalResource::AspectSet(next.clone()))
                .await
        };
        match result {
            Ok(()) => {
                let mut state = self.state.borrow_mut();
                state.next_timestamp = state.next_timestamp.saturating_add(1);
                state
                    .catalog
                    .insert_current(CanonicalResource::AspectSet(next.clone()));
                if let Some(editor) = state.editor.as_mut().filter(|editor| {
                    editor
                        .base
                        .as_ref()
                        .is_none_or(|base| base.id == resource_id)
                }) {
                    editor.base = Some(next.clone());
                    editor.title.clone_from(&next.title);
                    editor.draft = next.payload;
                    editor.state = DraftState::Clean {
                        revision: next.revision,
                    };
                    state.notice = Some(success(format!(
                        "Aspect Set saved as canonical revision {}",
                        next.revision
                    )));
                } else {
                    state.notice = Some(AppNotice {
                        kind: AppNoticeKind::Warning,
                        message: format!(
                            "Aspect Set revision {} was saved, but its editor was no longer open",
                            next.revision
                        ),
                    });
                }
                if expected_revision.is_none() {
                    let session = state.session.as_mut().ok_or_else(|| {
                        AppError::new(
                            AppErrorKind::Unavailable,
                            "No workspace session is active for the new Aspect Set binding",
                        )
                    })?;
                    session.document.profile.aspects =
                        mirabile_core::ResourceBinding::Follow { id: resource_id };
                    session.mark_document_dirty();
                    self.submit_active_view_refresh(&mut state)?;
                    state.notice = Some(success(format!(
                        "Aspect Set created as canonical revision {} and bound to the working workspace",
                        next.revision
                    )));
                }
                state.advance()
            }
            Err(RepositoryError::Conflict { actual, .. }) => {
                let Some(expected_revision) = expected_revision else {
                    let mut state = self.state.borrow_mut();
                    if let Some(editor) =
                        state.editor.as_mut().filter(|editor| editor.base.is_none())
                    {
                        editor.state = DraftState::New;
                    }
                    state.notice = Some(AppNotice {
                        kind: AppNoticeKind::Conflict,
                        message: format!(
                            "New Aspect Set identity unexpectedly conflicted with revision {actual}; the unsaved editor was retained"
                        ),
                    });
                    return state.advance();
                };
                let remote = self.repository.get(resource_id).await;
                let mut state = self.state.borrow_mut();
                match remote {
                    Ok(Some(CanonicalResource::AspectSet(remote))) => {
                        state
                            .catalog
                            .insert_current(CanonicalResource::AspectSet(remote));
                        if let Some(editor) = state.editor.as_mut().filter(|editor| {
                            editor
                                .base
                                .as_ref()
                                .is_some_and(|base| base.id == resource_id)
                        }) {
                            editor.state = DraftState::Conflict {
                                base_revision: expected_revision,
                                remote_revision: actual,
                            };
                        }
                        state.notice = Some(AppNotice {
                            kind: AppNoticeKind::Conflict,
                            message: format!(
                                "Aspect Set save conflict: draft revision {expected_revision}, remote revision {actual}; the local draft was retained"
                            ),
                        });
                    }
                    Ok(Some(remote)) => {
                        restore_dirty_editor(&mut state, resource_id, expected_revision);
                        state.notice = Some(conflict_refresh_warning(format!(
                            "resource {resource_id} was {:?}, not an AspectSet",
                            remote.kind()
                        )));
                    }
                    Ok(None) => {
                        restore_dirty_editor(&mut state, resource_id, expected_revision);
                        state.notice = Some(conflict_refresh_warning(format!(
                            "resource {resource_id} was not found"
                        )));
                    }
                    Err(error) => {
                        restore_dirty_editor(&mut state, resource_id, expected_revision);
                        state.notice = Some(conflict_refresh_warning(error));
                    }
                }
                state.advance()
            }
            Err(error) => {
                let mut state = self.state.borrow_mut();
                if let Some(expected_revision) = expected_revision {
                    restore_dirty_editor(&mut state, resource_id, expected_revision);
                } else if let Some(editor) =
                    state.editor.as_mut().filter(|editor| editor.base.is_none())
                {
                    editor.state = DraftState::New;
                }
                state.notice = Some(AppNotice {
                    kind: AppNoticeKind::Warning,
                    message: format!("Aspect Set save failed; the draft was retained: {error}"),
                });
                state.advance()
            }
        }
    }
}

fn authoring_aspect_set() -> AspectSet {
    use mirabile_core::{AspectClass, AspectDefinition, AspectId, OrbPolicy};

    let angle = |degrees| {
        crate::Angle::from_degrees(degrees).expect("built-in Aspect Set angles are valid")
    };
    AspectSet {
        aspects: vec![
            AspectDefinition {
                id: AspectId::new("conjunction").expect("built-in aspect ID is valid"),
                name: "Conjunction".into(),
                angle: angle(0.0),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: angle(8.0),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            },
            AspectDefinition {
                id: AspectId::new("square").expect("built-in aspect ID is valid"),
                name: "Square".into(),
                angle: angle(90.0),
                enabled: true,
                orbs: OrbPolicy {
                    maximum: angle(6.0),
                    applying_multiplier: 1.0,
                },
                classification: AspectClass::Major,
            },
        ],
    }
}

fn ensure_aspect_editor_can_be_replaced(editor: Option<&AspectSetEditor>) -> AppResult<()> {
    match editor.map(|editor| &editor.state) {
        None | Some(DraftState::Clean { .. }) => Ok(()),
        Some(DraftState::Saving { .. } | DraftState::Creating) => Err(AppError::new(
            AppErrorKind::Unavailable,
            "Wait for the current Aspect Set operation to finish",
        )),
        Some(DraftState::New | DraftState::Dirty { .. } | DraftState::Conflict { .. }) => {
            Err(AppError::new(
                AppErrorKind::Unavailable,
                "Save or cancel the current Aspect Set editor before opening another resource",
            ))
        }
    }
}
