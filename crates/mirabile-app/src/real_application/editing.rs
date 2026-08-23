use super::{
    AppError, AppErrorKind, AppNotice, AppNoticeKind, AppResult, AspectSet, AspectSetDraftMutation,
    AspectSetEditor, CalculationRuntime, CanonicalResource, ChartDefinition, ChartSource,
    DomainValidate, DraftState, InstanceId, PendingWork, RealApplication, RepositoryError,
    ResourceEnvelope, ResourceId, ResourceRepository, Revision, Timestamp, WorkspaceDocumentChart,
    conflict_refresh_warning, conjunction, info, not_found, repository_app_error,
    restore_dirty_editor, success,
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
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

    pub(super) async fn save_chart_draft(&self, instance_id: InstanceId) -> AppResult<()> {
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

        let result = self
            .repository
            .create_batch(vec![
                CanonicalResource::ChartRecord(record.clone()),
                CanonicalResource::ChartDefinition(definition.clone()),
            ])
            .await;
        if let Err(error) = result {
            self.state
                .borrow_mut()
                .saving_chart_drafts
                .remove(&instance_id);
            return Err(repository_app_error(
                "Could not atomically save the ChartDraft",
                &error,
            ));
        }

        let mut state = self.state.borrow_mut();
        state.saving_chart_drafts.remove(&instance_id);
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
        session.draft_charts.remove(index);
        session.selected_charts.retain(|id| *id != instance_id);
        for view in &mut session.document.views {
            view.charts.retain(|_, chart| *chart != instance_id);
        }
        if session.active_chart == Some(instance_id) {
            session.active_chart = session
                .document
                .chart_instances
                .first()
                .map(|chart| chart.instance_id)
                .or_else(|| session.draft_charts.first().map(|chart| chart.instance_id));
        }
        state.notice = Some(info(
            "Chart draft canceled; no canonical resources were created",
        ));
        state.advance()
    }

    pub(super) fn begin_aspect_set_edit(&self, resource_id: ResourceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if state
            .editor
            .as_ref()
            .is_some_and(|editor| matches!(editor.state, DraftState::Saving { .. }))
        {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Wait for the current Aspect Set save to finish",
            ));
        }
        let envelope = state
            .catalog
            .aspect_set(resource_id)
            .cloned()
            .ok_or_else(|| not_found("Aspect Set", resource_id))?;
        conjunction(&envelope.payload)?;
        state.editor = Some(AspectSetEditor {
            base: envelope.clone(),
            draft: envelope.payload,
            state: DraftState::Clean {
                revision: envelope.revision,
            },
        });
        state.notice = Some(info("Aspect Set draft opened from the canonical revision"));
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
        if matches!(editor.state, DraftState::Saving { .. }) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The Aspect Set draft cannot change while it is saving",
            ));
        }
        let base_revision = editor.state.base_revision();
        match mutation {
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
            }
        }
        if !matches!(editor.state, DraftState::Conflict { .. }) {
            editor.state = DraftState::Dirty { base_revision };
        }
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Draft preview accepted; analysis is refreshing with the last good Scene retained",
        ));
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
        let DraftState::Dirty { base_revision } = editor.state else {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "Only a dirty Aspect Set draft can be saved",
            ));
        };
        let next = editor
            .base
            .next_with_payload(editor.draft.clone(), Timestamp::from_unix_millis(timestamp))
            .map_err(|error| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    format!("Aspect Set draft was invalid: {error}"),
                )
            })?;
        editor.state = DraftState::Saving { base_revision };
        state.pending.push_back(PendingWork::SaveAspectSet {
            expected_revision: base_revision,
            next,
        });
        state.notice = Some(info(
            "Saving the Aspect Set draft with optimistic revision checks",
        ));
        state.advance()
    }

    pub(super) fn cancel_draft(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        if state
            .editor
            .as_ref()
            .is_some_and(|editor| matches!(editor.state, DraftState::Saving { .. }))
        {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Wait for the Aspect Set save to finish before canceling",
            ));
        }
        let resource_id = state
            .editor
            .as_ref()
            .ok_or_else(|| {
                AppError::new(AppErrorKind::InvalidIntent, "There is no draft to cancel")
            })?
            .base
            .id;
        let canonical = state
            .catalog
            .aspect_set(resource_id)
            .cloned()
            .ok_or_else(|| not_found("Aspect Set", resource_id))?;
        let editor = state.editor.as_mut().expect("editor was checked");
        editor.base = canonical.clone();
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
    pub(super) async fn complete_aspect_set_save(
        &self,
        expected_revision: Revision,
        next: ResourceEnvelope<AspectSet>,
    ) -> AppResult<()> {
        let resource_id = next.id;
        match self
            .repository
            .save(
                expected_revision,
                CanonicalResource::AspectSet(next.clone()),
            )
            .await
        {
            Ok(()) => {
                let mut state = self.state.borrow_mut();
                state.next_timestamp = state.next_timestamp.saturating_add(1);
                state
                    .catalog
                    .insert_current(CanonicalResource::AspectSet(next.clone()));
                if let Some(editor) = state
                    .editor
                    .as_mut()
                    .filter(|editor| editor.base.id == resource_id)
                {
                    editor.base = next.clone();
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
                state.advance()
            }
            Err(RepositoryError::Conflict { actual, .. }) => {
                let remote = self.repository.get(resource_id).await;
                let mut state = self.state.borrow_mut();
                match remote {
                    Ok(Some(CanonicalResource::AspectSet(remote))) => {
                        state
                            .catalog
                            .insert_current(CanonicalResource::AspectSet(remote));
                        if let Some(editor) = state
                            .editor
                            .as_mut()
                            .filter(|editor| editor.base.id == resource_id)
                        {
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
                restore_dirty_editor(&mut state, resource_id, expected_revision);
                state.notice = Some(AppNotice {
                    kind: AppNoticeKind::Warning,
                    message: format!("Aspect Set save failed; the draft was retained: {error}"),
                });
                state.advance()
            }
        }
    }
}
