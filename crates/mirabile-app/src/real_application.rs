use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
};

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use async_trait::async_trait;
use futures::{channel::oneshot, lock::Mutex};
use mirabile_core::{
    AnalysisProfile, AspectSet, CalculationSpec, CanonicalResource, ChartDefinition, ChartRecord,
    ChartSource, Command, ConfigurationStack, EffectiveConfiguration, InstanceId, PointSet,
    ResolutionLayer, Resolved, ResourceBinding, ResourceEnvelope, ResourceId, Revision, Theme,
    Timestamp, ViewDocument, ViewInstance, ViewInstanceId, WheelTemplate, WorkspaceDocument,
    WorkspaceDocumentChart, resolve_binding,
};
use mirabile_engine::{
    AspectAnalyzer, CalcKey, CalculationEngine, CalculationOutcome, CalculationRequestId,
    CalculationWorkerFailure, CalculationWorkerFailureCategory, CalculationWorkerRequest,
    CalculationWorkerResult, ComputationCache, DeterministicBackend, ImplementationIdentity,
    PreparedCalculation, Scene, SnapshotContext, WorkerProtocolVersion, layout_wheel, render_key,
};
#[cfg(target_arch = "wasm32")]
use mirabile_store::ResourceTombstone;
use mirabile_store::{MemoryRepository, RepositoryError, ResourceRepository, ResourceState};

use crate::{
    ActiveChartInspector, AppAction, AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind,
    AppReadModel, AppResult, Application, ApplicationStatus, AspectDraftValue,
    AspectSetDraftMutation, AspectSetDraftReadModel, AspectSetSummary, Availability,
    BindingSourceSummary, CalculationRuntime, CalculationRuntimeError, ChartPersistence,
    ChartSlotAssignment, CommandCapability, DraftState, InlineCalculationRuntime,
    InspectorReadModel, LibraryChartSummary, LibraryReadModel, OpenChartSummary, ProjectionVersion,
    ResourceBindingSummary, ResourceEditorReadModel, StartupCalculationProfile, StartupPolicy,
    ViewComputationState, ViewReadModel, ViewSummary, WorkspaceReadModel, WorkspaceSession,
    blank_workspace_session, current_transits_session, current_unix_millis,
    workspace_commands::apply_workspace_command,
};
#[cfg(feature = "xalen-backend")]
use mirabile_engine::XalenBackend;

pub const DEFAULT_INDEXED_DB_NAME: &str = "mirabile";

type Clock = fn() -> i64;

pub struct RealApplication<R, C = InlineCalculationRuntime<DeterministicBackend>> {
    repository: R,
    engine: CalculationEngine,
    runtime: C,
    startup_policy: StartupPolicy,
    startup_calculation_profile: StartupCalculationProfile,
    clock: Clock,
    runtime_receive_gate: Mutex<()>,
    state: RefCell<RealState>,
}

impl<R> RealApplication<R, InlineCalculationRuntime<DeterministicBackend>>
where
    R: ResourceRepository + Clone,
{
    pub fn with_repository(repository: R) -> Self {
        Self::with_backend(repository, DeterministicBackend)
    }

    pub fn with_repository_and_policy(repository: R, startup_policy: StartupPolicy) -> Self {
        Self::with_runtime_and_policy(
            repository,
            InlineCalculationRuntime::new(DeterministicBackend),
            startup_policy,
        )
    }
}

impl<R, B> RealApplication<R, InlineCalculationRuntime<B>>
where
    R: ResourceRepository + Clone,
    B: mirabile_engine::CalculationBackend + Clone,
{
    /// Constructs an inline application with the baseline no-correction seed.
    /// Backends that require an apparent-place profile must use their
    /// profile-specific constructor instead.
    pub fn with_backend(repository: R, backend: B) -> Self {
        Self::with_runtime(repository, InlineCalculationRuntime::new(backend))
    }
}

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    pub fn with_runtime(repository: R, runtime: C) -> Self {
        Self::with_runtime_and_policy(repository, runtime, StartupPolicy::default())
    }

    pub fn with_runtime_and_policy(
        repository: R,
        runtime: C,
        startup_policy: StartupPolicy,
    ) -> Self {
        Self::with_runtime_startup_profile_and_clock(
            repository,
            runtime,
            startup_policy,
            StartupCalculationProfile::Baseline,
            current_unix_millis,
        )
    }

    fn with_runtime_startup_profile_and_clock(
        repository: R,
        runtime: C,
        startup_policy: StartupPolicy,
        startup_calculation_profile: StartupCalculationProfile,
        clock: Clock,
    ) -> Self {
        let descriptor = runtime.backend_descriptor();
        Self {
            repository,
            engine: CalculationEngine::new(
                descriptor,
                ImplementationIdentity {
                    id: "mirabile-calculation-engine".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    revision: Some("calculation-runtime-v3".into()),
                },
                "deterministic-tz-v1",
            ),
            runtime,
            startup_policy,
            startup_calculation_profile,
            clock,
            runtime_receive_gate: Mutex::new(()),
            state: RefCell::new(RealState::default()),
        }
    }
}

impl RealApplication<MemoryRepository, InlineCalculationRuntime<DeterministicBackend>> {
    pub fn in_memory() -> Self {
        Self::with_repository(MemoryRepository::default())
    }
}

#[cfg(feature = "xalen-backend")]
impl<R> RealApplication<R, InlineCalculationRuntime<XalenBackend>>
where
    R: ResourceRepository + Clone,
{
    /// Constructs a native XALEN application with its required apparent-place profile.
    pub fn with_xalen_backend(repository: R) -> Self {
        Self::with_xalen_backend_and_policy(repository, StartupPolicy::default())
    }

    pub fn with_xalen_backend_and_policy(repository: R, startup_policy: StartupPolicy) -> Self {
        Self::with_runtime_startup_profile_and_clock(
            repository,
            InlineCalculationRuntime::new(XalenBackend),
            startup_policy,
            StartupCalculationProfile::ApparentPlace,
            current_unix_millis,
        )
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug)]
pub struct IndexedDbRepositorySource {
    database_name: String,
    repository: Rc<RefCell<Option<mirabile_store::IndexedDbRepository>>>,
}

#[cfg(target_arch = "wasm32")]
impl IndexedDbRepositorySource {
    pub fn new(database_name: impl Into<String>) -> Self {
        Self {
            database_name: database_name.into(),
            repository: Rc::new(RefCell::new(None)),
        }
    }

    async fn acquire(&self) -> Result<mirabile_store::IndexedDbRepository, RepositoryError> {
        if let Some(repository) = self.repository.borrow().clone() {
            return Ok(repository);
        }
        let opened = mirabile_store::IndexedDbRepository::open(&self.database_name).await?;
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
        kind: Option<mirabile_core::ResourceKind>,
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
impl RealApplication<IndexedDbRepositorySource, crate::WorkerCalculationRuntime> {
    #[cfg(feature = "xalen-backend")]
    pub fn indexed_db_with_runtime(
        database_name: impl Into<String>,
        runtime: crate::WorkerCalculationRuntime,
    ) -> Self {
        Self::indexed_db_with_runtime_and_policy(database_name, runtime, StartupPolicy::default())
    }

    #[cfg(feature = "xalen-backend")]
    pub fn indexed_db_with_runtime_and_policy(
        database_name: impl Into<String>,
        runtime: crate::WorkerCalculationRuntime,
        startup_policy: StartupPolicy,
    ) -> Self {
        Self::with_runtime_startup_profile_and_clock(
            IndexedDbRepositorySource::new(database_name),
            runtime,
            startup_policy,
            StartupCalculationProfile::ApparentPlace,
            current_unix_millis,
        )
    }

    pub fn indexed_db(database_name: impl Into<String>) -> Self {
        #[cfg(feature = "xalen-backend")]
        {
            return Self::indexed_db_with_runtime(
                database_name,
                crate::WorkerCalculationRuntime::xalen(),
            );
        }
        #[cfg(not(feature = "xalen-backend"))]
        Self::with_runtime(
            IndexedDbRepositorySource::new(database_name),
            crate::WorkerCalculationRuntime::deterministic(),
        )
    }

    pub fn browser_default() -> Self {
        Self::indexed_db(DEFAULT_INDEXED_DB_NAME)
    }
}

#[async_trait(?Send)]
impl<R, C> Application for RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    async fn initialize(&self) -> AppResult<AppReadModel> {
        if matches!(self.state.borrow().status, ApplicationStatus::Ready) {
            return self.read_model();
        }

        match self.hydrate().await {
            Ok(hydrated) => {
                let mut state = self.state.borrow_mut();
                state.catalog = hydrated.catalog;
                state.session = Some(hydrated.session);
                state.workspace = hydrated.workspace;
                state.next_timestamp = hydrated.next_timestamp;
                state.status = ApplicationStatus::Ready;
                state.editor = None;
                state.pending.clear();
                state.inflight.clear();
                state.notice = Some(info(
                    "Canonical library hydrated and startup session established",
                ));
                state.ensure_view_runtimes();
                self.submit_active_view_refresh(&mut state)?;
                state.advance()?;
            }
            Err(error) => {
                let mut state = self.state.borrow_mut();
                state.status = ApplicationStatus::Error(error);
                state.pending.clear();
                state.inflight.clear();
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
            | AppIntent::AssignChartSlot { .. }
            | AppIntent::SetWorkspaceAspectSet { .. } => {
                self.dispatch_workspace_intent(&intent)?;
            }
            AppIntent::ActivateChart { instance_id } => {
                self.activate_session_chart(instance_id)?;
            }
            AppIntent::SetChartSelection {
                instance_id,
                selected,
            } => self.set_session_chart_selection(instance_id, selected)?,
            AppIntent::SetActiveView { view_id } => self.set_active_session_view(view_id)?,
            AppIntent::SaveWorkspace => self.save_workspace().await?,
            AppIntent::SetTemporaryPointHidden { point_id, hidden } => {
                self.set_temporary_point_hidden(point_id, hidden)?;
            }
            AppIntent::PromoteTemporaryDisplay => self.promote_temporary_display()?,
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
                if state.pending.is_empty() && state.inflight.is_empty() {
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
                self.complete_next_pending(after).await?;
            }
        }
    }
}

impl<R, C> RealApplication<R, C>
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    async fn hydrate(&self) -> AppResult<HydratedState> {
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
        Ok(HydratedState {
            catalog,
            workspace,
            session,
            next_timestamp: latest_timestamp.saturating_add(1),
        })
    }

    fn activate_session_chart(&self, instance_id: InstanceId) -> AppResult<()> {
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

    fn set_session_chart_selection(
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

    fn set_active_session_view(&self, view_id: ViewInstanceId) -> AppResult<()> {
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

    fn startup_session(
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

    fn saved_startup_session(
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

    fn dispatch_workspace_intent(&self, intent: &AppIntent) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let workspace_id = state
            .workspace
            .as_ref()
            .map(|workspace| workspace.id)
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Unavailable,
                    "The active session has no saved WorkspaceDocument backing",
                )
            })?;
        let document = state
            .session
            .as_ref()
            .map(|session| session.document.clone())
            .ok_or_else(|| {
                AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
            })?;
        let (command, refresh, clear_editor, notice) =
            state.command_for_intent(workspace_id, &document, intent)?;
        let view_documents = state.resolve_view_documents(&document)?;
        let session = state.session.as_mut().expect("session was checked");
        apply_workspace_command(workspace_id, session, &command, &view_documents)
            .map_err(|error| AppError::new(AppErrorKind::InvalidIntent, error.to_string()))?;
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

    async fn save_workspace(&self) -> AppResult<()> {
        let (expected_revision, next) = {
            let state = self.state.borrow();
            let envelope = state.workspace.as_ref().ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Unavailable,
                    "The active session has no saved WorkspaceDocument backing",
                )
            })?;
            let session = state.session.as_ref().ok_or_else(|| {
                AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
            })?;
            if !session.document_dirty {
                return Err(AppError::new(
                    AppErrorKind::InvalidIntent,
                    "The WorkspaceDocument has no changes to save",
                ));
            }
            let next = envelope
                .next_with_payload(
                    session.document.clone(),
                    Timestamp::from_unix_millis(state.next_timestamp),
                )
                .map_err(|error| {
                    AppError::new(
                        AppErrorKind::InvalidIntent,
                        format!("WorkspaceDocument draft was invalid: {error}"),
                    )
                })?;
            (envelope.revision, next)
        };

        self.repository
            .save(
                expected_revision,
                CanonicalResource::WorkspaceDocument(next.clone()),
            )
            .await
            .map_err(|error| {
                repository_app_error("Could not save the WorkspaceDocument", &error)
            })?;

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
        state.notice = Some(success("Workspace saved as a new canonical revision"));
        state.advance()
    }

    fn set_temporary_point_hidden(
        &self,
        point_id: mirabile_core::PointId,
        hidden: bool,
    ) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let session = state.session.as_mut().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })?;
        let view_id = session
            .active_view
            .ok_or_else(|| AppError::new(AppErrorKind::Unavailable, "There is no active view"))?;
        let overrides = session.temporary_view_overrides.entry(view_id).or_default();
        if hidden && !overrides.hidden_points.contains(&point_id) {
            overrides.hidden_points.push(point_id);
        } else if !hidden {
            overrides.hidden_points.retain(|point| point != &point_id);
        }
        if overrides == &mirabile_core::ViewOverrides::default() {
            session.temporary_view_overrides.remove(&view_id);
        }
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Temporary display override changed for this session without dirtying the workspace",
        ));
        state.advance()
    }

    fn promote_temporary_display(&self) -> AppResult<()> {
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
        self.submit_active_view_refresh(&mut state)?;
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
        self.submit_active_view_refresh(&mut state)?;
        state.notice = Some(info(
            "Draft canceled; canonical Aspect Set semantics restored without a repository write",
        ));
        state.advance()
    }

    fn refresh_active_view(&self) -> AppResult<()> {
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

    async fn complete_next_pending(&self, after: ProjectionVersion) -> AppResult<()> {
        let pending = self.state.borrow_mut().pending.pop_front();
        match pending {
            Some(PendingWork::CompleteCachedView(pending)) => self.complete_cached_view(*pending),
            Some(PendingWork::SaveAspectSet {
                expected_revision,
                next,
            }) => self.complete_aspect_set_save(expected_revision, next).await,
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

    fn complete_cached_view(&self, pending: PendingCachedView) -> AppResult<()> {
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
        let result = Self::finish_scene(&mut state, &prepared, &plan, calculation);
        Self::publish_view_result(&mut state, view_id, result)
    }

    fn accept_worker_result(&self, result: CalculationWorkerResult) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let Some(pending) = state.inflight.remove(&result.request_id) else {
            return Ok(());
        };
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
                let scene =
                    Self::finish_scene(&mut state, &pending.prepared, &pending.plan, calculation);
                Self::publish_view_result(&mut state, pending.view_id, scene)
            }
            CalculationOutcome::Failure(failure) => Self::publish_view_result(
                &mut state,
                pending.view_id,
                Err(worker_failure_error(&failure)),
            ),
        }
    }

    fn accept_runtime_failure(&self, error: &CalculationRuntimeError) -> AppResult<()> {
        let mut state = self.state.borrow_mut();
        let affected = state
            .views
            .iter()
            .filter_map(|(view_id, runtime)| runtime.expected.as_ref().map(|_| *view_id))
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
            runtime.computation = ViewComputationState::Failed(AppError::new(
                AppErrorKind::ViewComputation,
                format!("Calculation runtime failed: {}", error.message),
            ));
        }
        state.notice = Some(AppNotice {
            kind: AppNoticeKind::Warning,
            message: format!(
                "Calculation runtime failed; last good Scenes remain visible: {}",
                error.message
            ),
        });
        state.advance()
    }

    fn publish_view_result(
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

    fn submit_active_view_refresh(&self, state: &mut RealState) -> AppResult<()> {
        let Some(view_id) = state
            .session
            .as_ref()
            .and_then(|session| session.active_view)
        else {
            return Ok(());
        };
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
                    prepared,
                    plan,
                },
            );
        }
        let runtime = state.views.entry(view_id).or_default();
        runtime.expected = Some(expected);
        runtime.computation = if runtime.scene.is_some() {
            ViewComputationState::Refreshing
        } else {
            ViewComputationState::Loading
        };
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn prepare_view_calculation(
        &self,
        state: &RealState,
        view_id: ViewInstanceId,
    ) -> AppResult<(PreparedCalculation, ViewCalculationPlan)> {
        let workspace = state
            .session
            .as_ref()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorKind::ViewComputation,
                    "No workspace session is active",
                )
            })?
            .document
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
        let (prepared, effective) = if let Some(workspace_chart) = workspace
            .chart_instances
            .iter()
            .find(|chart| chart.instance_id() == chart_instance)
        {
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
            },
        ))
    }

    fn finish_scene(
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
        let layout = layout_wheel(&snapshot, &analysis, &plan.displayed_points, &plan.wheel)
            .map_err(view_computation_error)?;
        render_key(&layout, &plan.theme).map_err(view_computation_error)?;
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
    workspace: Option<ResourceEnvelope<WorkspaceDocument>>,
    session: Option<WorkspaceSession>,
    views: BTreeMap<ViewInstanceId, ViewRuntime>,
    editor: Option<AspectSetEditor>,
    cache: ComputationCache,
    pending: VecDeque<PendingWork>,
    inflight: BTreeMap<CalculationRequestId, PendingViewCalculation>,
    next_request_id: CalculationRequestId,
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
            session: None,
            views: BTreeMap::new(),
            editor: None,
            cache: ComputationCache::default(),
            pending: VecDeque::new(),
            inflight: BTreeMap::new(),
            next_request_id: CalculationRequestId::FIRST,
            waiters: Vec::new(),
            notice: None,
            next_timestamp: 1,
        }
    }
}

impl RealState {
    fn workspace(&self) -> Option<&WorkspaceDocument> {
        self.session.as_ref().map(|session| &session.document)
    }

    fn session(&self) -> AppResult<&WorkspaceSession> {
        self.session.as_ref().ok_or_else(|| {
            AppError::new(AppErrorKind::Unavailable, "No workspace session is active")
        })
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

    fn effective_configuration(
        &self,
        calculation_spec: &CalculationSpec,
        view: &ViewInstance,
    ) -> AppResult<EffectiveConfiguration> {
        let workspace = self.workspace().ok_or_else(|| {
            AppError::new(AppErrorKind::ViewComputation, "No workspace is hydrated")
        })?;
        let calculation = ConfigurationStack {
            built_in: CalculationSpec::default(),
            user_default: None,
            workspace: None,
            chart_definition: Some(calculation_spec.clone()),
            view_override: None,
            editor_preview: None,
        }
        .resolve();
        let mut displayed_points =
            resolve_typed_binding(&workspace.profile.displayed_points, &self.catalog)
                .map_err(view_resolution_error)?;
        let temporary_hidden = self
            .session
            .as_ref()
            .and_then(|session| session.temporary_view_overrides.get(&view.id))
            .map(|overrides| overrides.hidden_points.as_slice())
            .unwrap_or_default();
        if !view.overrides.hidden_points.is_empty() || !temporary_hidden.is_empty() {
            displayed_points
                .value
                .points
                .retain(|selector| match selector {
                    mirabile_core::PointSelector::Point(point) => {
                        !view.overrides.hidden_points.contains(point)
                            && !temporary_hidden.contains(point)
                    }
                    mirabile_core::PointSelector::Category(_) => true,
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
        workspace: &WorkspaceDocument,
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
                        workspace: workspace_id,
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
                    workspace: workspace_id,
                    instance_id: *instance_id,
                },
                true,
                false,
                "Chart closed in the working document; selection and slots were repaired and the workspace is dirty",
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
                    "Chart slot assignment changed in the working document; save the workspace to persist it",
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
                    "Workspace Aspect Set binding changed; the workspace is dirty and analysis is refreshing",
                ))
            }
            AppIntent::BeginAspectSetEdit { .. }
            | AppIntent::UpdateAspectSetDraft(_)
            | AppIntent::SaveDraft
            | AppIntent::CancelDraft
            | AppIntent::SaveWorkspace
            | AppIntent::SetTemporaryPointHidden { .. }
            | AppIntent::PromoteTemporaryDisplay
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
            .session
            .as_ref()
            .and_then(|session| session.active_view)
            .and_then(|id| self.views.get(&id))
            .map_or_else(|| disabled("No active view"), |_| Availability::Enabled);
        let save_workspace = self.session.as_ref().map_or_else(
            || disabled("No workspace session"),
            |session| {
                if session.document_dirty && self.workspace.is_some() {
                    Availability::Enabled
                } else if self.workspace.is_none() {
                    disabled("This session has no saved WorkspaceDocument backing")
                } else {
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
            capability(AppAction::BeginAspectSetEdit, begin),
            capability(AppAction::SaveDraft, save),
            capability(AppAction::CancelDraft, cancel),
            capability(AppAction::SaveWorkspace, save_workspace),
            capability(AppAction::PromoteWorkspaceDisplay, promote_display),
            capability(AppAction::RefreshView, refresh),
        ]
    }
}

struct HydratedState {
    catalog: Catalog,
    workspace: Option<ResourceEnvelope<WorkspaceDocument>>,
    session: WorkspaceSession,
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

    fn workspace(&self, id: ResourceId) -> Option<&ResourceEnvelope<WorkspaceDocument>> {
        match self.current.get(&id) {
            Some(CanonicalResource::WorkspaceDocument(value)) => Some(value),
            _ => None,
        }
    }

    fn pinned_references(&self) -> Vec<(ResourceId, Revision)> {
        let mut references = Vec::new();
        for resource in self.current.values() {
            let CanonicalResource::WorkspaceDocument(workspace) = resource else {
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
                    ChartSource::Radix { record } => self.chart_record(record).map_or_else(
                        || "Missing source record".into(),
                        |record| chart_record_subtitle(&record.payload),
                    ),
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

    fn open_chart_summary(&self, chart: &WorkspaceDocumentChart) -> AppResult<OpenChartSummary> {
        let definition_envelope = self
            .chart_definition(chart.definition)
            .ok_or_else(|| not_found("ChartDefinition", chart.definition))?;
        let subtitle = match definition_envelope.payload.source {
            ChartSource::Radix { record } => self.chart_record(record).map_or_else(
                || "Missing source record".into(),
                |record| chart_record_subtitle(&record.payload),
            ),
            ChartSource::Derived { .. } => "Derived chart".into(),
        };
        Ok(OpenChartSummary {
            instance_id: chart.instance_id,
            title: definition_envelope.title.clone(),
            subtitle,
            persistence: ChartPersistence::Saved {
                definition_id: chart.definition,
            },
        })
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
    expected: Option<ExpectedCalculation>,
}

impl Default for ViewRuntime {
    fn default() -> Self {
        Self {
            scene: None,
            computation: ViewComputationState::Loading,
            expected: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedCalculation {
    request_id: CalculationRequestId,
    calc_key: CalcKey,
}

struct PendingViewCalculation {
    view_id: ViewInstanceId,
    prepared: PreparedCalculation,
    plan: ViewCalculationPlan,
}

struct ViewCalculationPlan {
    displayed_points: PointSet,
    aspected_points: PointSet,
    aspect_set: AspectSet,
    analysis: AnalysisProfile,
    wheel: WheelTemplate,
    theme: Theme,
}

struct AspectSetEditor {
    base: ResourceEnvelope<AspectSet>,
    draft: AspectSet,
    state: DraftState,
}

enum PendingWork {
    CompleteCachedView(Box<PendingCachedView>),
    SaveAspectSet {
        expected_revision: Revision,
        next: ResourceEnvelope<AspectSet>,
    },
}

struct PendingCachedView {
    view_id: ViewInstanceId,
    expected: ExpectedCalculation,
    prepared: PreparedCalculation,
    plan: ViewCalculationPlan,
    calculation: mirabile_engine::CalculationValue,
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
) -> Result<Resolved<T>, mirabile_core::BindingResolutionError> {
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

fn conjunction(aspects: &AspectSet) -> AppResult<&mirabile_core::AspectDefinition> {
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

fn chart_record_subtitle(record: &ChartRecord) -> String {
    let date = record.time.civil_datetime.date;
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
        record
            .location
            .as_ref()
            .map_or("Location unknown", |location| location
                .display_name
                .as_str())
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
        CanonicalResource::WorkspaceDocument(value) => value.modified_at.unix_millis(),
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

fn view_resolution_error(error: mirabile_core::BindingResolutionError) -> AppError {
    AppError::new(
        AppErrorKind::ViewComputation,
        format!("Effective configuration could not be resolved: {error}"),
    )
}

fn view_computation_error(error: impl std::fmt::Display) -> AppError {
    AppError::new(AppErrorKind::ViewComputation, error.to_string())
}

fn worker_failure_error(failure: &CalculationWorkerFailure) -> AppError {
    let category = match failure.category {
        CalculationWorkerFailureCategory::InvalidInput => "invalid calculation input",
        CalculationWorkerFailureCategory::UnsupportedCapability => "unsupported capability",
        CalculationWorkerFailureCategory::BackendFailure => "backend failure",
        CalculationWorkerFailureCategory::ProtocolMismatch => "worker protocol mismatch",
        CalculationWorkerFailureCategory::InternalExecutionFailure => {
            "internal worker execution failure"
        }
    };
    AppError::new(
        AppErrorKind::ViewComputation,
        format!("Calculation {category}: {}", failure.message),
    )
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    use futures::{
        executor::{LocalPool, block_on},
        task::LocalSpawnExt,
    };
    use mirabile_core::{
        Angle, CanonicalResource, PointId, PointSelector, ResourceEnvelope, ResourceKind,
    };
    use mirabile_engine::{
        BackendDescriptor, CalculationBackend, CalculationBackendError,
        CalculationBackendErrorCategory, CalculationBackendResult, ResolvedCalculationRequest,
    };
    use mirabile_store::ResourceTombstone;

    use crate::{demo_ids, demo_resources};

    use super::*;

    #[derive(Clone)]
    struct ControlledBackend {
        calls: Rc<Cell<u32>>,
        fail_next: Rc<Cell<bool>>,
    }

    impl ControlledBackend {
        fn new() -> Self {
            Self {
                calls: Rc::new(Cell::new(0)),
                fail_next: Rc::new(Cell::new(false)),
            }
        }
    }

    impl CalculationBackend for ControlledBackend {
        fn descriptor(&self) -> BackendDescriptor {
            DeterministicBackend.descriptor()
        }

        fn calculate(
            &self,
            request: &ResolvedCalculationRequest,
        ) -> Result<CalculationBackendResult, CalculationBackendError> {
            self.calls.set(self.calls.get() + 1);
            if self.fail_next.replace(false) {
                Err(CalculationBackendError {
                    category: CalculationBackendErrorCategory::ExecutionFailure,
                    capability: None,
                    message: "injected deterministic backend failure".into(),
                })
            } else {
                DeterministicBackend.calculate(request)
            }
        }
    }

    #[derive(Clone)]
    struct ControlledCalculationRuntime {
        descriptor: BackendDescriptor,
        submitted: Rc<RefCell<Vec<CalculationWorkerRequest>>>,
        receive_calls: Rc<Cell<usize>>,
        inbox: crate::RuntimeInbox,
    }

    impl ControlledCalculationRuntime {
        fn new() -> Self {
            Self {
                descriptor: DeterministicBackend.descriptor(),
                submitted: Rc::new(RefCell::new(Vec::new())),
                receive_calls: Rc::new(Cell::new(0)),
                inbox: crate::RuntimeInbox::default(),
            }
        }

        fn submitted(&self) -> Vec<CalculationWorkerRequest> {
            self.submitted.borrow().clone()
        }

        fn receive_calls(&self) -> usize {
            self.receive_calls.get()
        }

        fn complete_success(&self, request: &CalculationWorkerRequest, sun_shift: f64) {
            let mut result = mirabile_engine::execute_calculation_request(
                &DeterministicBackend,
                request.clone(),
            );
            if let CalculationOutcome::Success(calculation) = &mut result.outcome {
                let sun = PointId::new("sun").expect("point ID");
                if let Some(position) = calculation.celestial.positions.get_mut(&sun) {
                    position.longitude =
                        Angle::normalized(position.longitude.degrees() + sun_shift)
                            .expect("shifted longitude");
                    position.right_ascension = position.longitude;
                }
            }
            self.inbox.push(Ok(result));
        }

        fn complete_failure(&self, request: &CalculationWorkerRequest, message: &str) {
            self.inbox.push(Ok(CalculationWorkerResult {
                protocol_version: WorkerProtocolVersion::CURRENT,
                request_id: request.request_id,
                calc_key: request.calc_key.clone(),
                outcome: CalculationOutcome::Failure(CalculationWorkerFailure {
                    category: CalculationWorkerFailureCategory::BackendFailure,
                    message: message.into(),
                }),
            }));
        }

        fn complete_calc_key_mismatch(
            &self,
            request: &CalculationWorkerRequest,
            wrong_calc_key: CalcKey,
        ) {
            let mut result = mirabile_engine::execute_calculation_request(
                &DeterministicBackend,
                request.clone(),
            );
            result.calc_key = wrong_calc_key;
            self.inbox.push(Ok(result));
        }
    }

    #[async_trait(?Send)]
    impl CalculationRuntime for ControlledCalculationRuntime {
        fn backend_descriptor(&self) -> BackendDescriptor {
            self.descriptor.clone()
        }

        fn submit(&self, request: CalculationWorkerRequest) -> Result<(), CalculationRuntimeError> {
            self.submitted.borrow_mut().push(request);
            Ok(())
        }

        async fn receive(&self) -> Result<CalculationWorkerResult, CalculationRuntimeError> {
            self.receive_calls.set(self.receive_calls.get() + 1);
            self.inbox.receive().await
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

    fn ready<R, C>(application: &RealApplication<R, C>) -> AppReadModel
    where
        R: ResourceRepository + Clone,
        C: CalculationRuntime,
    {
        let loading = block_on(application.initialize()).expect("initialization succeeds");
        assert_eq!(loading.status, ApplicationStatus::Ready);
        assert!(matches!(
            loading.active_view.as_ref().map(|view| &view.computation),
            Some(ViewComputationState::Loading)
        ));
        block_on(application.wait_for_update(loading.version)).expect("initial view settles")
    }

    fn controlled_ready(
        application: &RealApplication<MemoryRepository, ControlledCalculationRuntime>,
        runtime: &ControlledCalculationRuntime,
    ) -> AppReadModel {
        let loading = block_on(application.initialize()).expect("initialization succeeds");
        assert!(matches!(
            loading.active_view.as_ref().map(|view| &view.computation),
            Some(ViewComputationState::Loading)
        ));
        let request = runtime
            .submitted()
            .last()
            .cloned()
            .expect("initial calculation submitted");
        runtime.complete_success(&request, 0.0);
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

    fn ensure_demo<R>(repository: &R)
    where
        R: ResourceRepository + Clone,
    {
        for resource in demo_resources() {
            if block_on(repository.get_head(resource.id()))
                .expect("demo identity can be inspected")
                .is_none()
            {
                block_on(repository.create(resource)).expect("demo resource can be created");
            }
        }
    }

    fn demo_application<R>(
        repository: R,
    ) -> RealApplication<R, InlineCalculationRuntime<DeterministicBackend>>
    where
        R: ResourceRepository + Clone,
    {
        ensure_demo(&repository);
        RealApplication::with_repository_and_policy(
            repository,
            StartupPolicy::OpenWorkspace(demo_ids().workspace),
        )
    }

    fn demo_backend_application<R, B>(
        repository: R,
        backend: B,
    ) -> RealApplication<R, InlineCalculationRuntime<B>>
    where
        R: ResourceRepository + Clone,
        B: CalculationBackend + Clone,
    {
        ensure_demo(&repository);
        RealApplication::with_runtime_and_policy(
            repository,
            InlineCalculationRuntime::new(backend),
            StartupPolicy::OpenWorkspace(demo_ids().workspace),
        )
    }

    fn demo_runtime_application<R, C>(repository: R, runtime: C) -> RealApplication<R, C>
    where
        R: ResourceRepository + Clone,
        C: CalculationRuntime,
    {
        ensure_demo(&repository);
        RealApplication::with_runtime_and_policy(
            repository,
            runtime,
            StartupPolicy::OpenWorkspace(demo_ids().workspace),
        )
    }

    #[derive(Clone, Copy)]
    enum TestViewDocumentBinding {
        Inline,
        Follow,
        Pinned,
    }

    fn repository_with_view_document_binding(binding: TestViewDocumentBinding) -> MemoryRepository {
        let repository = MemoryRepository::default();
        let demo = demo_application(repository.clone());
        ready(&demo);
        drop(demo);

        if matches!(binding, TestViewDocumentBinding::Inline) {
            return repository;
        }

        let workspace_id = demo_ids().workspace;
        let CanonicalResource::WorkspaceDocument(workspace) =
            block_on(repository.get(workspace_id))
                .expect("workspace read succeeds")
                .expect("workspace exists")
        else {
            panic!("demo resource is a WorkspaceDocument");
        };
        let ResourceBinding::Inline { value: document } = &workspace.payload.views[0].document
        else {
            panic!("demo ViewDocument is inline");
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
            .expect("WorkspaceDocument binding revision is valid");
        block_on(repository.save(
            workspace.revision,
            CanonicalResource::WorkspaceDocument(next),
        ))
        .expect("WorkspaceDocument binding persists");
        repository
    }

    fn assert_close_repair_uses_resolved_view_document(repository: MemoryRepository) {
        let application = demo_application(repository);
        let initial = ready(&application);
        let ids = demo_ids();
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
    fn initialization_is_versioned_and_snapshot_is_immediate() {
        let repository = MemoryRepository::default();
        let first = demo_application(repository.clone());
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

        let second = demo_application(repository.clone());
        ready(&second);
        assert_eq!(repository.current_count(), 7);
        assert_eq!(repository.revision_count(), 7);
    }

    #[test]
    fn genuinely_empty_repository_starts_ephemeral_current_transits_without_writes() {
        let repository = MemoryRepository::default();
        let application = RealApplication::with_repository(repository.clone());
        let projection = ready(&application);

        assert!(projection.library.charts.is_empty());
        assert!(projection.library.aspect_sets.is_empty());
        assert_eq!(projection.workspace.charts.len(), 1);
        assert_eq!(
            projection.workspace.charts[0].persistence,
            ChartPersistence::Ephemeral
        );
        assert_eq!(projection.workspace.document_id, None);
        assert_eq!(projection.workspace.document_revision, None);
        assert!(!projection.workspace.document_dirty);
        assert_eq!(repository.current_count(), 0);
        assert_eq!(repository.revision_count(), 0);
    }

    #[cfg(feature = "xalen-backend")]
    #[test]
    fn native_xalen_constructor_reaches_a_fresh_scene() {
        let application = RealApplication::with_xalen_backend(MemoryRepository::default());
        let projection = ready(&application);
        let active_view = projection.active_view.expect("active view");
        assert!(matches!(
            active_view.computation,
            ViewComputationState::Fresh
        ));
        assert!(active_view.scene.is_some());
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
    fn workspace_dirty_save_and_session_navigation_semantics() {
        let repository = MemoryRepository::default();
        let application = demo_application(repository.clone());
        let initial = ready(&application);
        let ids = demo_ids();
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
        let before_save = block_on(repository.get(ids.workspace))
            .expect("workspace read")
            .expect("workspace exists");
        assert_eq!(before_save.revision(), Revision::INITIAL);
        assert!(selected.workspace.document_dirty);

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
        assert!(closed.workspace.document_dirty);

        let saved = block_on(application.dispatch(AppIntent::SaveWorkspace))
            .expect("workspace save succeeds");
        assert!(!saved.workspace.document_dirty);
        assert_eq!(
            saved.workspace.document_revision.map(Revision::get),
            Some(2)
        );

        let reloaded = demo_application(repository);
        let restored = ready(&reloaded);
        assert_eq!(restored.workspace.active_chart, Some(chart_b));
        assert_eq!(
            restored.inspector.active_aspect_set,
            Some(ids.aspect_set_tight)
        );
        assert_eq!(restored.workspace.charts.len(), 1);
    }

    #[test]
    fn temporary_display_override_requires_explicit_promotion_and_workspace_save() {
        let repository = MemoryRepository::default();
        let application = demo_application(repository.clone());
        let initial = ready(&application);
        let ids = demo_ids();
        assert!(!initial.workspace.document_dirty);
        let sun = PointId::new("sun").expect("point ID");

        let temporary = block_on(application.dispatch(AppIntent::SetTemporaryPointHidden {
            point_id: sun.clone(),
            hidden: true,
        }))
        .expect("temporary override succeeds");
        assert!(!temporary.workspace.document_dirty);
        assert!(temporary.workspace.has_temporary_display_override);
        block_on(application.wait_for_update(temporary.version))
            .expect("temporary preview settles");
        let canonical = block_on(repository.get(ids.workspace))
            .expect("workspace read")
            .expect("workspace exists");
        assert_eq!(canonical.revision(), Revision::INITIAL);

        let promoted = block_on(application.dispatch(AppIntent::PromoteTemporaryDisplay))
            .expect("promotion succeeds");
        assert!(promoted.workspace.document_dirty);
        assert!(!promoted.workspace.has_temporary_display_override);
        block_on(application.wait_for_update(promoted.version)).expect("promoted preview settles");

        let saved = block_on(application.dispatch(AppIntent::SaveWorkspace))
            .expect("workspace save succeeds");
        assert!(!saved.workspace.document_dirty);
        let canonical = block_on(repository.get(ids.workspace))
            .expect("workspace read")
            .expect("workspace exists");
        let CanonicalResource::WorkspaceDocument(document) = canonical else {
            panic!("workspace document")
        };
        assert_eq!(document.revision.get(), 2);
        assert_eq!(document.payload.views[0].overrides.hidden_points, vec![sun]);
    }

    #[test]
    fn aspect_preview_cancel_and_save_reuse_calculation_value() {
        let repository = MemoryRepository::default();
        let backend = ControlledBackend::new();
        let calls = Rc::clone(&backend.calls);
        let application = demo_backend_application(repository, backend);
        let initial = ready(&application);
        assert_eq!(calls.get(), 1);
        let original_scene = initial.active_view.unwrap().scene.expect("initial Scene");
        let standard = demo_ids().aspect_set_standard;

        let begin = block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: standard,
        }))
        .expect("begin succeeds");
        assert!(matches!(editor_state(&begin), DraftState::Clean { .. }));
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
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
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
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
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
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
        let application = demo_application(repository.clone());
        ready(&application);
        let standard = demo_ids().aspect_set_standard;
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: standard,
        }))
        .expect("begin succeeds");
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(9.0),
            },
        )))
        .expect("draft update succeeds");
        block_on(application.wait_for_update(dirty.version)).expect("preview settles");

        let CanonicalResource::AspectSet(remote_one) = block_on(repository.get(standard))
            .expect("repository read succeeds")
            .expect("Aspect Set exists")
        else {
            panic!("demo resource is an Aspect Set");
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
        let application = demo_application(repository);
        ready(&application);
        let standard = demo_ids().aspect_set_standard;
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: standard,
        }))
        .expect("begin succeeds");
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
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
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
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
        let application = demo_application(repository);
        ready(&application);
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: demo_ids().aspect_set_standard,
        }))
        .expect("begin succeeds");
        let dirty = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
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
        let application = demo_application(MemoryRepository::default());
        ready(&application);
        block_on(application.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: demo_ids().aspect_set_standard,
        }))
        .expect("begin succeeds");
        let refreshing = block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
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
    fn asynchronous_worker_result_advances_all_projection_waiters_without_double_receive() {
        let runtime = ControlledCalculationRuntime::new();
        let application = Rc::new(demo_runtime_application(
            MemoryRepository::default(),
            runtime.clone(),
        ));
        let loading = block_on(application.initialize()).expect("initialization succeeds");
        let after = loading.version;
        let request = runtime
            .submitted()
            .last()
            .cloned()
            .expect("initial calculation submitted");

        let result_a = Rc::new(RefCell::new(None));
        let result_b = Rc::new(RefCell::new(None));
        let mut pool = LocalPool::new();
        for result in [&result_a, &result_b] {
            let application = Rc::clone(&application);
            let result = Rc::clone(result);
            pool.spawner()
                .spawn_local(async move {
                    result.replace(Some(application.wait_for_update(after).await));
                })
                .expect("waiter task spawns");
        }

        pool.run_until_stalled();
        assert_eq!(runtime.receive_calls(), 1);
        assert!(result_a.borrow().is_none());
        assert!(result_b.borrow().is_none());

        runtime.complete_success(&request, 0.0);
        pool.run_until_stalled();

        let waiter_a = result_a
            .borrow_mut()
            .take()
            .expect("waiter A completed")
            .expect("waiter A observes the update");
        let waiter_b = result_b
            .borrow_mut()
            .take()
            .expect("waiter B completed")
            .expect("waiter B observes the update");
        assert!(waiter_a.version > after);
        assert_eq!(waiter_a.version, waiter_b.version);
        assert_eq!(runtime.receive_calls(), 1);
    }

    #[test]
    fn registered_waiters_are_broadcast_a_later_dispatch_transition() {
        let application = demo_application(MemoryRepository::default());
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
        let backend = ControlledBackend::new();
        let fail_next = Rc::clone(&backend.fail_next);
        let application = demo_backend_application(repository, backend);
        let initial = ready(&application);
        let original = initial.active_view.unwrap().scene.expect("initial Scene");
        let opened = block_on(application.dispatch(AppIntent::OpenChart {
            definition_id: demo_ids().chart_definition_b,
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
    #[allow(clippy::similar_names)]
    fn latest_request_wins_when_controlled_runtime_completes_successes_out_of_order() {
        let runtime = ControlledCalculationRuntime::new();
        let application = demo_runtime_application(MemoryRepository::default(), runtime.clone());
        let initial = controlled_ready(&application, &runtime);
        let scene_zero = initial
            .active_view
            .as_ref()
            .and_then(|view| view.scene.clone())
            .expect("initial Scene");

        application.state.borrow_mut().cache.clear();
        let request_a_state = block_on(application.dispatch(AppIntent::RefreshActiveView))
            .expect("request A accepted");
        assert_eq!(
            request_a_state
                .active_view
                .as_ref()
                .and_then(|view| view.scene.clone()),
            Some(scene_zero.clone())
        );
        assert!(matches!(
            request_a_state
                .active_view
                .as_ref()
                .map(|view| &view.computation),
            Some(ViewComputationState::Refreshing)
        ));

        application.state.borrow_mut().cache.clear();
        let request_b_state = block_on(application.dispatch(AppIntent::RefreshActiveView))
            .expect("request B accepted while A is running");
        assert_eq!(
            request_b_state
                .active_view
                .as_ref()
                .and_then(|view| view.scene.clone()),
            Some(scene_zero.clone())
        );
        let requests = runtime.submitted();
        let request_a = requests[requests.len() - 2].clone();
        let request_b = requests[requests.len() - 1].clone();
        assert!(request_b.request_id > request_a.request_id);

        runtime.complete_success(&request_b, 20.0);
        let fresh_b = block_on(application.wait_for_update(request_b_state.version))
            .expect("request B completes");
        let scene_b = fresh_b
            .active_view
            .as_ref()
            .and_then(|view| view.scene.clone())
            .expect("Scene B");
        assert_ne!(scene_b, scene_zero);
        assert!(matches!(
            fresh_b.active_view.as_ref().map(|view| &view.computation),
            Some(ViewComputationState::Fresh)
        ));

        runtime.complete_success(&request_a, 10.0);
        block_on(application.complete_next_pending(fresh_b.version)).expect("stale A is processed");
        let after_stale = block_on(application.snapshot()).expect("snapshot after stale A");
        assert_eq!(after_stale.version, fresh_b.version);
        assert_eq!(
            after_stale
                .active_view
                .as_ref()
                .and_then(|view| view.scene.clone()),
            Some(scene_b)
        );
        assert!(matches!(
            after_stale.active_view.map(|view| view.computation),
            Some(ViewComputationState::Fresh)
        ));
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn stale_failure_cannot_overwrite_newer_success() {
        let runtime = ControlledCalculationRuntime::new();
        let application = demo_runtime_application(MemoryRepository::default(), runtime.clone());
        let initial = controlled_ready(&application, &runtime);
        let scene_zero = initial
            .active_view
            .and_then(|view| view.scene)
            .expect("Scene S0");

        application.state.borrow_mut().cache.clear();
        let request_c_state = block_on(application.dispatch(AppIntent::RefreshActiveView))
            .expect("request C accepted");
        application.state.borrow_mut().cache.clear();
        let request_d_state = block_on(application.dispatch(AppIntent::RefreshActiveView))
            .expect("request D accepted");
        let requests = runtime.submitted();
        let request_c = requests[requests.len() - 2].clone();
        let request_d = requests[requests.len() - 1].clone();

        runtime.complete_success(&request_d, 25.0);
        let fresh_d = block_on(application.wait_for_update(request_d_state.version))
            .expect("request D completes");
        let scene_d = fresh_d
            .active_view
            .as_ref()
            .and_then(|view| view.scene.clone())
            .expect("Scene D");
        assert_ne!(scene_d, scene_zero);

        runtime.complete_failure(&request_c, "stale C failure");
        block_on(application.complete_next_pending(fresh_d.version)).expect("stale C is processed");
        let after_stale = block_on(application.snapshot()).expect("snapshot after stale failure");
        assert_eq!(after_stale.version, fresh_d.version);
        assert_eq!(
            after_stale
                .active_view
                .as_ref()
                .and_then(|view| view.scene.clone()),
            Some(scene_d)
        );
        assert!(matches!(
            after_stale.active_view.map(|view| view.computation),
            Some(ViewComputationState::Fresh)
        ));
        assert!(request_c_state.version < request_d_state.version);
    }

    #[test]
    fn current_request_with_calc_key_mismatch_is_rejected_as_integrity_failure() {
        let runtime = ControlledCalculationRuntime::new();
        let application = demo_runtime_application(MemoryRepository::default(), runtime.clone());
        let initial = controlled_ready(&application, &runtime);
        let last_good = initial
            .active_view
            .and_then(|view| view.scene)
            .expect("last good Scene");

        application.state.borrow_mut().cache.clear();
        let refreshing =
            block_on(application.dispatch(AppIntent::RefreshActiveView)).expect("refresh accepted");
        let request = runtime
            .submitted()
            .last()
            .cloned()
            .expect("request submitted");
        let mut wrong_request = request.request.clone();
        wrong_request.celestial.corrections.aberration =
            !wrong_request.celestial.corrections.aberration;
        let wrong_key = CalcKey::derive(
            &wrong_request,
            application.engine.calculation_engine_identity(),
            &application.engine.backend_descriptor().fingerprint,
        )
        .expect("wrong key");
        assert_ne!(wrong_key, request.calc_key);
        runtime.complete_calc_key_mismatch(&request, wrong_key);

        let failed = block_on(application.wait_for_update(refreshing.version))
            .expect("integrity failure is projected");
        let view = failed.active_view.expect("active view");
        assert_eq!(view.scene, Some(last_good));
        assert!(matches!(
            view.computation,
            ViewComputationState::Failed(AppError {
                kind: AppErrorKind::ViewComputation,
                ref message,
            }) if message.contains("CalcKey")
        ));
    }

    #[test]
    fn memory_repository_reload_restores_saved_document_not_session_navigation() {
        let repository = MemoryRepository::default();
        let first = demo_application(repository.clone());
        ready(&first);
        let ids = demo_ids();
        let opened = block_on(first.dispatch(AppIntent::OpenChart {
            definition_id: ids.chart_definition_b,
        }))
        .expect("chart B opens");
        assert!(opened.workspace.active_chart.is_some());
        block_on(first.dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: ids.aspect_set_standard,
        }))
        .expect("edit begins");
        let dirty = block_on(first.dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
                maximum: angle(6.5),
            },
        )))
        .expect("draft updates");
        block_on(first.wait_for_update(dirty.version)).expect("preview settles");
        let saving = block_on(first.dispatch(AppIntent::SaveDraft)).expect("save starts");
        block_on(first.wait_for_update(saving.version)).expect("save settles");
        block_on(first.dispatch(AppIntent::SaveWorkspace)).expect("workspace saves explicitly");
        drop(first);

        let second = demo_application(repository);
        let restored = ready(&second);
        assert_eq!(restored.workspace.active_chart, Some(ids.chart_instance_a));
        assert!(restored.workspace.selected_charts.is_empty());
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
        let demo = demo_application(repository.clone());
        ready(&demo);
        let ids = demo_ids();

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

        let CanonicalResource::WorkspaceDocument(workspace_one) =
            block_on(repository.get(ids.workspace))
                .expect("repository read succeeds")
                .expect("WorkspaceDocument exists")
        else {
            panic!("demo resource is a WorkspaceDocument");
        };
        let mut pinned_payload = workspace_one.payload.clone();
        pinned_payload.profile.aspects = ResourceBinding::Pinned {
            id: ids.aspect_set_standard,
            revision: Revision::INITIAL,
        };
        let workspace_two = workspace_one
            .next_with_payload(pinned_payload, Timestamp::from_unix_millis(21))
            .expect("second WorkspaceDocument revision");
        block_on(repository.save(
            workspace_one.revision,
            CanonicalResource::WorkspaceDocument(workspace_two),
        ))
        .expect("WorkspaceDocument pin saves");
        drop(demo);

        let application = demo_application(repository);
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
