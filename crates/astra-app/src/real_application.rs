use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
};

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use astra_core::{
    AnalysisProfile, AspectSet, CalculationSpec, CanonicalResource, ChartDefinition, ChartRecord,
    ChartSource, Command, ConfigurationStack, EffectiveConfiguration, InstanceId, PointSet,
    ResolutionLayer, Resolved, ResourceBinding, ResourceEnvelope, ResourceId, Revision, Theme,
    Timestamp, ViewDocument, ViewInstance, ViewInstanceId, WheelTemplate, Workspace,
    WorkspaceChart, resolve_binding,
};
use astra_engine::{
    AspectAnalyzer, CalculationEngine, ComputationCache, DeterministicEphemeris, EphemerisProvider,
    Scene, layout_wheel, render_key,
};
#[cfg(target_arch = "wasm32")]
use astra_store::ResourceTombstone;
use astra_store::{MemoryRepository, RepositoryError, ResourceRepository, ResourceState};
use async_trait::async_trait;
use futures::channel::oneshot;

use crate::{
    ActiveChartInspector, AppAction, AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind,
    AppReadModel, AppResult, Application, ApplicationStatus, AspectDraftValue,
    AspectSetDraftMutation, AspectSetDraftReadModel, AspectSetSummary, Availability,
    BindingSourceSummary, ChartPersistence, ChartSlotAssignment, CommandCapability, DraftState,
    InspectorReadModel, LibraryChartSummary, LibraryReadModel, OpenChartSummary, ProjectionVersion,
    ResourceBindingSummary, ResourceEditorReadModel, ViewComputationState, ViewReadModel,
    ViewSummary, WorkspaceReadModel, bootstrap_ids, bootstrap_resources,
    workspace_commands::apply_workspace_command,
};

pub const DEFAULT_INDEXED_DB_NAME: &str = "astra";

pub struct RealApplication<R, P = DeterministicEphemeris> {
    repository: R,
    engine: CalculationEngine<P>,
    state: RefCell<RealState>,
}

impl<R> RealApplication<R, DeterministicEphemeris>
where
    R: ResourceRepository + Clone,
{
    pub fn with_repository(repository: R) -> Self {
        Self::with_provider(repository, DeterministicEphemeris)
    }
}

impl<R, P> RealApplication<R, P>
where
    R: ResourceRepository + Clone,
    P: EphemerisProvider,
{
    pub fn with_provider(repository: R, provider: P) -> Self {
        Self {
            repository,
            engine: CalculationEngine::new(provider, "astra-engine-v1", "deterministic-tz-v1"),
            state: RefCell::new(RealState::default()),
        }
    }
}

impl RealApplication<MemoryRepository, DeterministicEphemeris> {
    pub fn in_memory() -> Self {
        Self::with_repository(MemoryRepository::default())
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub struct IndexedDbRepositorySource {
    database_name: String,
    repository: Rc<RefCell<Option<astra_store::IndexedDbRepository>>>,
}

#[cfg(target_arch = "wasm32")]
impl IndexedDbRepositorySource {
    pub fn new(database_name: impl Into<String>) -> Self {
        Self {
            database_name: database_name.into(),
            repository: Rc::new(RefCell::new(None)),
        }
    }

    async fn acquire(&self) -> Result<astra_store::IndexedDbRepository, RepositoryError> {
        if let Some(repository) = self.repository.borrow().clone() {
            return Ok(repository);
        }
        let opened = astra_store::IndexedDbRepository::open(&self.database_name).await?;
        self.repository.replace(Some(opened.clone()));
        Ok(opened)
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
impl ResourceRepository for IndexedDbRepositorySource {
    async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError> {
        self.acquire().await?.create(resource).await
    }

    async fn save(
        &self,
        expected_revision: Revision,
        resource: CanonicalResource,
    ) -> Result<(), RepositoryError> {
        self.acquire()
            .await?
            .save(expected_revision, resource)
            .await
    }

    async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError> {
        self.acquire().await?.get(id).await
    }

    async fn get_head(&self, id: ResourceId) -> Result<Option<ResourceState>, RepositoryError> {
        self.acquire().await?.get_head(id).await
    }

    async fn get_revision(
        &self,
        id: ResourceId,
        revision: Revision,
    ) -> Result<Option<ResourceState>, RepositoryError> {
        self.acquire().await?.get_revision(id, revision).await
    }

    async fn list(
        &self,
        kind: Option<astra_core::ResourceKind>,
    ) -> Result<Vec<CanonicalResource>, RepositoryError> {
        self.acquire().await?.list(kind).await
    }

    async fn delete(
        &self,
        id: ResourceId,
        expected_revision: Revision,
        deleted_at: Timestamp,
    ) -> Result<ResourceTombstone, RepositoryError> {
        self.acquire()
            .await?
            .delete(id, expected_revision, deleted_at)
            .await
    }
}

#[cfg(target_arch = "wasm32")]
impl RealApplication<IndexedDbRepositorySource, DeterministicEphemeris> {
    pub fn indexed_db(database_name: impl Into<String>) -> Self {
        Self::with_repository(IndexedDbRepositorySource::new(database_name))
    }

    pub fn browser_default() -> Self {
        Self::indexed_db(DEFAULT_INDEXED_DB_NAME)
    }
}

#[async_trait(?Send)]
impl<R, P> Application for RealApplication<R, P>
where
    R: ResourceRepository + Clone,
    P: EphemerisProvider,
{
    async fn initialize(&self) -> AppResult<AppReadModel> {
        if matches!(self.state.borrow().status, ApplicationStatus::Ready) {
            return self.read_model();
        }

        match self.hydrate().await {
            Ok(hydrated) => {
                let mut state = self.state.borrow_mut();
                state.catalog = hydrated.catalog;
                state.workspace = Some(hydrated.workspace);
                state.next_timestamp = hydrated.next_timestamp;
                state.status = ApplicationStatus::Ready;
                state.editor = None;
                state.pending.clear();
                state.notice = Some(info(
                    "Canonical library and workspace hydrated; calculating the active view",
                ));
                state.ensure_view_runtimes();
                state.queue_active_view_refresh();
                state.advance()?;
            }
            Err(error) => {
                let mut state = self.state.borrow_mut();
                state.status = ApplicationStatus::Error(error);
                state.pending.clear();
                state.notice = None;
                state.advance()?;
            }
        }
        self.read_model()
    }

    async fn dispatch(&self, intent: AppIntent) -> AppResult<AppReadModel> {
        if !matches!(self.state.borrow().status, ApplicationStatus::Ready) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The application must be ready before it can accept intents",
            ));
        }
        if self
            .state
            .borrow()
            .pending
            .iter()
            .any(|pending| matches!(pending, PendingWork::SaveAspectSet { .. }))
        {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Wait for the pending Aspect Set save to finish",
            ));
        }

        match intent {
            AppIntent::OpenChart { .. }
            | AppIntent::CloseChart { .. }
            | AppIntent::ActivateChart { .. }
            | AppIntent::SetChartSelection { .. }
            | AppIntent::SetActiveView { .. }
            | AppIntent::AssignChartSlot { .. }
            | AppIntent::SetWorkspaceAspectSet { .. } => {
                self.dispatch_workspace_intent(intent).await?;
            }
            AppIntent::BeginAspectSetEdit { resource_id } => {
                self.begin_aspect_set_edit(resource_id)?;
            }
            AppIntent::UpdateAspectSetDraft(mutation) => {
                self.update_aspect_set_draft(mutation)?;
            }
            AppIntent::SaveDraft => self.begin_save_draft()?,
            AppIntent::CancelDraft => self.cancel_draft()?,
            AppIntent::RefreshActiveView => self.refresh_active_view()?,
        }
        self.read_model()
    }

    async fn snapshot(&self) -> AppResult<AppReadModel> {
        self.read_model()
    }

    async fn wait_for_update(&self, after: ProjectionVersion) -> AppResult<AppReadModel> {
        loop {
            let receiver = {
                let mut state = self.state.borrow_mut();
                if state.version > after {
                    return state.read_model();
                }
                if state.version < after {
                    return Err(AppError::new(
                        AppErrorKind::InvalidIntent,
                        format!(
                            "Cannot wait after future projection {after}; current projection is {}",
                            state.version
                        ),
                    ));
                }
                if state.pending.is_empty() {
                    let (sender, receiver) = oneshot::channel();
                    state.waiters.push(sender);
                    Some(receiver)
                } else {
                    None
                }
            };

            if let Some(receiver) = receiver {
                receiver.await.map_err(|_| {
                    AppError::new(
                        AppErrorKind::Unavailable,
                        "Application update notification ended before state advanced",
                    )
                })?;
            } else {
                self.complete_next_pending().await?;
            }
        }
    }
}

impl<R, P> RealApplication<R, P>
where
    R: ResourceRepository + Clone,
    P: EphemerisProvider,
{
    async fn hydrate(&self) -> AppResult<HydratedState> {
        self.ensure_bootstrap().await?;
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

        let workspace_id = bootstrap_ids().workspace;
        let workspace = catalog.workspace(workspace_id).cloned().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Initialization,
                format!("Bootstrap workspace {workspace_id} was not available after hydration"),
            )
        })?;
        Ok(HydratedState {
            catalog,
            workspace,
            next_timestamp: latest_timestamp.saturating_add(1),
        })
    }

    async fn ensure_bootstrap(&self) -> AppResult<()> {
        for resource in bootstrap_resources() {
            let id = resource.id();
            match self.repository.get_head(id).await.map_err(|error| {
                initialization_error(format!("Could not inspect bootstrap resource {id}"), &error)
            })? {
                None => match self.repository.create(resource.clone()).await {
                    Ok(()) | Err(RepositoryError::AlreadyExists(_)) => {}
                    Err(error) => {
                        return Err(initialization_error(
                            format!("Could not create bootstrap resource {id}"),
                            &error,
                        ));
                    }
                },
                Some(ResourceState::Present(existing)) if existing.kind() == resource.kind() => {}
                Some(ResourceState::Present(existing)) => {
                    return Err(AppError::new(
                        AppErrorKind::Initialization,
                        format!(
                            "Bootstrap identity {id} contains {:?}, expected {:?}",
                            existing.kind(),
                            resource.kind()
                        ),
                    ));
                }
                Some(ResourceState::Deleted(_)) => {
                    return Err(AppError::new(
                        AppErrorKind::Initialization,
                        format!("Bootstrap resource {id} was deleted and cannot be recreated"),
                    ));
                }
            }
        }
        Ok(())
    }

    async fn hydrate_pinned_revisions(&self, catalog: &mut Catalog) -> AppResult<()> {
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

    async fn dispatch_workspace_intent(&self, intent: AppIntent) -> AppResult<()> {
        let (expected_revision, next, refresh, clear_editor, notice) = {
            let state = self.state.borrow();
            let envelope = state.workspace.as_ref().ok_or_else(|| {
                AppError::new(AppErrorKind::Unavailable, "No hydrated workspace is active")
            })?;
            let workspace_id = envelope.id;
            let mut workspace = envelope.payload.clone();
            let (command, refresh, clear_editor, notice) =
                state.command_for_intent(workspace_id, &workspace, &intent)?;
            let view_documents = state.resolve_view_documents(&workspace)?;
            apply_workspace_command(workspace_id, &mut workspace, &command, &view_documents)
                .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
            let next = envelope
                .next_with_payload(workspace, Timestamp::from_unix_millis(state.next_timestamp))
                .map_err(|error| {
                    AppError::new(
                        AppErrorKind::InvalidIntent,
                        format!("Workspace mutation was invalid: {error}"),
                    )
                })?;
            (envelope.revision, next, refresh, clear_editor, notice)
        };

        self.repository
            .save(
                expected_revision,
                CanonicalResource::Workspace(next.clone()),
            )
            .await
            .map_err(|error| repository_app_error("Could not persist the workspace", &error))?;

        let mut state = self.state.borrow_mut();
        state.next_timestamp = state.next_timestamp.saturating_add(1);
        state
            .catalog
            .insert_current(CanonicalResource::Workspace(next.clone()));
        state.workspace = Some(next);
        if clear_editor {
            state.editor = None;
        }
        state.ensure_view_runtimes();
        if refresh {
            state.queue_active_view_refresh();
        }
        state.notice = Some(info(notice));
        state.advance()
    }

    fn begin_aspect_set_edit(&self, resource_id: ResourceId) -> AppResult<()> {
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

    fn update_aspect_set_draft(&self, mutation: AspectSetDraftMutation) -> AppResult<()> {
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
        state.queue_active_view_refresh();
        state.notice = Some(info(
            "Draft preview accepted; analysis is refreshing with the last good Scene retained",
        ));
        state.advance()
    }

    fn begin_save_draft(&self) -> AppResult<()> {
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

    fn cancel_draft(&self) -> AppResult<()> {
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
        state.queue_active_view_refresh();
        state.notice = Some(info(
            "Draft canceled; canonical Aspect Set semantics restored without a repository write",
        ));
        state.advance()
    }

    fn refresh_active_view(&self) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let Some(view_id) = state
            .workspace()
            .and_then(|workspace| workspace.active_view)
        else {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "There is no active view to refresh",
            ));
        };
        let runtime = state.views.get(&view_id).ok_or_else(|| {
            AppError::new(
                AppErrorKind::NotFound,
                format!("Active view {view_id} was not found"),
            )
        })?;
        if matches!(
            runtime.computation,
            ViewComputationState::Loading | ViewComputationState::Refreshing
        ) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The active view is already computing",
            ));
        }
        state.queue_active_view_refresh();
        state.notice = Some(info("Active view refresh requested"));
        state.advance()
    }

    async fn complete_next_pending(&self) -> AppResult<()> {
        let pending = self.state.borrow_mut().pending.pop_front();
        let Some(pending) = pending else {
            return Ok(());
        };
        match pending {
            PendingWork::ComputeView { view_id } => self.complete_view(view_id),
            PendingWork::SaveAspectSet {
                expected_revision,
                next,
            } => self.complete_aspect_set_save(expected_revision, next).await,
        }
    }

    fn complete_view(&self, view_id: ViewInstanceId) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let result = self.compute_scene(&mut state, view_id);
        let runtime = state.views.entry(view_id).or_default();
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

    async fn complete_aspect_set_save(
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

    #[allow(clippy::too_many_lines)]
    fn compute_scene(&self, state: &mut RealState, view_id: ViewInstanceId) -> AppResult<Scene> {
        let workspace = state
            .workspace
            .as_ref()
            .ok_or_else(|| {
                AppError::new(AppErrorKind::ViewComputation, "No workspace is hydrated")
            })?
            .payload
            .clone();
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
        let document =
            resolve_typed_binding(&view.document, &state.catalog).map_err(view_resolution_error)?;
        let chart_instance = document
            .value
            .chart_slots
            .iter()
            .filter(|slot| slot.required)
            .find_map(|slot| view.charts.get(&slot.id).copied())
            .or_else(|| view.charts.values().next().copied())
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::ViewComputation,
                    "The active view has no assigned chart",
                )
            })?;
        let workspace_chart = workspace
            .chart_instances
            .iter()
            .find(|chart| chart.instance_id() == chart_instance)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::ViewComputation,
                    format!("Assigned chart {chart_instance} is not open"),
                )
            })?;
        let definition_id = match workspace_chart {
            WorkspaceChart::Saved { definition, .. } => *definition,
            WorkspaceChart::Ephemeral { .. } => {
                return Err(AppError::new(
                    AppErrorKind::ViewComputation,
                    "Ephemeral chart calculation is not implemented in this integration slice",
                ));
            }
        };
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
        let effective = state.effective_configuration(&definition, &view)?;
        let mut effective_definition = definition;
        effective_definition.payload.calculation = effective.calculation.value;

        let calc_key = self
            .engine
            .calc_key(&effective_definition, &record)
            .map_err(view_computation_error)?;
        let snapshot = if let Some(calculation) = state.cache.calculation(&calc_key).cloned() {
            self.engine
                .snapshot_from_cached(&effective_definition, &record, calculation)
                .map_err(view_computation_error)?
        } else {
            let snapshot = self
                .engine
                .calculate(&effective_definition, &record)
                .map_err(view_computation_error)?;
            state.cache.insert_snapshot(snapshot.clone());
            snapshot
        };
        let analysis = AspectAnalyzer::analyze(
            &snapshot,
            &effective.aspected_points.value,
            &effective.aspect_set.value,
            &effective.analysis.value,
        )
        .map_err(view_computation_error)?;
        state.cache.insert_analysis(analysis.clone());
        let layout = layout_wheel(
            &snapshot,
            &analysis,
            &effective.displayed_points.value,
            &effective.wheel.value,
        )
        .map_err(view_computation_error)?;
        render_key(&layout, &effective.theme.value).map_err(view_computation_error)?;
        Ok(Scene::from_wheel(&layout))
    }

    fn read_model(&self) -> AppResult<AppReadModel> {
        self.state.borrow().read_model()
    }
}

struct RealState {
    version: ProjectionVersion,
    status: ApplicationStatus,
    catalog: Catalog,
    workspace: Option<ResourceEnvelope<Workspace>>,
    views: BTreeMap<ViewInstanceId, ViewRuntime>,
    editor: Option<AspectSetEditor>,
    cache: ComputationCache,
    pending: VecDeque<PendingWork>,
    waiters: Vec<oneshot::Sender<()>>,
    notice: Option<AppNotice>,
    next_timestamp: i64,
}

impl Default for RealState {
    fn default() -> Self {
        Self {
            version: ProjectionVersion::INITIAL,
            status: ApplicationStatus::Initializing,
            catalog: Catalog::default(),
            workspace: None,
            views: BTreeMap::new(),
            editor: None,
            cache: ComputationCache::default(),
            pending: VecDeque::new(),
            waiters: Vec::new(),
            notice: None,
            next_timestamp: 1,
        }
    }
}

impl RealState {
    fn workspace(&self) -> Option<&Workspace> {
        self.workspace.as_ref().map(|workspace| &workspace.payload)
    }

    fn advance(&mut self) -> AppResult<()> {
        self.version = self.version.checked_next().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Unavailable,
                "Application projection version overflowed",
            )
        })?;
        for waiter in self.waiters.drain(..) {
            let _ = waiter.send(());
        }
        Ok(())
    }

    fn ensure_view_runtimes(&mut self) {
        let view_ids = self
            .workspace()
            .map(|workspace| {
                workspace
                    .views
                    .iter()
                    .map(|view| view.id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.views.retain(|id, _| view_ids.contains(id));
        for id in view_ids {
            self.views.entry(id).or_default();
        }
    }

    fn queue_active_view_refresh(&mut self) {
        let Some(view_id) = self.workspace().and_then(|workspace| workspace.active_view) else {
            return;
        };
        let runtime = self.views.entry(view_id).or_default();
        runtime.computation = if runtime.scene.is_some() {
            ViewComputationState::Refreshing
        } else {
            ViewComputationState::Loading
        };
        self.pending.retain(|pending| {
            !matches!(pending, PendingWork::ComputeView { view_id: pending_id } if *pending_id == view_id)
        });
        self.pending
            .push_front(PendingWork::ComputeView { view_id });
    }

    fn effective_configuration(
        &self,
        definition: &ResourceEnvelope<ChartDefinition>,
        view: &ViewInstance,
    ) -> AppResult<EffectiveConfiguration> {
        let workspace = self.workspace().ok_or_else(|| {
            AppError::new(AppErrorKind::ViewComputation, "No workspace is hydrated")
        })?;
        let calculation = ConfigurationStack {
            built_in: CalculationSpec::default(),
            user_default: None,
            workspace: None,
            chart_definition: Some(definition.payload.calculation.clone()),
            view_override: None,
            editor_preview: None,
        }
        .resolve();
        let mut displayed_points =
            resolve_typed_binding(&workspace.profile.displayed_points, &self.catalog)
                .map_err(view_resolution_error)?;
        if !view.overrides.hidden_points.is_empty() {
            displayed_points
                .value
                .points
                .retain(|selector| match selector {
                    astra_core::PointSelector::Point(point) => {
                        !view.overrides.hidden_points.contains(point)
                    }
                    astra_core::PointSelector::Category(_) => true,
                });
            displayed_points.layer = ResolutionLayer::ViewOverride;
            displayed_points.resource = None;
            displayed_points.revision = None;
        }
        let aspected_points =
            resolve_typed_binding(&workspace.profile.aspected_points, &self.catalog)
                .map_err(view_resolution_error)?;
        let mut aspect_set = resolve_typed_binding(&workspace.profile.aspects, &self.catalog)
            .map_err(view_resolution_error)?;
        if let Some(editor) = &self.editor
            && workspace.profile.aspects.id() == Some(editor.base.id)
        {
            aspect_set = Resolved {
                value: editor.draft.clone(),
                layer: ResolutionLayer::EditorPreview,
                resource: Some(editor.base.id),
                revision: Some(editor.state.base_revision()),
            };
        }
        let analysis = resolve_typed_binding(&workspace.profile.analysis, &self.catalog)
            .map_err(view_resolution_error)?;
        let wheel = resolve_typed_binding(&workspace.profile.wheel, &self.catalog)
            .map_err(view_resolution_error)?;
        let theme = resolve_typed_binding(&workspace.profile.theme, &self.catalog)
            .map_err(view_resolution_error)?;
        Ok(EffectiveConfiguration {
            calculation,
            displayed_points,
            aspected_points,
            aspect_set,
            analysis,
            wheel,
            theme,
        })
    }

    fn resolve_view_documents(
        &self,
        workspace: &Workspace,
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
    fn command_for_intent(
        &self,
        workspace_id: ResourceId,
        workspace: &Workspace,
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
                    "Chart opened and activated; selection was preserved",
                ))
            }
            AppIntent::CloseChart { instance_id } => Ok((
                Command::CloseChart {
                    workspace: workspace_id,
                    instance_id: *instance_id,
                },
                true,
                false,
                "Chart closed; selection and chart slots were repaired by workspace policy",
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
                    "Chart slot assignment persisted; the view is refreshing",
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
                    "Workspace Aspect Set binding changed; analysis is refreshing",
                ))
            }
            AppIntent::BeginAspectSetEdit { .. }
            | AppIntent::UpdateAspectSetDraft(_)
            | AppIntent::SaveDraft
            | AppIntent::CancelDraft
            | AppIntent::RefreshActiveView => Err(AppError::new(
                AppErrorKind::InvalidIntent,
                "The intent is not a workspace persistence command",
            )),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn read_model(&self) -> AppResult<AppReadModel> {
        if !matches!(self.status, ApplicationStatus::Ready) {
            let mut model = AppReadModel::initializing();
            model.version = self.version;
            model.status = self.status.clone();
            model.notice.clone_from(&self.notice);
            return Ok(model);
        }
        let workspace = self.workspace().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Unavailable,
                "Ready application has no workspace",
            )
        })?;
        let library_charts = self.catalog.library_charts()?;
        let open_charts = workspace
            .chart_instances
            .iter()
            .map(|chart| self.catalog.open_chart_summary(chart))
            .collect::<AppResult<Vec<_>>>()?;
        let active_chart = workspace.active_chart.and_then(|active_id| {
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
        let active_view = workspace
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
        if let Some(view) = workspace
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
            library: LibraryReadModel {
                charts: library_charts,
                aspect_sets: self.catalog.aspect_set_summaries()?,
            },
            workspace: WorkspaceReadModel {
                charts: open_charts,
                active_chart: workspace.active_chart,
                selected_charts: workspace.selected_charts.clone(),
                views: view_summaries,
                active_view: workspace.active_view,
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

    fn view_read_model(&self, view_id: ViewInstanceId) -> AppResult<ViewReadModel> {
        let workspace = self.workspace().expect("read model checked workspace");
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
        let document = resolve_typed_binding(&view.document, &self.catalog)
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
                    chart: view.charts.get(&slot.id).copied(),
                    slot: slot.id,
                    label: slot.label,
                    required: slot.required,
                })
                .collect(),
        })
    }

    fn capabilities(&self) -> Vec<CommandCapability> {
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
            .workspace()
            .and_then(|workspace| workspace.active_view)
            .and_then(|id| self.views.get(&id))
            .map_or_else(
                || disabled("No active view"),
                |runtime| match runtime.computation {
                    ViewComputationState::Loading | ViewComputationState::Refreshing => {
                        disabled("The active view is already computing")
                    }
                    ViewComputationState::Fresh | ViewComputationState::Failed(_) => {
                        Availability::Enabled
                    }
                },
            );
        vec![
            capability(AppAction::BeginAspectSetEdit, begin),
            capability(AppAction::SaveDraft, save),
            capability(AppAction::CancelDraft, cancel),
            capability(AppAction::RefreshView, refresh),
        ]
    }
}

struct HydratedState {
    catalog: Catalog,
    workspace: ResourceEnvelope<Workspace>,
    next_timestamp: i64,
}

#[derive(Clone, Default)]
struct Catalog {
    current: BTreeMap<ResourceId, CanonicalResource>,
    history: BTreeMap<(ResourceId, Revision), CanonicalResource>,
}

impl Catalog {
    fn insert_current(&mut self, resource: CanonicalResource) {
        self.history
            .insert((resource.id(), resource.revision()), resource.clone());
        self.current.insert(resource.id(), resource);
    }

    fn chart_record(&self, id: ResourceId) -> Option<&ResourceEnvelope<ChartRecord>> {
        match self.current.get(&id) {
            Some(CanonicalResource::ChartRecord(value)) => Some(value),
            _ => None,
        }
    }

    fn chart_definition(&self, id: ResourceId) -> Option<&ResourceEnvelope<ChartDefinition>> {
        match self.current.get(&id) {
            Some(CanonicalResource::ChartDefinition(value)) => Some(value),
            _ => None,
        }
    }

    fn aspect_set(&self, id: ResourceId) -> Option<&ResourceEnvelope<AspectSet>> {
        match self.current.get(&id) {
            Some(CanonicalResource::AspectSet(value)) => Some(value),
            _ => None,
        }
    }

    fn workspace(&self, id: ResourceId) -> Option<&ResourceEnvelope<Workspace>> {
        match self.current.get(&id) {
            Some(CanonicalResource::Workspace(value)) => Some(value),
            _ => None,
        }
    }

    fn pinned_references(&self) -> Vec<(ResourceId, Revision)> {
        let mut references = Vec::new();
        for resource in self.current.values() {
            let CanonicalResource::Workspace(workspace) = resource else {
                continue;
            };
            push_pin(&workspace.payload.profile.displayed_points, &mut references);
            push_pin(&workspace.payload.profile.aspected_points, &mut references);
            push_pin(&workspace.payload.profile.transit_points, &mut references);
            push_pin(&workspace.payload.profile.aspects, &mut references);
            push_pin(&workspace.payload.profile.analysis, &mut references);
            push_pin(&workspace.payload.profile.theme, &mut references);
            push_pin(&workspace.payload.profile.wheel, &mut references);
            for view in &workspace.payload.views {
                push_pin(&view.document, &mut references);
            }
        }
        references.sort_unstable();
        references.dedup();
        references
    }

    fn library_charts(&self) -> AppResult<Vec<LibraryChartSummary>> {
        self.current
            .values()
            .filter_map(|resource| match resource {
                CanonicalResource::ChartDefinition(definition) => Some(definition),
                _ => None,
            })
            .map(|definition| {
                let subtitle = match definition.payload.source {
                    ChartSource::Radix { record } => self
                        .chart_record(record)
                        .map_or_else(|| "Missing source record".into(), chart_subtitle),
                    ChartSource::Derived { .. } => "Derived chart".into(),
                };
                Ok(LibraryChartSummary {
                    definition_id: definition.id,
                    title: definition.title.clone(),
                    subtitle,
                })
            })
            .collect()
    }

    fn open_chart_summary(&self, chart: &WorkspaceChart) -> AppResult<OpenChartSummary> {
        match chart {
            WorkspaceChart::Saved {
                instance_id,
                definition,
            } => {
                let definition_envelope = self
                    .chart_definition(*definition)
                    .ok_or_else(|| not_found("ChartDefinition", *definition))?;
                let subtitle = match definition_envelope.payload.source {
                    ChartSource::Radix { record } => self
                        .chart_record(record)
                        .map_or_else(|| "Missing source record".into(), chart_subtitle),
                    ChartSource::Derived { .. } => "Derived chart".into(),
                };
                Ok(OpenChartSummary {
                    instance_id: *instance_id,
                    title: definition_envelope.title.clone(),
                    subtitle,
                    persistence: ChartPersistence::Saved {
                        definition_id: *definition,
                    },
                })
            }
            WorkspaceChart::Ephemeral {
                instance_id,
                definition,
            } => Ok(OpenChartSummary {
                instance_id: *instance_id,
                title: "Unsaved chart".into(),
                subtitle: match definition.source {
                    ChartSource::Radix { .. } => "Ephemeral radix definition".into(),
                    ChartSource::Derived { .. } => "Ephemeral derived definition".into(),
                },
                persistence: ChartPersistence::Ephemeral,
            }),
        }
    }

    fn aspect_set_summaries(&self) -> AppResult<Vec<AspectSetSummary>> {
        self.current
            .values()
            .filter_map(|resource| match resource {
                CanonicalResource::AspectSet(envelope) => Some(envelope),
                _ => None,
            })
            .map(|envelope| {
                Ok(AspectSetSummary {
                    resource_id: envelope.id,
                    title: envelope.title.clone(),
                    revision: envelope.revision,
                    conjunction_orb: conjunction(&envelope.payload)?.orbs.maximum,
                })
            })
            .collect()
    }
}

#[derive(Clone)]
struct ViewRuntime {
    scene: Option<Scene>,
    computation: ViewComputationState,
}

impl Default for ViewRuntime {
    fn default() -> Self {
        Self {
            scene: None,
            computation: ViewComputationState::Loading,
        }
    }
}

struct AspectSetEditor {
    base: ResourceEnvelope<AspectSet>,
    draft: AspectSet,
    state: DraftState,
}

enum PendingWork {
    ComputeView {
        view_id: ViewInstanceId,
    },
    SaveAspectSet {
        expected_revision: Revision,
        next: ResourceEnvelope<AspectSet>,
    },
}

trait BoundPayload: Clone + Sized {
    fn envelope(resource: &CanonicalResource) -> Option<&ResourceEnvelope<Self>>;
}

macro_rules! bound_payload {
    ($payload:ty, $variant:ident) => {
        impl BoundPayload for $payload {
            fn envelope(resource: &CanonicalResource) -> Option<&ResourceEnvelope<Self>> {
                match resource {
                    CanonicalResource::$variant(envelope) => Some(envelope),
                    _ => None,
                }
            }
        }
    };
}

bound_payload!(PointSet, PointSet);
bound_payload!(AspectSet, AspectSet);
bound_payload!(AnalysisProfile, AnalysisProfile);
bound_payload!(WheelTemplate, WheelTemplate);
bound_payload!(Theme, Theme);
bound_payload!(ViewDocument, ViewDocument);

fn resolve_typed_binding<T: BoundPayload>(
    binding: &ResourceBinding<T>,
    catalog: &Catalog,
) -> Result<Resolved<T>, astra_core::BindingResolutionError> {
    resolve_binding(
        binding,
        |id| catalog.current.get(&id).and_then(T::envelope).cloned(),
        |id, revision| {
            catalog
                .history
                .get(&(id, revision))
                .and_then(T::envelope)
                .cloned()
        },
    )
}

fn binding_summary<T: BoundPayload>(
    label: &str,
    binding: &ResourceBinding<T>,
    catalog: &Catalog,
) -> AppResult<ResourceBindingSummary> {
    let resolved = resolve_typed_binding(binding, catalog)
        .map_err(|error| AppError::new(AppErrorKind::NotFound, error.to_string()))?;
    let source = match binding {
        ResourceBinding::Inline { .. } => BindingSourceSummary::Inline,
        ResourceBinding::Follow { id } => BindingSourceSummary::Follow {
            resource_id: *id,
            resource_title: resource_title(catalog, *id, resolved.revision)?,
            revision: resolved
                .revision
                .expect("follow resolution includes a revision"),
        },
        ResourceBinding::Pinned { id, revision } => BindingSourceSummary::Pinned {
            resource_id: *id,
            resource_title: resource_title(catalog, *id, Some(*revision))?,
            revision: *revision,
        },
    };
    Ok(ResourceBindingSummary {
        label: label.into(),
        source,
    })
}

fn resource_title(
    catalog: &Catalog,
    id: ResourceId,
    revision: Option<Revision>,
) -> AppResult<String> {
    catalog
        .current
        .get(&id)
        .filter(|resource| revision.is_none_or(|expected| resource.revision() == expected))
        .or_else(|| revision.and_then(|revision| catalog.history.get(&(id, revision))))
        .map(|resource| resource.title().to_owned())
        .ok_or_else(|| not_found("bound resource", id))
}

fn view_title(view: &ViewInstance, catalog: &Catalog) -> AppResult<String> {
    match &view.document {
        ResourceBinding::Inline { .. } => Ok("Single wheel".into()),
        ResourceBinding::Follow { id } => resource_title(catalog, *id, None),
        ResourceBinding::Pinned { id, revision } => resource_title(catalog, *id, Some(*revision)),
    }
}

fn aspect_editor_read_model(editor: &AspectSetEditor) -> AppResult<AspectSetDraftReadModel> {
    let conjunction = conjunction(&editor.draft)?;
    Ok(AspectSetDraftReadModel {
        resource_id: editor.base.id,
        title: editor.base.title.clone(),
        state: editor.state.clone(),
        conjunction: AspectDraftValue {
            aspect_id: conjunction.id.clone(),
            label: conjunction.name.clone(),
            enabled: conjunction.enabled,
            maximum_orb: conjunction.orbs.maximum,
        },
    })
}

fn conjunction(aspects: &AspectSet) -> AppResult<&astra_core::AspectDefinition> {
    aspects
        .aspects
        .iter()
        .find(|aspect| aspect.id.as_str() == "conjunction")
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::NotFound,
                "Aspect Set has no conjunction definition",
            )
        })
}

fn push_pin<T>(binding: &ResourceBinding<T>, output: &mut Vec<(ResourceId, Revision)>) {
    if let ResourceBinding::Pinned { id, revision } = binding {
        output.push((*id, *revision));
    }
}

fn chart_subtitle(record: &ResourceEnvelope<ChartRecord>) -> String {
    let date = record.payload.time.civil_datetime.date;
    let month = match date.month() {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "?",
    };
    format!(
        "{month} {}, {} · {}",
        date.day(),
        date.year(),
        record.payload.location.display_name
    )
}

fn resource_modified_at(resource: &CanonicalResource) -> i64 {
    match resource {
        CanonicalResource::ChartRecord(value) => value.modified_at.unix_millis(),
        CanonicalResource::ChartDefinition(value) => value.modified_at.unix_millis(),
        CanonicalResource::PointSet(value) => value.modified_at.unix_millis(),
        CanonicalResource::AspectSet(value) => value.modified_at.unix_millis(),
        CanonicalResource::AnalysisProfile(value) => value.modified_at.unix_millis(),
        CanonicalResource::WheelTemplate(value) => value.modified_at.unix_millis(),
        CanonicalResource::ViewDocument(value) => value.modified_at.unix_millis(),
        CanonicalResource::Theme(value) => value.modified_at.unix_millis(),
        CanonicalResource::QueryDefinition(value) => value.modified_at.unix_millis(),
        CanonicalResource::Workspace(value) => value.modified_at.unix_millis(),
    }
}

fn capability(action: AppAction, availability: Availability) -> CommandCapability {
    CommandCapability {
        action,
        availability,
    }
}

fn disabled(reason: &str) -> Availability {
    Availability::Disabled {
        reason: Some(reason.into()),
    }
}

fn info(message: impl Into<String>) -> AppNotice {
    AppNotice {
        kind: AppNoticeKind::Info,
        message: message.into(),
    }
}

fn success(message: impl Into<String>) -> AppNotice {
    AppNotice {
        kind: AppNoticeKind::Success,
        message: message.into(),
    }
}

fn restore_dirty_editor(state: &mut RealState, resource_id: ResourceId, base_revision: Revision) {
    if let Some(editor) = state
        .editor
        .as_mut()
        .filter(|editor| editor.base.id == resource_id)
    {
        editor.state = DraftState::Dirty { base_revision };
    }
}

fn conflict_refresh_warning(error: impl std::fmt::Display) -> AppNotice {
    AppNotice {
        kind: AppNoticeKind::Warning,
        message: format!(
            "Aspect Set save conflict was detected, but the remote revision could not be loaded; the local draft was retained: {error}"
        ),
    }
}

fn not_found(noun: &str, id: ResourceId) -> AppError {
    AppError::new(AppErrorKind::NotFound, format!("{noun} {id} was not found"))
}

fn not_found_for_view(noun: &str, id: ResourceId) -> AppError {
    AppError::new(
        AppErrorKind::ViewComputation,
        format!("{noun} {id} was not found"),
    )
}

fn initialization_error(context: impl AsRef<str>, error: &RepositoryError) -> AppError {
    AppError::new(
        AppErrorKind::Initialization,
        format!("{}: {error}", context.as_ref()),
    )
}

fn repository_app_error(context: &str, error: &RepositoryError) -> AppError {
    let kind = match error {
        RepositoryError::Conflict { .. } => AppErrorKind::Conflict,
        RepositoryError::NotFound(_) | RepositoryError::ResourceDeleted(_) => {
            AppErrorKind::NotFound
        }
        RepositoryError::AlreadyExists(_)
        | RepositoryError::InitialRevisionRequired { .. }
        | RepositoryError::NonSequentialRevision { .. }
        | RepositoryError::IdentityChanged { .. }
        | RepositoryError::KindChanged { .. }
        | RepositoryError::InvalidResource(_)
        | RepositoryError::UnsupportedSchemaVersion { .. }
        | RepositoryError::Serialization(_)
        | RepositoryError::Adapter(_) => AppErrorKind::Unavailable,
    };
    AppError::new(kind, format!("{context}: {error}"))
}

fn view_resolution_error(error: astra_core::BindingResolutionError) -> AppError {
    AppError::new(
        AppErrorKind::ViewComputation,
        format!("Effective configuration could not be resolved: {error}"),
    )
}

fn view_computation_error(error: impl std::fmt::Display) -> AppError {
    AppError::new(AppErrorKind::ViewComputation, error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use astra_core::{Angle, CanonicalResource, PointSelector, ResourceEnvelope, ResourceKind};
    use astra_engine::{EphemerisError, EphemerisOutput, EphemerisRequest, ProviderIdentity};
    use astra_store::ResourceTombstone;
    use futures::executor::block_on;

    use super::*;

    #[derive(Clone)]
    struct ControlledProvider {
        calls: Rc<Cell<u32>>,
        fail_next: Rc<Cell<bool>>,
    }

    impl ControlledProvider {
        fn new() -> Self {
            Self {
                calls: Rc::new(Cell::new(0)),
                fail_next: Rc::new(Cell::new(false)),
            }
        }
    }

    impl EphemerisProvider for ControlledProvider {
        fn identity(&self) -> ProviderIdentity {
            ProviderIdentity {
                name: "controlled-deterministic-provider".into(),
                version: "1".into(),
                data_version: Some("fixture-v1".into()),
            }
        }

        fn calculate(&self, request: &EphemerisRequest) -> Result<EphemerisOutput, EphemerisError> {
            self.calls.set(self.calls.get() + 1);
            if self.fail_next.replace(false) {
                Err(EphemerisError::NonFiniteInput)
            } else {
                DeterministicEphemeris.calculate(request)
            }
        }
    }

    #[derive(Clone, Copy)]
    enum InjectedSaveFailure {
        ConflictThenReadFailure,
        Adapter,
        None,
    }

    #[derive(Clone)]
    struct SaveFailureRepository {
        inner: MemoryRepository,
        save_failure: Rc<Cell<InjectedSaveFailure>>,
        fail_next_get: Rc<Cell<bool>>,
    }

    impl SaveFailureRepository {
        fn new(save_failure: InjectedSaveFailure) -> Self {
            Self {
                inner: MemoryRepository::default(),
                save_failure: Rc::new(Cell::new(save_failure)),
                fail_next_get: Rc::new(Cell::new(false)),
            }
        }
    }

    #[async_trait(?Send)]
    impl ResourceRepository for SaveFailureRepository {
        async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError> {
            self.inner.create(resource).await
        }

        async fn save(
            &self,
            expected_revision: Revision,
            resource: CanonicalResource,
        ) -> Result<(), RepositoryError> {
            if matches!(&resource, CanonicalResource::AspectSet(_)) {
                match self.save_failure.replace(InjectedSaveFailure::None) {
                    InjectedSaveFailure::ConflictThenReadFailure => {
                        self.fail_next_get.set(true);
                        return Err(RepositoryError::Conflict {
                            expected: expected_revision,
                            actual: expected_revision.next().expect("test revision can advance"),
                        });
                    }
                    InjectedSaveFailure::Adapter => {
                        return Err(RepositoryError::Adapter(
                            "injected AspectSet save failure".into(),
                        ));
                    }
                    InjectedSaveFailure::None => {}
                }
            }
            self.inner.save(expected_revision, resource).await
        }

        async fn get(&self, id: ResourceId) -> Result<Option<CanonicalResource>, RepositoryError> {
            if self.fail_next_get.replace(false) {
                return Err(RepositoryError::Adapter(
                    "injected remote-head read failure".into(),
                ));
            }
            self.inner.get(id).await
        }

        async fn get_head(&self, id: ResourceId) -> Result<Option<ResourceState>, RepositoryError> {
            self.inner.get_head(id).await
        }

        async fn get_revision(
            &self,
            id: ResourceId,
            revision: Revision,
        ) -> Result<Option<ResourceState>, RepositoryError> {
            self.inner.get_revision(id, revision).await
        }

        async fn list(
            &self,
            kind: Option<ResourceKind>,
        ) -> Result<Vec<CanonicalResource>, RepositoryError> {
            self.inner.list(kind).await
        }

        async fn delete(
            &self,
            id: ResourceId,
            expected_revision: Revision,
            deleted_at: Timestamp,
        ) -> Result<ResourceTombstone, RepositoryError> {
            self.inner.delete(id, expected_revision, deleted_at).await
        }
    }

    fn ready<R, P>(application: &RealApplication<R, P>) -> AppReadModel
    where
        R: ResourceRepository + Clone,
        P: EphemerisProvider,
    {
        let loading = block_on(application.initialize()).expect("initialization succeeds");
        assert_eq!(loading.status, ApplicationStatus::Ready);
        assert!(matches!(
            loading.active_view.as_ref().map(|view| &view.computation),
            Some(ViewComputationState::Loading)
        ));
        block_on(application.wait_for_update(loading.version)).expect("initial view settles")
    }

    fn angle(value: f64) -> Angle {
        Angle::from_degrees(value).expect("test angle is valid")
    }

    fn editor_state(model: &AppReadModel) -> &DraftState {
        &model
            .resource_editor
            .aspect_set
            .as_ref()
            .expect("draft is projected")
            .state
    }

    #[derive(Clone, Copy)]
    enum TestViewDocumentBinding {
        Inline,
        Follow,
        Pinned,
    }

    fn repository_with_view_document_binding(binding: TestViewDocumentBinding) -> MemoryRepository {
        let repository = MemoryRepository::default();
        let bootstrap = RealApplication::with_repository(repository.clone());
        ready(&bootstrap);
        drop(bootstrap);

        if matches!(binding, TestViewDocumentBinding::Inline) {
            return repository;
        }

        let workspace_id = bootstrap_ids().workspace;
        let CanonicalResource::Workspace(workspace) = block_on(repository.get(workspace_id))
            .expect("workspace read succeeds")
            .expect("workspace exists")
        else {
            panic!("bootstrap resource is a Workspace");
        };
        let ResourceBinding::Inline { value: document } = &workspace.payload.views[0].document
        else {
            panic!("bootstrap ViewDocument is inline");
        };
        let document_id = ResourceId::new();
        let first = ResourceEnvelope::with_id(
            document_id,
            "Close repair ViewDocument",
            document.clone(),
            Timestamp::from_unix_millis(30),
        );
        block_on(repository.create(CanonicalResource::ViewDocument(first.clone())))
            .expect("ViewDocument creation succeeds");

        let document_binding = match binding {
            TestViewDocumentBinding::Follow => ResourceBinding::Follow { id: document_id },
            TestViewDocumentBinding::Pinned => {
                let mut current_payload = first.payload.clone();
                for slot in &mut current_payload.chart_slots {
                    slot.required = !slot.required;
                }
                let current = first
                    .next_with_payload(current_payload, Timestamp::from_unix_millis(31))
                    .expect("current ViewDocument revision is valid");
                block_on(repository.save(first.revision, CanonicalResource::ViewDocument(current)))
                    .expect("ViewDocument head advances");
                ResourceBinding::Pinned {
                    id: document_id,
                    revision: first.revision,
                }
            }
            TestViewDocumentBinding::Inline => unreachable!(),
        };
        let mut payload = workspace.payload.clone();
        payload.views[0].document = document_binding;
        let next = workspace
            .next_with_payload(payload, Timestamp::from_unix_millis(32))
            .expect("Workspace binding revision is valid");
        block_on(repository.save(workspace.revision, CanonicalResource::Workspace(next)))
            .expect("Workspace binding persists");
        repository
    }

    fn assert_close_repair_uses_resolved_view_document(repository: MemoryRepository) {
        let application = RealApplication::with_repository(repository);
        let initial = ready(&application);
        let ids = bootstrap_ids();
        let view_id = initial.workspace.active_view.expect("active view");
        let view = initial.active_view.expect("active view projection");
        let required = view
            .slots
            .iter()
            .find(|slot| slot.required)
            .expect("required slot")
            .slot
            .clone();
        let optional = view
            .slots
            .iter()
            .find(|slot| !slot.required)
            .expect("optional slot")
            .slot
            .clone();
        assert_eq!(required.as_str(), "radix");
        assert_eq!(optional.as_str(), "comparison");

        let opened = block_on(application.dispatch(AppIntent::OpenChart {
            definition_id: ids.chart_definition_b,
        }))
        .expect("chart B opens");
        let neighbor = opened.workspace.active_chart.expect("chart B is active");
        block_on(application.dispatch(AppIntent::SetChartSelection {
            instance_id: ids.chart_instance_a,
            selected: true,
        }))
        .expect("chart A is selected");
        block_on(application.dispatch(AppIntent::ActivateChart {
            instance_id: ids.chart_instance_a,
        }))
        .expect("chart A is active before close");
        block_on(application.dispatch(AppIntent::AssignChartSlot {
            view_id,
            slot: optional.clone(),
            chart: Some(ids.chart_instance_a),
        }))
        .expect("optional slot receives chart A");

        let refreshing = block_on(application.dispatch(AppIntent::CloseChart {
            instance_id: ids.chart_instance_a,
        }))
        .expect("chart A closes");
        let closed = block_on(application.wait_for_update(refreshing.version))
            .expect("close refresh settles");
        assert!(
            closed
                .workspace
                .charts
                .iter()
                .all(|chart| chart.instance_id != ids.chart_instance_a)
        );
        assert!(
            !closed
                .workspace
                .selected_charts
                .contains(&ids.chart_instance_a)
        );
        assert_eq!(closed.workspace.active_chart, Some(neighbor));
        let view = closed.active_view.expect("active view remains");
        assert_eq!(
            view.slots
                .iter()
                .find(|slot| slot.slot == required)
                .expect("required slot remains")
                .chart,
            Some(neighbor)
        );
        assert_eq!(
            view.slots
                .iter()
                .find(|slot| slot.slot == optional)
                .expect("optional slot remains")
                .chart,
            None
        );
    }

    #[test]
    fn initialization_is_versioned_snapshot_is_immediate_and_bootstrap_is_idempotent() {
        let repository = MemoryRepository::default();
        let first = RealApplication::with_repository(repository.clone());
        let initial = block_on(first.snapshot()).expect("snapshot succeeds");
        assert_eq!(initial.version, ProjectionVersion::INITIAL);
        assert_eq!(initial.status, ApplicationStatus::Initializing);

        let loading = block_on(first.initialize()).expect("initialization succeeds");
        assert_eq!(loading.version, ProjectionVersion::new(1));
        assert_eq!(repository.current_count(), 7);
        let immediate = block_on(first.snapshot()).expect("snapshot succeeds");
        assert_eq!(immediate, loading);
        assert!(matches!(
            immediate.active_view.map(|view| view.computation),
            Some(ViewComputationState::Loading)
        ));
        let fresh = block_on(first.wait_for_update(loading.version)).expect("view settles");
        assert_eq!(fresh.version, ProjectionVersion::new(2));
        assert!(matches!(
            fresh.active_view.map(|view| view.computation),
            Some(ViewComputationState::Fresh)
        ));

        let second = RealApplication::with_repository(repository.clone());
        ready(&second);
        assert_eq!(repository.current_count(), 7);
        assert_eq!(repository.revision_count(), 7);
    }

    #[test]
    fn close_chart_repairs_required_and_optional_slots_for_inline_view_document() {
        assert_close_repair_uses_resolved_view_document(repository_with_view_document_binding(
            TestViewDocumentBinding::Inline,
        ));
    }

    #[test]
    fn close_chart_repairs_required_and_optional_slots_for_follow_view_document() {
        assert_close_repair_uses_resolved_view_document(repository_with_view_document_binding(
            TestViewDocumentBinding::Follow,
        ));
    }

    #[test]
    fn close_chart_uses_exact_pinned_view_document_revision_for_slot_repair() {
        assert_close_repair_uses_resolved_view_document(repository_with_view_document_binding(
            TestViewDocumentBinding::Pinned,
        ));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn every_workspace_intent_persists_documented_semantics() {
        let repository = MemoryRepository::default();
        let application = RealApplication::with_repository(repository.clone());
        let initial = ready(&application);
        let ids = bootstrap_ids();
        let view_id = initial.workspace.active_view.expect("active view");
        let required_slot = initial
            .active_view
            .as_ref()
            .expect("active view")
            .slots
            .iter()
            .find(|slot| slot.required)
            .expect("required slot")
            .slot
            .clone();
        let optional_slot = initial
            .active_view
            .as_ref()
            .expect("active view")
            .slots
            .iter()
            .find(|slot| !slot.required)
            .expect("optional slot")
            .slot
            .clone();

        let opened = block_on(application.dispatch(AppIntent::OpenChart {
            definition_id: ids.chart_definition_b,
        }))
        .expect("open succeeds");
        let chart_b = opened
            .workspace
            .active_chart
            .expect("opened chart is active");
        assert_eq!(
            opened.workspace.selected_charts,
            initial.workspace.selected_charts
        );

        let selected = block_on(application.dispatch(AppIntent::SetChartSelection {
            instance_id: ids.chart_instance_a,
            selected: true,
        }))
        .expect("selection succeeds");
        assert_eq!(selected.workspace.active_chart, Some(chart_b));
        assert_eq!(
            selected.workspace.selected_charts,
            vec![ids.chart_instance_a]
        );

        let activated = block_on(application.dispatch(AppIntent::ActivateChart {
            instance_id: ids.chart_instance_a,
        }))
        .expect("activation succeeds");
        assert_eq!(activated.workspace.active_chart, Some(ids.chart_instance_a));
        assert_eq!(
            activated.workspace.selected_charts,
            vec![ids.chart_instance_a]
        );

        let assigned = block_on(application.dispatch(AppIntent::AssignChartSlot {
            view_id,
            slot: optional_slot,
            chart: Some(chart_b),
        }))
        .expect("slot assignment succeeds");
        let assigned =
            block_on(application.wait_for_update(assigned.version)).expect("slot refresh settles");
        assert!(
            assigned
                .active_view
                .as_ref()
                .expect("active view")
                .slots
                .iter()
                .any(|slot| !slot.required && slot.chart == Some(chart_b))
        );

        let active_view = block_on(application.dispatch(AppIntent::SetActiveView { view_id }))
            .expect("active view succeeds");
        block_on(application.wait_for_update(active_view.version)).expect("view refresh settles");

        let tight = block_on(application.dispatch(AppIntent::SetWorkspaceAspectSet {
            resource_id: ids.aspect_set_tight,
        }))
        .expect("Aspect Set selection succeeds");
        let tight = block_on(application.wait_for_update(tight.version))
            .expect("Aspect Set refresh settles");
        assert_eq!(
            tight.inspector.active_aspect_set,
            Some(ids.aspect_set_tight)
        );

        let closed = block_on(application.dispatch(AppIntent::CloseChart {
            instance_id: ids.chart_instance_a,
        }))
        .expect("close succeeds");
        let closed =
            block_on(application.wait_for_update(closed.version)).expect("close refresh settles");
        assert_eq!(closed.workspace.active_chart, Some(chart_b));
        assert!(
            !closed
                .workspace
                .selected_charts
                .contains(&ids.chart_instance_a)
        );
        let view = closed.active_view.expect("active view");
        assert_eq!(
            view.slots
                .iter()
                .find(|slot| slot.slot == required_slot)
                .expect("required slot")
                .chart,
            Some(chart_b)
        );

        let reloaded = RealApplication::with_repository(repository);
        let restored = ready(&reloaded);
        assert_eq!(restored.workspace.active_chart, Some(chart_b));
        assert_eq!(
            restored.inspector.active_aspect_set,
            Some(ids.aspect_set_tight)
        );
        assert_eq!(restored.workspace.charts.len(), 1);
    }

    #[test]
    fn aspect_preview_cancel_and_save_reuse_calculation_value() {
        let repository = MemoryRepository::default();
        let provider = ControlledProvider::new();
        let calls = Rc::clone(&provider.calls);
        let application = RealApplication::with_provider(repository, provider);
        let initial = ready(&application);
        assert_eq!(calls.get(), 1);
        let original_scene = initial.active_view.unwrap().scene.expect("initial Scene");
        let standard = bootstrap_ids().aspect_set_standard;

        let begin = block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: standard,
        }))
        .expect("begin succeeds");
        assert!(matches!(editor_state(&begin), DraftState::Clean { .. }));
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(4.0),
            },
        )))
        .expect("draft update succeeds");
        assert_eq!(calls.get(), 1);
        assert!(matches!(editor_state(&dirty), DraftState::Dirty { .. }));
        assert_eq!(
            dirty
                .active_view
                .as_ref()
                .and_then(|view| view.scene.clone()),
            Some(original_scene.clone())
        );
        assert!(matches!(
            dirty.active_view.as_ref().map(|view| &view.computation),
            Some(ViewComputationState::Refreshing)
        ));
        let preview =
            block_on(application.wait_for_update(dirty.version)).expect("preview refresh settles");
        assert_eq!(calls.get(), 1);
        assert_ne!(
            preview.active_view.unwrap().scene.expect("preview Scene"),
            original_scene
        );

        let toggled = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetEnabled {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                enabled: false,
            },
        )))
        .expect("enabled mutation succeeds");
        block_on(application.wait_for_update(toggled.version)).expect("toggle preview settles");
        assert_eq!(calls.get(), 1);

        let canceled =
            block_on(application.dispatch(AppIntent::CancelDraft)).expect("cancel succeeds");
        assert!(matches!(editor_state(&canceled), DraftState::Clean { .. }));
        block_on(application.wait_for_update(canceled.version)).expect("cancel refresh settles");
        assert_eq!(calls.get(), 1);

        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(6.5),
            },
        )))
        .expect("second update succeeds");
        block_on(application.wait_for_update(dirty.version)).expect("preview settles");
        let saving = block_on(application.dispatch(AppIntent::SaveDraft)).expect("save starts");
        assert!(matches!(editor_state(&saving), DraftState::Saving { .. }));
        let saved = block_on(application.wait_for_update(saving.version)).expect("save settles");
        assert!(
            matches!(editor_state(&saved), DraftState::Clean { revision } if revision.get() == 2)
        );
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn optimistic_conflict_retains_local_draft_and_cancel_adopts_remote() {
        let repository = MemoryRepository::default();
        let application = RealApplication::with_repository(repository.clone());
        ready(&application);
        let standard = bootstrap_ids().aspect_set_standard;
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: standard,
        }))
        .expect("begin succeeds");
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(9.0),
            },
        )))
        .expect("draft update succeeds");
        block_on(application.wait_for_update(dirty.version)).expect("preview settles");

        let CanonicalResource::AspectSet(remote_one) = block_on(repository.get(standard))
            .expect("repository read succeeds")
            .expect("Aspect Set exists")
        else {
            panic!("bootstrap resource is an Aspect Set");
        };
        let mut remote_payload = remote_one.payload.clone();
        remote_payload
            .aspects
            .iter_mut()
            .find(|aspect| aspect.id.as_str() == "conjunction")
            .expect("conjunction exists")
            .orbs
            .maximum = angle(5.0);
        let remote_two = remote_one
            .next_with_payload(remote_payload, Timestamp::from_unix_millis(50))
            .expect("remote revision is valid");
        block_on(repository.save(Revision::INITIAL, CanonicalResource::AspectSet(remote_two)))
            .expect("external writer saves revision two");

        let saving = block_on(application.dispatch(AppIntent::SaveDraft)).expect("save starts");
        assert!(matches!(editor_state(&saving), DraftState::Saving { .. }));
        let conflict =
            block_on(application.wait_for_update(saving.version)).expect("conflict is projected");
        assert_eq!(conflict.status, ApplicationStatus::Ready);
        assert!(matches!(
            editor_state(&conflict),
            DraftState::Conflict {
                base_revision,
                remote_revision,
            } if *base_revision == Revision::INITIAL && remote_revision.get() == 2
        ));
        let draft = conflict.resource_editor.aspect_set.expect("draft remains");
        assert_eq!(draft.conjunction.maximum_orb, angle(9.0));
        assert_eq!(
            conflict
                .library
                .aspect_sets
                .iter()
                .find(|summary| summary.resource_id == standard)
                .expect("canonical summary")
                .revision
                .get(),
            2
        );

        let canceled = block_on(application.dispatch(AppIntent::CancelDraft))
            .expect("conflict cancel succeeds");
        let canceled_draft = canceled.resource_editor.aspect_set.expect("editor remains");
        assert!(matches!(
            canceled_draft.state,
            DraftState::Clean { revision } if revision.get() == 2
        ));
        assert_eq!(canceled_draft.conjunction.maximum_orb, angle(5.0));
    }

    #[test]
    fn conflict_remote_read_failure_settles_dirty_and_allows_cancel_and_retry() {
        let repository = SaveFailureRepository::new(InjectedSaveFailure::ConflictThenReadFailure);
        let application = RealApplication::with_repository(repository);
        ready(&application);
        let standard = bootstrap_ids().aspect_set_standard;
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: standard,
        }))
        .expect("begin succeeds");
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(9.0),
            },
        )))
        .expect("draft update succeeds");
        block_on(application.wait_for_update(dirty.version)).expect("preview settles");

        let saving = block_on(application.dispatch(AppIntent::SaveDraft)).expect("save starts");
        assert!(matches!(editor_state(&saving), DraftState::Saving { .. }));
        assert!(
            application
                .state
                .borrow()
                .pending
                .iter()
                .any(|pending| matches!(pending, PendingWork::SaveAspectSet { .. }))
        );

        let settled = block_on(application.wait_for_update(saving.version))
            .expect("failed remote refresh settles");
        assert!(settled.version > saving.version);
        assert_eq!(settled.status, ApplicationStatus::Ready);
        assert!(matches!(
            editor_state(&settled),
            DraftState::Dirty { base_revision } if *base_revision == Revision::INITIAL
        ));
        assert_eq!(
            settled
                .resource_editor
                .aspect_set
                .as_ref()
                .expect("draft remains")
                .conjunction
                .maximum_orb,
            angle(9.0)
        );
        let notice = settled.notice.as_ref().expect("warning is projected");
        assert_eq!(notice.kind, AppNoticeKind::Warning);
        assert!(
            notice
                .message
                .contains("remote revision could not be loaded")
        );
        assert!(
            !application
                .state
                .borrow()
                .pending
                .iter()
                .any(|pending| matches!(pending, PendingWork::SaveAspectSet { .. }))
        );

        let canceled = block_on(application.dispatch(AppIntent::CancelDraft))
            .expect("cancel remains available");
        assert!(matches!(editor_state(&canceled), DraftState::Clean { .. }));
        block_on(application.wait_for_update(canceled.version)).expect("cancel refresh settles");

        let retry_dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(7.0),
            },
        )))
        .expect("a new draft update is accepted");
        block_on(application.wait_for_update(retry_dirty.version)).expect("retry preview settles");
        let retry_saving =
            block_on(application.dispatch(AppIntent::SaveDraft)).expect("retry save starts");
        let retried = block_on(application.wait_for_update(retry_saving.version))
            .expect("retry save settles");
        assert!(matches!(
            editor_state(&retried),
            DraftState::Clean { revision } if revision.get() == 2
        ));
        assert!(
            !application
                .state
                .borrow()
                .pending
                .iter()
                .any(|pending| matches!(pending, PendingWork::SaveAspectSet { .. }))
        );
    }

    #[test]
    fn generic_repository_save_failure_settles_dirty_and_can_retry() {
        let repository = SaveFailureRepository::new(InjectedSaveFailure::Adapter);
        let application = RealApplication::with_repository(repository);
        ready(&application);
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: bootstrap_ids().aspect_set_standard,
        }))
        .expect("begin succeeds");
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(8.0),
            },
        )))
        .expect("draft update succeeds");
        block_on(application.wait_for_update(dirty.version)).expect("preview settles");

        let saving = block_on(application.dispatch(AppIntent::SaveDraft)).expect("save starts");
        let failed =
            block_on(application.wait_for_update(saving.version)).expect("generic failure settles");
        assert!(failed.version > saving.version);
        assert_eq!(failed.status, ApplicationStatus::Ready);
        assert!(matches!(editor_state(&failed), DraftState::Dirty { .. }));
        assert_eq!(
            failed
                .resource_editor
                .aspect_set
                .as_ref()
                .expect("draft remains")
                .conjunction
                .maximum_orb,
            angle(8.0)
        );
        assert!(
            !application
                .state
                .borrow()
                .pending
                .iter()
                .any(|pending| matches!(pending, PendingWork::SaveAspectSet { .. }))
        );

        let retry = block_on(application.dispatch(AppIntent::SaveDraft)).expect("retry starts");
        let saved =
            block_on(application.wait_for_update(retry.version)).expect("retry save succeeds");
        assert!(matches!(editor_state(&saved), DraftState::Clean { .. }));
    }

    #[test]
    fn update_notification_is_non_consuming_for_multiple_waiters() {
        let application = RealApplication::in_memory();
        ready(&application);
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: bootstrap_ids().aspect_set_standard,
        }))
        .expect("begin succeeds");
        let refreshing = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(4.0),
            },
        )))
        .expect("update succeeds");
        let after = refreshing.version;

        let (waiter_a, waiter_b) = block_on(async {
            futures::join!(
                application.wait_for_update(after),
                application.wait_for_update(after)
            )
        });
        let waiter_a = waiter_a.expect("waiter A observes the update");
        let waiter_b = waiter_b.expect("waiter B observes the update");
        assert!(waiter_a.version > after);
        assert!(waiter_b.version > after);
        assert_eq!(waiter_a.version, waiter_b.version);
    }

    #[test]
    fn registered_waiters_are_broadcast_a_later_dispatch_transition() {
        let application = RealApplication::in_memory();
        let ready = ready(&application);
        let after = ready.version;
        let active = ready.workspace.active_chart.expect("active chart");

        let (waiter_a, waiter_b, dispatched) = block_on(async {
            futures::join!(
                application.wait_for_update(after),
                application.wait_for_update(after),
                application.dispatch(AppIntent::ActivateChart {
                    instance_id: active,
                })
            )
        });
        let waiter_a = waiter_a.expect("waiter A observes the dispatch");
        let waiter_b = waiter_b.expect("waiter B observes the dispatch");
        let dispatched = dispatched.expect("dispatch succeeds");
        assert_eq!(waiter_a.version, dispatched.version);
        assert_eq!(waiter_b.version, dispatched.version);
        assert!(dispatched.version > after);
    }

    #[test]
    fn failed_real_refresh_keeps_last_good_scene() {
        let repository = MemoryRepository::default();
        let provider = ControlledProvider::new();
        let fail_next = Rc::clone(&provider.fail_next);
        let application = RealApplication::with_provider(repository, provider);
        let initial = ready(&application);
        let original = initial.active_view.unwrap().scene.expect("initial Scene");
        let opened = block_on(application.dispatch(AppIntent::OpenChart {
            definition_id: bootstrap_ids().chart_definition_b,
        }))
        .expect("chart B opens");
        let chart_b = opened.workspace.active_chart.expect("chart B active");
        let view = opened.active_view.expect("active view");
        let required = view
            .slots
            .iter()
            .find(|slot| slot.required)
            .expect("required slot")
            .slot
            .clone();
        fail_next.set(true);
        let refreshing = block_on(application.dispatch(AppIntent::AssignChartSlot {
            view_id: view.view_id,
            slot: required,
            chart: Some(chart_b),
        }))
        .expect("slot assignment succeeds");
        assert_eq!(
            refreshing
                .active_view
                .as_ref()
                .and_then(|view| view.scene.clone()),
            Some(original.clone())
        );
        let failed = block_on(application.wait_for_update(refreshing.version))
            .expect("failure is projected");
        let failed_view = failed.active_view.expect("active view");
        assert_eq!(failed_view.scene, Some(original));
        assert!(matches!(
            failed_view.computation,
            ViewComputationState::Failed(AppError {
                kind: AppErrorKind::ViewComputation,
                ..
            })
        ));

        let retry = block_on(application.dispatch(AppIntent::RefreshActiveView))
            .expect("manual retry is accepted");
        let fresh = block_on(application.wait_for_update(retry.version)).expect("retry settles");
        assert!(matches!(
            fresh.active_view.map(|view| view.computation),
            Some(ViewComputationState::Fresh)
        ));
    }

    #[test]
    fn memory_repository_reload_restores_workspace_and_saved_aspect_revision() {
        let repository = MemoryRepository::default();
        let first = RealApplication::with_repository(repository.clone());
        ready(&first);
        let ids = bootstrap_ids();
        let opened = block_on(first.dispatch(AppIntent::OpenChart {
            definition_id: ids.chart_definition_b,
        }))
        .expect("chart B opens");
        let chart_b = opened.workspace.active_chart.expect("chart B active");
        block_on(first.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: ids.aspect_set_standard,
        }))
        .expect("edit begins");
        let dirty = block_on(first.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: astra_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(6.5),
            },
        )))
        .expect("draft updates");
        block_on(first.wait_for_update(dirty.version)).expect("preview settles");
        let saving = block_on(first.dispatch(AppIntent::SaveDraft)).expect("save starts");
        block_on(first.wait_for_update(saving.version)).expect("save settles");
        drop(first);

        let second = RealApplication::with_repository(repository);
        let restored = ready(&second);
        assert_eq!(restored.workspace.active_chart, Some(chart_b));
        assert!(restored.workspace.charts.iter().any(|chart| {
            matches!(
                chart.persistence,
                ChartPersistence::Saved { definition_id }
                    if definition_id == ids.chart_definition_b
            )
        }));
        let standard = restored
            .library
            .aspect_sets
            .iter()
            .find(|summary| summary.resource_id == ids.aspect_set_standard)
            .expect("Standard is restored");
        assert_eq!(standard.revision.get(), 2);
        assert_eq!(standard.conjunction_orb, angle(6.5));
    }

    #[test]
    fn binding_resolver_preserves_inline_follow_and_pinned_semantics() {
        let id = ResourceId::new();
        let first = ResourceEnvelope::with_id(
            id,
            "Points",
            PointSet {
                points: vec![PointSelector::Category("personal".into())],
            },
            Timestamp::from_unix_millis(1),
        );
        let second = first
            .next_with_payload(
                PointSet {
                    points: vec![PointSelector::Category("planets".into())],
                },
                Timestamp::from_unix_millis(2),
            )
            .expect("second revision");
        let mut catalog = Catalog::default();
        catalog.insert_current(CanonicalResource::PointSet(first.clone()));
        catalog.insert_current(CanonicalResource::PointSet(second.clone()));

        let followed = resolve_typed_binding(&ResourceBinding::<PointSet>::Follow { id }, &catalog)
            .expect("follow resolves");
        let pinned = resolve_typed_binding(
            &ResourceBinding::<PointSet>::Pinned {
                id,
                revision: Revision::INITIAL,
            },
            &catalog,
        )
        .expect("pin resolves");
        let inline = resolve_typed_binding(
            &ResourceBinding::Inline {
                value: PointSet { points: Vec::new() },
            },
            &catalog,
        )
        .expect("inline resolves");

        assert_eq!(followed.revision, Some(second.revision));
        assert_eq!(followed.layer, ResolutionLayer::FollowedResource);
        assert_eq!(pinned.revision, Some(Revision::INITIAL));
        assert_eq!(pinned.layer, ResolutionLayer::PinnedResource);
        assert_eq!(inline.resource, None);
        assert_eq!(inline.revision, None);
        assert_eq!(inline.layer, ResolutionLayer::Inline);
    }

    #[test]
    fn initialization_hydrates_pinned_history_and_projects_true_binding_sources() {
        let repository = MemoryRepository::default();
        let bootstrap = RealApplication::with_repository(repository.clone());
        ready(&bootstrap);
        let ids = bootstrap_ids();

        let CanonicalResource::AspectSet(standard_one) =
            block_on(repository.get(ids.aspect_set_standard))
                .expect("repository read succeeds")
                .expect("Standard exists")
        else {
            panic!("Standard is an Aspect Set");
        };
        let standard_two = standard_one
            .next_with_payload(
                standard_one.payload.clone(),
                Timestamp::from_unix_millis(20),
            )
            .expect("second Aspect Set revision");
        block_on(repository.save(
            standard_one.revision,
            CanonicalResource::AspectSet(standard_two),
        ))
        .expect("Aspect Set advances");

        let CanonicalResource::Workspace(workspace_one) = block_on(repository.get(ids.workspace))
            .expect("repository read succeeds")
            .expect("Workspace exists")
        else {
            panic!("bootstrap resource is a Workspace");
        };
        let mut pinned_payload = workspace_one.payload.clone();
        pinned_payload.profile.aspects = ResourceBinding::Pinned {
            id: ids.aspect_set_standard,
            revision: Revision::INITIAL,
        };
        let workspace_two = workspace_one
            .next_with_payload(pinned_payload, Timestamp::from_unix_millis(21))
            .expect("second Workspace revision");
        block_on(repository.save(
            workspace_one.revision,
            CanonicalResource::Workspace(workspace_two),
        ))
        .expect("Workspace pin saves");
        drop(bootstrap);

        let application = RealApplication::with_repository(repository);
        let restored = ready(&application);
        let aspect_binding = restored
            .inspector
            .bindings
            .iter()
            .find(|binding| binding.label == "Aspect set")
            .expect("Aspect Set binding is projected");
        assert!(matches!(
            aspect_binding.source,
            BindingSourceSummary::Pinned {
                resource_id,
                revision,
                ..
            } if resource_id == ids.aspect_set_standard && revision == Revision::INITIAL
        ));
        assert!(
            restored
                .inspector
                .bindings
                .iter()
                .any(|binding| binding.source == BindingSourceSummary::Inline)
        );
        assert_eq!(
            restored
                .library
                .aspect_sets
                .iter()
                .find(|summary| summary.resource_id == ids.aspect_set_standard)
                .expect("current Standard summary")
                .revision
                .get(),
            2
        );
    }
}
