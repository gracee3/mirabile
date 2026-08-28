use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
};

#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

use async_trait::async_trait;
use futures::{channel::oneshot, lock::Mutex};
use mirabile_core::{
    AnalysisProfile, AspectSet, CalculationSpec, CanonicalResource, ChartDefinition, ChartRecord,
    ChartSource, Command, ConfigurationLayer, ConfigurationStack, DerivationSpec, DomainValidate,
    EffectiveConfiguration, InstanceId, PointSet, Resolved, ResourceBinding, ResourceEnvelope,
    ResourceId, ResourceKind, Revision, Theme, Timestamp, ValueSource, ViewDocument, ViewInstance,
    ViewInstanceId, WheelTemplate, WorkspaceDocument, WorkspaceDocumentChart, resolve_binding,
};
use mirabile_engine::{
    AnalysisKey, AspectAnalyzer, CalcKey, CalculationEngine, CalculationOutcome,
    CalculationRequestId, CalculationWorkerFailure, CalculationWorkerFailureCategory,
    CalculationWorkerRequest, CalculationWorkerResult, ComputationCache, DeterministicBackend,
    ImplementationIdentity, PreparedCalculation, Scene, SnapshotContext, WorkerProtocolVersion,
    layout_wheel, render_key,
};
#[cfg(target_arch = "wasm32")]
use mirabile_store::ResourceTombstone;
use mirabile_store::{
    AtomicSaveBatch, MemoryRepository, RepositoryError, ResourceRepository, ResourceState,
    RevisionExpectation,
};

use crate::{
    ActiveChartInspector, AppAction, AppError, AppErrorKind, AppIntent, AppNotice, AppNoticeKind,
    AppReadModel, AppResult, Application, ApplicationActivityReadModel, ApplicationStatus,
    AspectDraftValue, AspectSetDraftMutation, AspectSetDraftReadModel, AspectSetSummary,
    AuthoringCapabilitiesReadModel, Availability, BindingSourceSummary,
    CalculationDiagnosticsReadModel, CalculationRuntime, CalculationRuntimeError, ChartEditorState,
    ChartMutation, ChartPersistence, ChartSlotAssignment, ChartSlotOption, CommandCapability,
    DisplayValueSource, DraftState, ImplementationIdentityReadModel, InlineCalculationRuntime,
    InspectorReadModel, LibraryChartSummary, LibraryReadModel, OpenChartSummary,
    PendingOperationReadModel, PointVisibilityReadModel, ProjectionVersion,
    RepositoryHeadReadModel, RepositoryHeadState, RepositoryReadModel, RepositoryRevisionReadModel,
    RepositoryRevisionState, ResourceBindingSummary, ResourceCatalogReadModel,
    ResourceEditorReadModel, ResourceInventoryReadModel, ResourceSummaryReadModel,
    SlotAssignmentSource, StartupCalculationProfile, StartupPolicy, ViewComputationState,
    ViewDisplayReadModel, ViewReadModel, ViewSummary, WorkspaceDocumentBacking, WorkspaceReadModel,
    WorkspaceSession, WorkspaceSwitchDecisionReadModel, WorkspaceSwitchTarget,
    blank_workspace_session, current_transits_session, current_unix_millis,
    workspace_commands::apply_workspace_command,
};
#[cfg(feature = "xalen-backend")]
use mirabile_engine::XalenBackend;

mod binding_editing;
mod calculation;
mod catalog;
mod configuration;
mod deletion;
mod editing;
mod hydration;
mod projection;
mod resource_editing;
mod state;
#[cfg(test)]
mod tests;
mod validation;
mod workspace;

use catalog::{BoundPayload, Catalog, resolve_typed_binding};

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

    async fn create_batch(&self, resources: Vec<CanonicalResource>) -> Result<(), RepositoryError> {
        self.acquire().await?.create_batch(resources).await
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

    async fn save_batch(
        &self,
        batch: mirabile_store::AtomicSaveBatch,
    ) -> Result<(), RepositoryError> {
        self.acquire().await?.save_batch(batch).await
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

    async fn list_heads(
        &self,
        kind: Option<mirabile_core::ResourceKind>,
    ) -> Result<Vec<ResourceState>, RepositoryError> {
        self.acquire().await?.list_heads(kind).await
    }

    async fn list_revisions(&self, id: ResourceId) -> Result<Vec<ResourceState>, RepositoryError> {
        self.acquire().await?.list_revisions(id).await
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
                state.chart_editor = None;
                state.pending.clear();
                state.inflight.clear();
                state.saving_chart_drafts.clear();
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
                state.saving_chart_drafts.clear();
                state.notice = None;
                state.advance()?;
            }
        }
        self.read_model()
    }

    #[allow(clippy::too_many_lines)]
    async fn dispatch(&self, intent: AppIntent) -> AppResult<AppReadModel> {
        if !matches!(self.state.borrow().status, ApplicationStatus::Ready) {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "The application must be ready before it can accept intents",
            ));
        }
        if self.state.borrow().has_pending_write() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Wait for the pending repository operation to finish",
            ));
        }
        if !self.state.borrow().saving_chart_drafts.is_empty() {
            return Err(AppError::new(
                AppErrorKind::Unavailable,
                "Wait for the pending chart save to finish",
            ));
        }

        match intent {
            AppIntent::BeginNewChart => self.begin_new_chart()?,
            AppIntent::BeginSavedChartEdit { instance_id } => {
                self.begin_saved_chart_edit(instance_id)?;
            }
            AppIntent::ApplyChartMutation(mutation) => self.apply_chart_mutation(mutation)?,
            AppIntent::SaveChartEditor => self.begin_save_chart_editor()?,
            AppIntent::CancelChartEditor => self.cancel_chart_editor()?,
            AppIntent::StartChartDraft { draft } => self.start_chart_draft(*draft)?,
            AppIntent::SaveChartDraft { instance_id } => {
                self.begin_save_chart_draft(instance_id)?;
            }
            AppIntent::CancelChartDraft { instance_id } => {
                self.cancel_chart_draft(instance_id)?;
            }
            AppIntent::OpenChart { .. }
            | AppIntent::CloseChart { .. }
            | AppIntent::AssignChartSlot { .. }
            | AppIntent::SetWorkspaceAspectSet { .. } => {
                self.dispatch_workspace_intent(&intent)?;
            }
            AppIntent::SetWorkspaceBinding { slot, selection } => {
                self.set_workspace_binding(slot, selection)?;
            }
            AppIntent::ApplyWorkspaceComposition(mutation) => {
                self.apply_workspace_composition(mutation)?;
            }
            AppIntent::ActivateChart { instance_id } => {
                self.activate_session_chart(instance_id)?;
            }
            AppIntent::SetChartSelection {
                instance_id,
                selected,
            } => self.set_session_chart_selection(instance_id, selected)?,
            AppIntent::SetActiveView { view_id } => self.set_active_session_view(view_id)?,
            AppIntent::NewWorkspace => {
                self.request_workspace_switch(WorkspaceSwitchTarget::New)?;
            }
            AppIntent::OpenWorkspace { resource_id } => {
                self.request_workspace_switch(WorkspaceSwitchTarget::Saved { resource_id })?;
            }
            AppIntent::RenameWorkspace { title } => self.rename_workspace(&title)?,
            AppIntent::DiscardWorkspaceChanges => self.discard_workspace_changes()?,
            AppIntent::ResolveWorkspaceSwitch { action } => {
                self.resolve_workspace_switch(action)?;
            }
            AppIntent::LoadDemoBundle => self.begin_load_demo_bundle()?,
            AppIntent::SaveWorkspace => self.begin_save_workspace()?,
            AppIntent::SetTemporaryPointHidden { point_id, hidden } => {
                self.set_temporary_point_hidden(point_id, hidden)?;
            }
            AppIntent::PromoteTemporaryDisplay => self.promote_temporary_display()?,
            AppIntent::BeginAspectSetEdit { resource_id } => {
                self.begin_aspect_set_edit(resource_id)?;
            }
            AppIntent::BeginNewAspectSet => self.begin_new_aspect_set()?,
            AppIntent::DuplicateAspectSet { resource_id } => {
                self.duplicate_aspect_set(resource_id)?;
            }
            AppIntent::SelectRepositoryResource { resource_id } => {
                self.select_repository_resource(resource_id).await?;
            }
            AppIntent::BeginDeleteResource {
                resource_id,
                expected_revision,
            } => self.begin_delete_resource(resource_id, expected_revision)?,
            AppIntent::ConfirmDeleteResource {
                resource_id,
                expected_revision,
            } => {
                self.confirm_delete_resource(resource_id, expected_revision)
                    .await?;
            }
            AppIntent::BeginResourceEdit { resource_id } => {
                self.begin_resource_edit(resource_id)?;
            }
            AppIntent::BeginResourceCreate { kind } => self.begin_resource_create(kind)?,
            AppIntent::ApplyResourceMutation(mutation) => {
                self.apply_resource_mutation(*mutation)?;
            }
            AppIntent::SaveResourceDraft { kind } => self.begin_save_resource_draft(kind)?,
            AppIntent::CancelResourceDraft { kind } => self.cancel_resource_draft(kind)?,
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
                    drop(state);
                    return self.read_model();
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

struct RealState {
    version: ProjectionVersion,
    status: ApplicationStatus,
    catalog: Catalog,
    workspace: Option<ResourceEnvelope<WorkspaceDocument>>,
    session: Option<WorkspaceSession>,
    views: BTreeMap<ViewInstanceId, ViewRuntime>,
    editor: Option<AspectSetEditor>,
    chart_editor: Option<crate::ChartAuthoringEditor>,
    repository_selection: Option<RepositorySelection>,
    delete_confirmation: Option<(ResourceId, Revision)>,
    resource_drafts: BTreeMap<crate::ResourceDraftKind, resource_editing::GenericResourceDraft>,
    workspace_switch: Option<WorkspaceSwitchDecisionReadModel>,
    pending_workspace_switch: Option<WorkspaceSwitchTarget>,
    cache: ComputationCache,
    pending: VecDeque<PendingWork>,
    inflight: BTreeMap<CalculationRequestId, PendingViewCalculation>,
    saving_chart_drafts: BTreeSet<InstanceId>,
    next_request_id: CalculationRequestId,
    waiters: Vec<oneshot::Sender<()>>,
    notice: Option<AppNotice>,
    next_timestamp: i64,
}

struct HydratedState {
    catalog: Catalog,
    workspace: Option<ResourceEnvelope<WorkspaceDocument>>,
    session: WorkspaceSession,
    next_timestamp: i64,
}

struct RepositorySelection {
    resource_id: ResourceId,
    history: Vec<ResourceState>,
}

#[derive(Clone)]
struct ViewRuntime {
    scene: Option<Scene>,
    semantic_calculation: Option<mirabile_engine::CalculationValue>,
    semantic_analysis: Option<mirabile_engine::ChartAnalysis>,
    computation: ViewComputationState,
    expected: Option<ExpectedCalculation>,
    last_expected: Option<ExpectedCalculation>,
}

impl Default for ViewRuntime {
    fn default() -> Self {
        Self {
            scene: None,
            semantic_calculation: None,
            semantic_analysis: None,
            computation: ViewComputationState::Loading,
            expected: None,
            last_expected: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedCalculation {
    request_id: CalculationRequestId,
    calc_key: CalcKey,
    analysis_key: AnalysisKey,
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
    base: Option<ResourceEnvelope<AspectSet>>,
    title: String,
    draft: AspectSet,
    state: DraftState,
}

enum PendingWork {
    CompleteCachedView(Box<PendingCachedView>),
    SaveAspectSet {
        expected_revision: Option<Revision>,
        next: ResourceEnvelope<AspectSet>,
    },
    SaveTypedResource {
        kind: crate::ResourceDraftKind,
        expected_revision: Option<Revision>,
        next: Box<CanonicalResource>,
    },
    CreateChart {
        instance_id: InstanceId,
        record: Box<ResourceEnvelope<ChartRecord>>,
        definition: Box<ResourceEnvelope<ChartDefinition>>,
    },
    SaveChartEdit {
        instance_id: InstanceId,
        definition_id: ResourceId,
        batch: AtomicSaveBatch,
    },
    SaveWorkspace {
        expected_revision: Option<Revision>,
        next: Box<ResourceEnvelope<WorkspaceDocument>>,
    },
    LoadDemoBundle {
        resources: Vec<CanonicalResource>,
    },
}

struct PendingCachedView {
    view_id: ViewInstanceId,
    expected: ExpectedCalculation,
    prepared: PreparedCalculation,
    plan: ViewCalculationPlan,
    calculation: mirabile_engine::CalculationValue,
}

fn binding_summary<T: BoundPayload>(
    slot: crate::WorkspaceBindingSlot,
    label: &str,
    binding: &ResourceBinding<T>,
    catalog: &Catalog,
) -> AppResult<ResourceBindingSummary> {
    let resolved = resolve_typed_binding(binding, catalog, ConfigurationLayer::Workspace)
        .map_err(|error| AppError::new(AppErrorKind::NotFound, error.to_string()))?;
    let source = match resolved.source {
        ValueSource::Inline => BindingSourceSummary::Inline,
        ValueSource::Follow {
            resource_id,
            revision,
        } => BindingSourceSummary::Follow {
            resource_id,
            resource_title: resource_title(catalog, resource_id, Some(revision))?,
            revision,
        },
        ValueSource::Pinned {
            resource_id,
            revision,
        } => BindingSourceSummary::Pinned {
            resource_id,
            resource_title: resource_title(catalog, resource_id, Some(revision))?,
            revision,
        },
    };
    Ok(ResourceBindingSummary {
        slot,
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
    editor.draft.domain_validate().map_err(|error| {
        AppError::new(
            AppErrorKind::InvalidIntent,
            format!("Aspect Set draft was invalid: {error}"),
        )
    })?;
    Ok(AspectSetDraftReadModel {
        resource_id: editor.base.as_ref().map(|base| base.id),
        title: editor.title.clone(),
        state: editor.state.clone(),
        aspects: editor
            .draft
            .aspects
            .iter()
            .map(|aspect| AspectDraftValue {
                aspect_id: aspect.id.clone(),
                label: aspect.name.clone(),
                angle: aspect.angle,
                enabled: aspect.enabled,
                maximum_orb: aspect.orbs.maximum,
                applying_multiplier: aspect.orbs.applying_multiplier,
                classification: aspect.classification,
            })
            .collect(),
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
    if let Some(editor) = state.editor.as_mut().filter(|editor| {
        editor
            .base
            .as_ref()
            .is_some_and(|base| base.id == resource_id)
    }) {
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
        RepositoryError::Conflict { .. } | RepositoryError::BatchConflict { .. } => {
            AppErrorKind::Conflict
        }
        RepositoryError::NotFound(_) | RepositoryError::ResourceDeleted(_) => {
            AppErrorKind::NotFound
        }
        RepositoryError::AlreadyExists(_)
        | RepositoryError::EmptyCreateBatch
        | RepositoryError::EmptySaveBatch
        | RepositoryError::DuplicateBatchIdentity(_)
        | RepositoryError::MissingBatchExpectation(_)
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
