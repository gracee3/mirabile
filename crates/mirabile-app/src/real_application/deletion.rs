use super::{
    AppError, AppErrorKind, AppNotice, AppNoticeKind, AppResult, CanonicalResource, ChartSource,
    DerivationSpec, RealApplication, RealState, RepositoryError, ResourceId, ResourceRepository,
    ResourceState, Revision, Timestamp,
};
use crate::RepositoryDeletionReadModel;

impl RealState {
    pub(super) fn repository_deletion_read_model(&self) -> Option<RepositoryDeletionReadModel> {
        let selection = self.repository_selection.as_ref()?;
        let ResourceState::Present(resource) = selection.history.last()? else {
            return None;
        };
        let blockers = self.deletion_blockers(resource.id());
        Some(RepositoryDeletionReadModel {
            resource_id: resource.id(),
            expected_revision: resource.revision(),
            references: blockers.clone(),
            enabled: blockers.is_empty(),
            disabled_reason: (!blockers.is_empty()).then(|| blockers.join("; ")),
            first_confirmation_complete: self.delete_confirmation
                == Some((resource.id(), resource.revision())),
        })
    }

    fn deletion_blockers(&self, id: ResourceId) -> Vec<String> {
        let mut blockers = Vec::new();
        if self
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.id == id)
        {
            blockers.push("The resource is the active saved workspace".into());
        }
        if self
            .resource_drafts
            .values()
            .any(|draft| draft.read_model().resource_id == Some(id))
        {
            blockers.push("The resource has an active typed editor".into());
        }
        if self
            .editor
            .as_ref()
            .and_then(|editor| editor.base.as_ref())
            .is_some_and(|base| base.id == id)
        {
            blockers.push("The resource has an active Aspect Set editor".into());
        }
        if self.chart_editor.as_ref().is_some_and(|editor| {
            matches!(
                editor.target,
                crate::ChartEditorTarget::Saved {
                    record_id,
                    definition_id,
                    ..
                } if record_id == id || definition_id == id
            )
        }) {
            blockers.push("The resource belongs to the active composite chart editor".into());
        }
        for resource in self.catalog.current.values() {
            if resource.id() != id {
                collect_references(resource, id, &mut blockers);
            }
        }
        if let Some(session) = &self.session {
            collect_workspace_references(
                &session.document,
                id,
                "Active workspace session",
                &mut blockers,
            );
        }
        blockers.sort();
        blockers.dedup();
        blockers
    }
}

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: super::CalculationRuntime,
{
    pub(super) fn begin_delete_resource(
        &self,
        resource_id: ResourceId,
        expected_revision: Revision,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let deletion = state.repository_deletion_read_model().ok_or_else(|| {
            AppError::new(
                AppErrorKind::InvalidIntent,
                "Select a present resource before deletion",
            )
        })?;
        if deletion.resource_id != resource_id || deletion.expected_revision != expected_revision {
            return Err(AppError::new(
                AppErrorKind::Conflict,
                "The selected resource revision changed before deletion confirmation",
            ));
        }
        if !deletion.enabled {
            return Err(AppError::new(
                AppErrorKind::InvalidIntent,
                deletion
                    .disabled_reason
                    .unwrap_or_else(|| "Resource deletion is blocked".into()),
            ));
        }
        state.delete_confirmation = Some((resource_id, expected_revision));
        state.notice = Some(AppNotice {
            kind: AppNoticeKind::Warning,
            message:
                "First deletion confirmation recorded; use the second action to create a tombstone"
                    .into(),
        });
        state.advance()
    }

    pub(super) async fn confirm_delete_resource(
        &self,
        resource_id: ResourceId,
        expected_revision: Revision,
    ) -> AppResult<()> {
        let deleted_at = {
            let state = self.state.borrow();
            let deletion = state.repository_deletion_read_model().ok_or_else(|| {
                AppError::new(
                    AppErrorKind::InvalidIntent,
                    "Select a present resource before deletion",
                )
            })?;
            if state.delete_confirmation != Some((resource_id, expected_revision)) {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    "Complete the first deletion confirmation action",
                ));
            }
            if !deletion.enabled {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    deletion
                        .disabled_reason
                        .unwrap_or_else(|| "Resource deletion is blocked".into()),
                ));
            }
            Timestamp::from_unix_millis(state.next_timestamp)
        };
        let tombstone = self
            .repository
            .delete(resource_id, expected_revision, deleted_at)
            .await
            .map_err(|error| delete_error(&error))?;
        let history = self
            .repository
            .list_revisions(resource_id)
            .await
            .map_err(|error| delete_error(&error))?;
        let mut state = self.state.borrow_mut();
        state.catalog.insert_head(ResourceState::Deleted(tombstone));
        state.repository_selection = Some(super::RepositorySelection {
            resource_id,
            history,
        });
        state.delete_confirmation = None;
        state.next_timestamp = state.next_timestamp.saturating_add(1);
        state.notice = Some(AppNotice {
            kind: AppNoticeKind::Info,
            message:
                "Resource deleted; its tombstone and immutable revision history remain inspectable"
                    .into(),
        });
        state.advance()
    }
}

fn delete_error(error: &RepositoryError) -> AppError {
    let kind = if matches!(error, RepositoryError::Conflict { .. }) {
        AppErrorKind::Conflict
    } else {
        AppErrorKind::Unavailable
    };
    AppError::new(kind, format!("Could not delete resource: {error}"))
}

fn collect_references(resource: &CanonicalResource, target: ResourceId, output: &mut Vec<String>) {
    match resource {
        CanonicalResource::ChartDefinition(definition) => match &definition.payload.source {
            ChartSource::Radix { record } if *record == target => output.push(format!(
                "Chart Definition '{}' references this Chart Record",
                definition.title
            )),
            ChartSource::Derived { recipe } => collect_derivation_references(
                recipe,
                target,
                &format!("Chart Definition '{}'", definition.title),
                output,
            ),
            ChartSource::Radix { .. } => {}
        },
        CanonicalResource::WorkspaceDocument(workspace) => collect_workspace_references(
            &workspace.payload,
            target,
            &format!("Workspace '{}'", workspace.title),
            output,
        ),
        _ => {}
    }
}

fn collect_derivation_references(
    recipe: &DerivationSpec,
    target: ResourceId,
    owner: &str,
    output: &mut Vec<String>,
) {
    let referenced = match recipe {
        DerivationSpec::Harmonic { radix, .. } | DerivationSpec::Relocation { radix, .. } => {
            *radix == target
        }
        DerivationSpec::Composite { charts, .. } => charts.contains(&target),
        DerivationSpec::Transit { .. } => false,
    };
    if referenced {
        output.push(format!("{owner} derives from this Chart Definition"));
    }
}

fn collect_workspace_references(
    workspace: &super::WorkspaceDocument,
    target: ResourceId,
    owner: &str,
    output: &mut Vec<String>,
) {
    if workspace
        .chart_instances
        .iter()
        .any(|chart| chart.definition == target)
    {
        output.push(format!("{owner} contains this Chart Definition"));
    }
    let bindings = [
        workspace.profile.displayed_points.id(),
        workspace.profile.aspected_points.id(),
        workspace.profile.transit_points.id(),
        workspace.profile.aspects.id(),
        workspace.profile.analysis.id(),
        workspace.profile.theme.id(),
        workspace.profile.wheel.id(),
    ];
    if bindings.into_iter().flatten().any(|id| id == target)
        || workspace
            .views
            .iter()
            .filter_map(|view| view.document.id())
            .any(|id| id == target)
    {
        output.push(format!("{owner} binds this resource"));
    }
}
