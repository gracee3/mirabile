use super::{
    AppError, AppErrorKind, AppResult, CalculationRuntime, Catalog, DomainValidate, HydratedState,
    RealApplication, ResourceEnvelope, ResourceId, ResourceRepository, ResourceState,
    StartupPolicy, WorkspaceDocument, WorkspaceSession, blank_workspace_session,
    current_transits_session, initialization_error, resource_modified_at,
    validation::validate_session_references,
};

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub(super) async fn hydrate(&self) -> AppResult<HydratedState> {
        let heads = self.repository.list_heads(None).await.map_err(|error| {
            initialization_error("Could not load canonical resource heads", &error)
        })?;
        let mut catalog = Catalog::default();
        let mut latest_timestamp = 1;
        for head in heads {
            latest_timestamp = latest_timestamp.max(match &head {
                ResourceState::Present(resource) => resource_modified_at(resource),
                ResourceState::Deleted(tombstone) => tombstone.deleted_at.unix_millis(),
            });
            catalog.insert_head(head);
        }
        self.hydrate_pinned_revisions(&mut catalog).await?;

        let (workspace, session) = self.startup_session(&catalog)?;
        session.document.domain_validate().map_err(|error| {
            AppError::new(
                AppErrorKind::Initialization,
                format!("Startup WorkspaceDocument failed structural validation: {error}"),
            )
        })?;
        validate_session_references(&session, &catalog).map_err(|error| {
            AppError::new(
                AppErrorKind::Initialization,
                format!("Startup session failed referential validation: {error}"),
            )
        })?;
        Ok(HydratedState {
            catalog,
            workspace,
            session,
            next_timestamp: latest_timestamp.saturating_add(1),
        })
    }

    pub(super) fn startup_session(
        &self,
        catalog: &Catalog,
    ) -> AppResult<(
        Option<ResourceEnvelope<WorkspaceDocument>>,
        WorkspaceSession,
    )> {
        let policy = match &self.startup_policy {
            StartupPolicy::RestorePreviousSession => StartupPolicy::CurrentTransits,
            policy => policy.clone(),
        };
        match policy {
            StartupPolicy::CurrentTransits | StartupPolicy::RestorePreviousSession => Ok((
                None,
                current_transits_session((self.clock)(), self.startup_calculation_profile),
            )),
            StartupPolicy::BlankWorkspace => Ok((None, blank_workspace_session())),
            StartupPolicy::OpenWorkspace(id) => Self::saved_startup_session(catalog, id),
        }
    }

    pub(super) fn saved_startup_session(
        catalog: &Catalog,
        id: ResourceId,
    ) -> AppResult<(
        Option<ResourceEnvelope<WorkspaceDocument>>,
        WorkspaceSession,
    )> {
        let workspace = catalog.workspace(id).cloned().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Initialization,
                format!("Requested startup WorkspaceDocument {id} was not found"),
            )
        })?;
        let session = WorkspaceSession::from_saved(&workspace);
        Ok((Some(workspace), session))
    }

    pub(super) async fn hydrate_pinned_revisions(&self, catalog: &mut Catalog) -> AppResult<()> {
        let pinned = catalog.pinned_references();
        for (id, revision) in pinned {
            if catalog.history.contains_key(&(id, revision)) {
                continue;
            }
            let state = self
                .repository
                .get_revision(id, revision)
                .await
                .map_err(|error| {
                    initialization_error(
                        format!("Could not load pinned resource {id} revision {revision}"),
                        &error,
                    )
                })?;
            let Some(ResourceState::Present(resource)) = state else {
                return Err(AppError::new(
                    AppErrorKind::Initialization,
                    format!("Pinned resource {id} revision {revision} was not available"),
                ));
            };
            catalog.history.insert((id, revision), resource);
        }
        Ok(())
    }

    pub(super) async fn select_repository_resource(&self, id: ResourceId) -> AppResult<()> {
        let history = self.repository.list_revisions(id).await.map_err(|error| {
            AppError::new(
                AppErrorKind::Unavailable,
                format!("Could not load resource {id} revision history: {error}"),
            )
        })?;
        let Some(head) = history.last().cloned() else {
            return Err(AppError::new(
                AppErrorKind::NotFound,
                format!("Resource {id} was not found"),
            ));
        };
        if history.iter().any(|state| state.id() != id) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                format!("Repository returned mismatched identities for resource {id}"),
            ));
        }

        let mut state = self.state.borrow_mut();
        for revision in &history {
            if let ResourceState::Present(resource) = revision {
                state
                    .catalog
                    .history
                    .insert((resource.id(), resource.revision()), resource.clone());
            }
        }
        state.catalog.insert_head(head);
        state.repository_selection = Some(super::RepositorySelection {
            resource_id: id,
            history,
        });
        state.delete_confirmation = None;
        state.notice = None;
        state.advance()
    }
}
