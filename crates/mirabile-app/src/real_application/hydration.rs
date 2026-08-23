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
        let resources =
            self.repository.list(None).await.map_err(|error| {
                initialization_error("Could not load canonical resources", &error)
            })?;
        let mut catalog = Catalog::default();
        let mut latest_timestamp = 1;
        for resource in resources {
            latest_timestamp = latest_timestamp.max(resource_modified_at(&resource));
            catalog.insert_current(resource);
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
            StartupPolicy::OpenWorkspaces(ids) => ids.first().copied().map_or_else(
                || Ok((None, blank_workspace_session())),
                |id| Self::saved_startup_session(catalog, id),
            ),
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
}
