use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use futures::{
    executor::{LocalPool, block_on},
    task::LocalSpawnExt,
};
use mirabile_core::{
    Angle, CanonicalResource, CoordinateSystem, EventKind, HouseSystem, Latitude, Longitude,
    PointId, PointSelector, PointSet, ResourceEnvelope, ResourceKind,
};
use mirabile_engine::{
    BackendDescriptor, CalculationBackend, CalculationBackendError,
    CalculationBackendErrorCategory, CalculationBackendResult, ResolvedCalculationRequest,
};
use mirabile_store::ResourceTombstone;

use crate::{
    PointSetMutation, ResourceDraftKind, ResourceMetadataMutation, ResourceMutation,
    TypedResourceDraftReadModel, demo_ids, demo_resources,
};

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
        let mut result =
            mirabile_engine::execute_calculation_request(&DeterministicBackend, request.clone());
        if let CalculationOutcome::Success(calculation) = &mut result.outcome {
            let sun = PointId::new("sun").expect("point ID");
            if let Some(position) = calculation.celestial.positions.get_mut(&sun) {
                position.longitude = Angle::normalized(position.longitude.degrees() + sun_shift)
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
        let mut result =
            mirabile_engine::execute_calculation_request(&DeterministicBackend, request.clone());
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
    fail_next_create_batch: Rc<Cell<bool>>,
}

impl SaveFailureRepository {
    fn new(save_failure: InjectedSaveFailure) -> Self {
        Self {
            inner: MemoryRepository::default(),
            save_failure: Rc::new(Cell::new(save_failure)),
            fail_next_get: Rc::new(Cell::new(false)),
            fail_next_create_batch: Rc::new(Cell::new(false)),
        }
    }

    fn with_atomic_create_failure() -> Self {
        let repository = Self::new(InjectedSaveFailure::None);
        repository.fail_next_create_batch.set(true);
        repository
    }
}

#[async_trait(?Send)]
impl ResourceRepository for SaveFailureRepository {
    async fn create(&self, resource: CanonicalResource) -> Result<(), RepositoryError> {
        self.inner.create(resource).await
    }

    async fn create_batch(&self, resources: Vec<CanonicalResource>) -> Result<(), RepositoryError> {
        if self.fail_next_create_batch.replace(false) {
            return Err(RepositoryError::Adapter(
                "injected atomic chart create failure".into(),
            ));
        }
        self.inner.create_batch(resources).await
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

    async fn save_batch(
        &self,
        batch: mirabile_store::AtomicSaveBatch,
    ) -> Result<(), RepositoryError> {
        self.inner.save_batch(batch).await
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

    async fn list_heads(
        &self,
        kind: Option<ResourceKind>,
    ) -> Result<Vec<ResourceState>, RepositoryError> {
        self.inner.list_heads(kind).await
    }

    async fn list_revisions(&self, id: ResourceId) -> Result<Vec<ResourceState>, RepositoryError> {
        self.inner.list_revisions(id).await
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
    settle(application, loading)
}

#[test]
fn initialization_projects_all_canonical_inventories_and_repository_heads() {
    let repository = MemoryRepository::default();
    for resource in demo_resources() {
        block_on(repository.create(resource)).expect("seed demo resource");
    }
    let deleted = CanonicalResource::PointSet(ResourceEnvelope::new(
        "Deleted points",
        PointSet { points: Vec::new() },
        Timestamp::from_unix_millis(2),
    ));
    let deleted_id = deleted.id();
    block_on(repository.create(deleted)).expect("seed deletable resource");
    let tombstone = block_on(repository.delete(
        deleted_id,
        Revision::INITIAL,
        Timestamp::from_unix_millis(3),
    ))
    .expect("delete resource");

    let application = RealApplication::with_repository(repository);
    let model = ready(&application);
    assert_eq!(model.resources.inventories.len(), 10);
    assert_eq!(
        model
            .resources
            .inventories
            .iter()
            .map(|inventory| inventory.kind)
            .collect::<Vec<_>>(),
        CanonicalResource::KINDS
    );
    assert!(
        model
            .resources
            .inventories
            .iter()
            .all(|inventory| !inventory.label.is_empty())
    );
    assert_eq!(
        model
            .resources
            .inventories
            .iter()
            .map(|inventory| inventory.resources.len())
            .sum::<usize>(),
        demo_resources().len()
    );
    assert!(model.repository.heads.iter().any(|head| {
        head.resource_id == deleted_id
            && head.revision == tombstone.revision
            && matches!(
                &head.state,
                RepositoryHeadState::Deleted { deleted_at }
                    if *deleted_at == tombstone.deleted_at
            )
    }));
    assert!(
        model
            .resources
            .inventories
            .iter()
            .flat_map(|inventory| &inventory.resources)
            .all(|resource| resource.resource_id != deleted_id)
    );

    let selected = block_on(application.dispatch(AppIntent::SelectRepositoryResource {
        resource_id: deleted_id,
    }))
    .expect("select deleted resource history");
    assert_eq!(selected.repository.selected_resource, Some(deleted_id));
    assert_eq!(selected.repository.selected_history.len(), 2);
    assert_eq!(
        selected.repository.selected_history[0].revision,
        Revision::INITIAL
    );
    assert!(matches!(
        selected.repository.selected_history[1].state,
        RepositoryRevisionState::Deleted { deleted_at }
            if deleted_at == tombstone.deleted_at
    ));
}

#[test]
fn typed_resource_draft_saves_conflicts_cancels_and_reloads() {
    let repository = MemoryRepository::default();
    let original = CanonicalResource::PointSet(ResourceEnvelope::new(
        "Original points",
        PointSet {
            points: vec![PointSelector::Point(PointId::new("sun").expect("point"))],
        },
        Timestamp::from_unix_millis(1),
    ));
    let resource_id = original.id();
    block_on(repository.create(original.clone())).expect("seed point set");
    let application = RealApplication::with_repository(repository.clone());
    let model = ready(&application);

    let opened = block_on(application.dispatch(AppIntent::BeginResourceEdit { resource_id }))
        .expect("open typed draft");
    assert!(opened.version > model.version);
    assert!(matches!(
        opened.resource_editor.drafts.as_slice(),
        [TypedResourceDraftReadModel {
            kind: ResourceDraftKind::PointSet,
            state: DraftState::Clean {
                revision: Revision::INITIAL
            },
            ..
        }]
    ));
    let dirty = block_on(
        application.dispatch(AppIntent::ApplyResourceMutation(Box::new(
            ResourceMutation::PointSet(PointSetMutation::Metadata(
                ResourceMetadataMutation::SetTitle("Edited points".into()),
            )),
        ))),
    )
    .expect("mutate typed draft");
    assert!(matches!(
        dirty.resource_editor.drafts[0].state,
        DraftState::Dirty {
            base_revision: Revision::INITIAL
        }
    ));
    assert_eq!(
        block_on(repository.get(resource_id))
            .expect("repository read")
            .expect("resource")
            .title(),
        "Original points"
    );

    let saving = block_on(application.dispatch(AppIntent::SaveResourceDraft {
        kind: ResourceDraftKind::PointSet,
    }))
    .expect("begin typed save");
    assert!(!saving.is_settled());
    let saved = settle(&application, saving);
    assert_eq!(saved.resource_editor.drafts[0].title, "Edited points");
    assert!(matches!(
        saved.resource_editor.drafts[0].state,
        DraftState::Clean { revision } if revision.get() == 2
    ));

    let reloaded = ready(&RealApplication::with_repository(repository.clone()));
    let inventory = reloaded
        .resources
        .inventories
        .iter()
        .find(|inventory| inventory.kind == ResourceKind::PointSet)
        .expect("point inventory");
    assert_eq!(inventory.resources[0].title, "Edited points");
    assert_eq!(inventory.resources[0].revision.get(), 2);

    block_on(
        application.dispatch(AppIntent::ApplyResourceMutation(Box::new(
            ResourceMutation::PointSet(PointSetMutation::Metadata(
                ResourceMetadataMutation::SetTitle("Local conflict".into()),
            )),
        ))),
    )
    .expect("local conflicting edit");
    let remote_head = block_on(repository.get(resource_id))
        .expect("remote read")
        .expect("remote head");
    let mut remote_next = remote_head
        .next_revision(Timestamp::from_unix_millis(5))
        .expect("remote revision");
    remote_next.set_title("Remote conflict".into());
    block_on(repository.save(remote_head.revision(), remote_next)).expect("remote save");
    let saving = block_on(application.dispatch(AppIntent::SaveResourceDraft {
        kind: ResourceDraftKind::PointSet,
    }))
    .expect("begin stale save");
    let conflicted = settle(&application, saving);
    assert!(matches!(
        conflicted.resource_editor.drafts[0].state,
        DraftState::Conflict {
            base_revision,
            remote_revision
        } if base_revision.get() == 2 && remote_revision.get() == 3
    ));
    assert_eq!(conflicted.resource_editor.drafts[0].title, "Local conflict");

    let canceled = block_on(application.dispatch(AppIntent::CancelResourceDraft {
        kind: ResourceDraftKind::PointSet,
    }))
    .expect("cancel conflicted draft");
    assert!(canceled.resource_editor.drafts.is_empty());
}

#[test]
fn typed_resource_creation_publishes_each_independent_canonical_payload() {
    let repository = MemoryRepository::default();
    let application = RealApplication::with_repository(repository.clone());
    let mut model = ready(&application);
    let independent = [
        ResourceDraftKind::PointSet,
        ResourceDraftKind::AnalysisProfile,
        ResourceDraftKind::WheelTemplate,
        ResourceDraftKind::ViewDocument,
        ResourceDraftKind::Theme,
        ResourceDraftKind::QueryDefinition,
    ];
    for kind in independent {
        model = block_on(application.dispatch(AppIntent::BeginResourceCreate { kind }))
            .expect("begin typed create");
        let draft = model
            .resource_editor
            .drafts
            .iter()
            .find(|draft| draft.kind == kind)
            .expect("created draft projection");
        assert_eq!(draft.state, DraftState::New);
        let creating = block_on(application.dispatch(AppIntent::SaveResourceDraft { kind }))
            .expect("begin create save");
        assert!(matches!(
            creating
                .resource_editor
                .drafts
                .iter()
                .find(|draft| draft.kind == kind)
                .expect("creating draft")
                .state,
            DraftState::Creating
        ));
        model = settle(&application, creating);
    }
    assert_eq!(repository.current_count(), independent.len());
    for kind in independent {
        let inventory = model
            .resources
            .inventories
            .iter()
            .find(|inventory| inventory.kind == kind.resource_kind())
            .expect("inventory group");
        assert_eq!(inventory.resources.len(), 1);
        assert_eq!(inventory.resources[0].revision, Revision::INITIAL);
    }
}

fn settle<R, C>(application: &RealApplication<R, C>, mut model: AppReadModel) -> AppReadModel
where
    R: ResourceRepository + Clone,
    C: CalculationRuntime,
{
    while !model.is_settled() {
        model = block_on(application.wait_for_update(model.version))
            .expect("authoritative application work settles");
    }
    model
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

fn aspect_row<'a>(
    draft: &'a crate::AspectSetDraftReadModel,
    aspect_id: &str,
) -> &'a crate::AspectDraftValue {
    draft
        .aspects
        .iter()
        .find(|aspect| aspect.aspect_id.as_str() == aspect_id)
        .expect("Aspect Set row is projected")
}

fn point_visibility<'a>(
    model: &'a AppReadModel,
    point_id: &str,
) -> &'a crate::PointVisibilityReadModel {
    model
        .active_view
        .as_ref()
        .expect("active view")
        .display
        .points
        .iter()
        .find(|point| point.point_id.as_str() == point_id)
        .expect("supported point visibility is projected")
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
    let CanonicalResource::WorkspaceDocument(workspace) = block_on(repository.get(workspace_id))
        .expect("workspace read succeeds")
        .expect("workspace exists")
    else {
        panic!("demo resource is a WorkspaceDocument");
    };
    let ResourceBinding::Inline { value: document } = &workspace.payload.views[0].document else {
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
    let closed =
        block_on(application.wait_for_update(refreshing.version)).expect("close refresh settles");
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
        immediate.active_view.as_ref().map(|view| &view.computation),
        Some(ViewComputationState::Loading)
    ));
    let loading_diagnostics = immediate
        .calculation
        .as_ref()
        .expect("provider-neutral diagnostics are projected");
    assert_eq!(loading_diagnostics.backend.id, DeterministicBackend::ID);
    assert_eq!(loading_diagnostics.worker_protocol, 3);
    assert!(loading_diagnostics.active_request_id.is_some());
    assert!(loading_diagnostics.calc_key.is_some());
    assert!(loading_diagnostics.analysis_key.is_some());
    assert!(!loading_diagnostics.last_good_scene_present);
    let fresh = block_on(first.wait_for_update(loading.version)).expect("view settles");
    assert_eq!(fresh.version, ProjectionVersion::new(2));
    assert!(matches!(
        fresh.active_view.as_ref().map(|view| &view.computation),
        Some(ViewComputationState::Fresh)
    ));
    let fresh_diagnostics = fresh.calculation.as_ref().expect("diagnostics remain");
    assert_eq!(fresh_diagnostics.active_request_id, None);
    assert_eq!(
        fresh_diagnostics.calc_key, loading_diagnostics.calc_key,
        "the last completed calculation identity remains inspectable"
    );
    assert!(fresh_diagnostics.last_good_scene_present);

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

#[test]
fn fresh_unsaved_session_creates_workspace_revision_one_without_persisting_draft_assignments() {
    let repository = MemoryRepository::default();
    let application = RealApplication::with_repository(repository.clone());
    let projection = ready(&application);
    let draft_id = projection
        .workspace
        .active_chart
        .expect("Current Transits draft");

    assert!(
        projection
            .availability(AppAction::SaveWorkspace)
            .is_enabled(),
        "an unsaved session is first-saveable even before a durable mutation"
    );
    let saving = block_on(application.dispatch(AppIntent::SaveWorkspace))
        .expect("first workspace save is accepted");
    assert!(!saving.is_settled());
    let saved = settle(&application, saving);
    let workspace_id = saved
        .workspace
        .document_id
        .expect("first save assigns canonical identity");
    assert_eq!(
        saved.workspace.document_revision.map(Revision::get),
        Some(1)
    );
    assert!(!saved.workspace.document_dirty);
    assert_eq!(repository.current_count(), 1);

    let CanonicalResource::WorkspaceDocument(document) = block_on(repository.get(workspace_id))
        .expect("workspace read")
        .expect("workspace exists")
    else {
        panic!("first save creates a WorkspaceDocument")
    };
    assert!(document.payload.chart_instances.is_empty());
    assert!(
        document
            .payload
            .views
            .iter()
            .all(|view| !view.charts.values().any(|chart| *chart == draft_id))
    );
    drop(application);

    let reloaded = RealApplication::with_repository_and_policy(
        repository,
        StartupPolicy::OpenWorkspace(workspace_id),
    );
    let restored = block_on(reloaded.initialize()).expect("saved empty workspace initializes");
    assert_eq!(restored.status, ApplicationStatus::Ready);
    assert_eq!(restored.workspace.document_id, Some(workspace_id));
    assert!(restored.workspace.charts.is_empty());
    assert!(restored.active_view.is_some_and(|view| {
        view.slots.iter().all(|slot| slot.chart.is_none())
            && matches!(view.computation, ViewComputationState::Failed(_))
    }));
}

#[test]
fn fresh_unsaved_session_can_open_and_assign_saved_library_chart_before_first_save() {
    let repository = MemoryRepository::default();
    ensure_demo(&repository);
    let application = RealApplication::with_repository_and_policy(
        repository.clone(),
        StartupPolicy::CurrentTransits,
    );
    let initial = ready(&application);
    let view_id = initial
        .workspace
        .active_view
        .expect("Current Transits view");
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
    let opened = block_on(application.dispatch(AppIntent::OpenChart {
        definition_id: demo_ids().chart_definition_b,
    }))
    .expect("saved library chart opens in an unsaved session");
    let saved_instance = opened
        .workspace
        .active_chart
        .expect("saved chart is active");
    assert_eq!(opened.workspace.document_id, None);
    assert!(opened.workspace.document_dirty);

    let assigning = block_on(application.dispatch(AppIntent::AssignChartSlot {
        view_id,
        slot: required_slot,
        chart: Some(saved_instance),
    }))
    .expect("saved chart can replace a draft slot overlay");
    let assigned = block_on(application.wait_for_update(assigning.version))
        .expect("saved chart preview settles");
    assert!(assigned.active_view.is_some_and(|view| {
        view.slots
            .iter()
            .any(|slot| slot.required && slot.chart == Some(saved_instance))
    }));

    let saving = block_on(application.dispatch(AppIntent::SaveWorkspace))
        .expect("unsaved session accepts its first WorkspaceDocument save");
    let saved = settle(&application, saving);
    let workspace_id = saved.workspace.document_id.expect("new workspace identity");
    let CanonicalResource::WorkspaceDocument(document) = block_on(repository.get(workspace_id))
        .expect("workspace read")
        .expect("workspace exists")
    else {
        panic!("saved resource is a WorkspaceDocument")
    };
    assert_eq!(document.revision, Revision::INITIAL);
    assert_eq!(document.payload.chart_instances.len(), 1);
    assert!(
        document.payload.views[0]
            .charts
            .values()
            .all(|chart| *chart == saved_instance)
    );
    assert_eq!(
        block_on(repository.list(Some(ResourceKind::WorkspaceDocument)))
            .expect("workspace list")
            .len(),
        2,
        "the demo workspace and the newly saved session remain distinct"
    );
}

#[test]
fn new_workspace_projects_switch_decisions_and_requires_explicit_discard() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository);
    let initial = ready(&application);
    let ids = demo_ids();
    assert_eq!(initial.workspace.title, "Mirabile Workspace");
    assert_eq!(initial.library.workspaces.len(), 1);

    let creating = block_on(application.dispatch(AppIntent::NewWorkspace))
        .expect("clean saved workspace switches directly to new");
    let created = settle(&application, creating);
    assert_eq!(created.workspace.title, "Untitled Workspace");
    assert!(created.workspace.document_id.is_none());
    assert_eq!(created.workspace.views.len(), 1);
    assert!(created.workspace.charts.iter().any(|chart| {
        chart.title == "Current Transits" && chart.persistence == ChartPersistence::Ephemeral
    }));

    let requested = block_on(application.dispatch(AppIntent::OpenWorkspace {
        resource_id: ids.workspace,
    }))
    .expect("unsafe switch projects a decision");
    let decision = requested
        .workspace
        .switch_decision
        .as_ref()
        .expect("switch decision");
    assert!(!decision.save_and_switch_enabled);
    assert!(
        decision
            .reasons
            .iter()
            .any(|reason| reason.contains("draft"))
    );
    assert!(requested.workspace.document_id.is_none());

    let stayed = block_on(application.dispatch(AppIntent::ResolveWorkspaceSwitch {
        action: crate::WorkspaceSwitchAction::Stay,
    }))
    .expect("stay is explicit");
    assert!(stayed.workspace.switch_decision.is_none());
    block_on(application.dispatch(AppIntent::OpenWorkspace {
        resource_id: ids.workspace,
    }))
    .expect("switch can be requested again");
    let switching = block_on(application.dispatch(AppIntent::ResolveWorkspaceSwitch {
        action: crate::WorkspaceSwitchAction::DiscardAndSwitch,
    }))
    .expect("discard and switch is explicit");
    let switched = settle(&application, switching);
    assert_eq!(switched.workspace.document_id, Some(ids.workspace));
    assert_eq!(switched.workspace.title, "Mirabile Workspace");
}

#[test]
fn save_and_switch_publishes_working_title_before_opening_target() {
    let repository = MemoryRepository::default();
    ensure_demo(&repository);
    let ids = demo_ids();
    let second_id = ResourceId::new();
    let CanonicalResource::WorkspaceDocument(first) = block_on(repository.get(ids.workspace))
        .expect("workspace read")
        .expect("workspace exists")
    else {
        panic!("workspace resource")
    };
    let second = ResourceEnvelope::with_id(
        second_id,
        "Second Workspace",
        first.payload.clone(),
        Timestamp::from_unix_millis(2),
    );
    block_on(repository.create(CanonicalResource::WorkspaceDocument(second)))
        .expect("second workspace");
    let application = RealApplication::with_repository_and_policy(
        repository.clone(),
        StartupPolicy::OpenWorkspace(ids.workspace),
    );
    ready(&application);
    let renamed = block_on(application.dispatch(AppIntent::RenameWorkspace {
        title: "Renamed First Workspace".into(),
    }))
    .expect("working title changes");
    assert!(renamed.workspace.document_dirty);
    let requested = block_on(application.dispatch(AppIntent::OpenWorkspace {
        resource_id: second_id,
    }))
    .expect("dirty switch decision");
    assert!(
        requested
            .workspace
            .switch_decision
            .as_ref()
            .expect("decision")
            .save_and_switch_enabled
    );
    let saving = block_on(application.dispatch(AppIntent::ResolveWorkspaceSwitch {
        action: crate::WorkspaceSwitchAction::SaveAndSwitch,
    }))
    .expect("save and switch begins");
    assert!(!saving.is_settled());
    let switched = settle(&application, saving);
    assert_eq!(switched.workspace.document_id, Some(second_id));
    assert_eq!(switched.workspace.title, "Second Workspace");
    let first_saved = block_on(repository.get(ids.workspace))
        .expect("first read")
        .expect("first exists");
    assert_eq!(first_saved.title(), "Renamed First Workspace");
    assert_eq!(first_saved.revision().get(), 2);
}

#[test]
fn save_and_switch_rechecks_new_editor_blockers_before_persisting() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    ready(&application);
    block_on(application.dispatch(AppIntent::RenameWorkspace {
        title: "Dirty before switch".into(),
    }))
    .expect("rename");
    let requested =
        block_on(application.dispatch(AppIntent::NewWorkspace)).expect("switch decision");
    assert!(
        requested
            .workspace
            .switch_decision
            .expect("decision")
            .save_and_switch_enabled
    );
    let editor = block_on(application.dispatch(AppIntent::BeginNewChart))
        .expect("chart work may begin while decision is visible");
    let editor = settle(&application, editor);
    assert!(
        !editor
            .workspace
            .switch_decision
            .expect("recomputed decision")
            .save_and_switch_enabled
    );
    assert!(matches!(
        block_on(application.dispatch(AppIntent::ResolveWorkspaceSwitch {
            action: crate::WorkspaceSwitchAction::SaveAndSwitch,
        })),
        Err(AppError {
            kind: AppErrorKind::Unavailable,
            ..
        })
    ));
    assert_eq!(repository.revision_count(), 7);
}

#[test]
fn discard_workspace_restores_saved_envelope_title() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    ready(&application);
    block_on(application.dispatch(AppIntent::RenameWorkspace {
        title: "Transient title".into(),
    }))
    .expect("rename");
    let discarding = block_on(application.dispatch(AppIntent::DiscardWorkspaceChanges))
        .expect("discard is explicit");
    let discarded = settle(&application, discarding);
    assert_eq!(discarded.workspace.title, "Mirabile Workspace");
    assert!(!discarded.workspace.document_dirty);
    assert_eq!(repository.revision_count(), 7);
}

#[test]
fn demo_bundle_load_is_explicit_idempotent_and_atomic() {
    let repository = MemoryRepository::default();
    let application = RealApplication::with_repository(repository.clone());
    let initial = ready(&application);
    assert!(initial.library.workspaces.is_empty());
    assert_eq!(repository.current_count(), 0);

    let loading = block_on(application.dispatch(AppIntent::LoadDemoBundle))
        .expect("explicit demo load begins");
    assert_eq!(
        loading.activity.pending_operations,
        vec![PendingOperationReadModel::DemoLoading]
    );
    let loaded = settle(&application, loading);
    assert_eq!(repository.current_count(), 7);
    assert_eq!(repository.revision_count(), 7);
    assert_eq!(loaded.library.workspaces.len(), 1);
    let again =
        block_on(application.dispatch(AppIntent::LoadDemoBundle)).expect("idempotent load begins");
    let again = settle(&application, again);
    assert_eq!(repository.current_count(), 7);
    assert_eq!(repository.revision_count(), 7);
    assert!(
        again
            .notice
            .as_ref()
            .is_some_and(|notice| notice.message.contains("already present"))
    );
}

#[test]
fn demo_bundle_rejects_incompatible_stable_identity_without_partial_creation() {
    let repository = MemoryRepository::default();
    let collision = CanonicalResource::PointSet(ResourceEnvelope::with_id(
        demo_ids().workspace,
        "Incompatible collision",
        PointSet {
            points: vec![PointSelector::Point(PointId::new("sun").expect("point ID"))],
        },
        Timestamp::from_unix_millis(1),
    ));
    block_on(repository.create(collision)).expect("collision fixture");
    let application = RealApplication::with_repository(repository.clone());
    ready(&application);
    let loading = block_on(application.dispatch(AppIntent::LoadDemoBundle))
        .expect("load begins before async collision inspection");
    let rejected = settle(&application, loading);
    assert_eq!(repository.current_count(), 1);
    assert_eq!(repository.revision_count(), 1);
    assert!(rejected.notice.as_ref().is_some_and(|notice| {
        notice.kind == AppNoticeKind::Warning && notice.message.contains("incompatible")
    }));
}

#[test]
fn closing_last_saved_chart_repairs_required_slot_with_draft_session_overlay() {
    let repository = MemoryRepository::default();
    ensure_demo(&repository);
    let application =
        RealApplication::with_repository_and_policy(repository, StartupPolicy::CurrentTransits);
    let initial = ready(&application);
    let draft_id = initial
        .workspace
        .active_chart
        .expect("Current Transits draft");
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
    let opened = block_on(application.dispatch(AppIntent::OpenChart {
        definition_id: demo_ids().chart_definition_b,
    }))
    .expect("saved chart opens");
    let saved_id = opened
        .workspace
        .active_chart
        .expect("saved chart is active");
    block_on(application.dispatch(AppIntent::AssignChartSlot {
        view_id,
        slot: required_slot.clone(),
        chart: Some(saved_id),
    }))
    .expect("saved assignment replaces draft overlay");

    let closing = block_on(application.dispatch(AppIntent::CloseChart {
        instance_id: saved_id,
    }))
    .expect("last saved chart closes while draft remains");
    assert_eq!(closing.workspace.active_chart, Some(draft_id));
    assert!(closing.active_view.is_some_and(|view| {
        view.slots
            .iter()
            .any(|slot| slot.slot == required_slot && slot.chart == Some(draft_id))
    }));
    let state = application.state.borrow();
    let session = state.session.as_ref().expect("session");
    assert!(session.document.chart_instances.is_empty());
    assert!(session.document.views[0].charts.is_empty());
    assert_eq!(
        session.effective_chart_assignment(view_id, &required_slot),
        Some(draft_id)
    );
}

#[test]
fn draft_slot_overlay_never_enters_saved_workspace_and_reload_restores_durable_assignment() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    let initial = ready(&application);
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
    let mut fixture =
        current_transits_session(946_728_000_000, StartupCalculationProfile::Baseline);
    let draft = fixture
        .draft_charts
        .pop()
        .expect("Current Transits provides a draft")
        .draft;
    let started = block_on(application.dispatch(AppIntent::StartChartDraft {
        draft: Box::new(draft),
    }))
    .expect("draft starts");
    let draft_id = started.workspace.active_chart.expect("draft is active");
    let previewing = block_on(application.dispatch(AppIntent::AssignChartSlot {
        view_id,
        slot: required_slot.clone(),
        chart: Some(draft_id),
    }))
    .expect("draft receives session-side slot overlay");
    assert!(
        !previewing.workspace.document_dirty,
        "draft assignment alone does not change the durable document"
    );
    let preview =
        block_on(application.wait_for_update(previewing.version)).expect("draft preview settles");
    assert!(preview.active_view.as_ref().is_some_and(|view| {
        view.slots
            .iter()
            .any(|slot| slot.slot == required_slot && slot.chart == Some(draft_id))
    }));

    let dirty = block_on(application.dispatch(AppIntent::SetWorkspaceAspectSet {
        resource_id: demo_ids().aspect_set_tight,
    }))
    .expect("unrelated durable mutation succeeds while draft overlay is active");
    block_on(application.wait_for_update(dirty.version)).expect("preview settles");
    let saving = block_on(application.dispatch(AppIntent::SaveWorkspace))
        .expect("workspace save is accepted without serializing the draft overlay");
    settle(&application, saving);

    let CanonicalResource::WorkspaceDocument(document) =
        block_on(repository.get(demo_ids().workspace))
            .expect("workspace read")
            .expect("workspace exists")
    else {
        panic!("workspace resource")
    };
    assert_eq!(document.revision.get(), 2);
    assert_eq!(
        document.payload.views[0].charts.get(&required_slot),
        Some(&demo_ids().chart_instance_a)
    );
    assert!(
        !document.payload.views[0]
            .charts
            .values()
            .any(|chart| *chart == draft_id)
    );
    drop(application);

    let reloaded = demo_application(repository);
    let restored = ready(&reloaded);
    assert!(restored.active_view.is_some_and(|view| {
        view.slots.iter().any(|slot| {
            slot.slot == required_slot && slot.chart == Some(demo_ids().chart_instance_a)
        })
    }));
}

#[test]
fn pre_save_validation_rejects_unknown_chart_instance_in_durable_view() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    let initial = ready(&application);
    let view_id = initial.workspace.active_view.expect("active view");
    let required_slot = initial
        .active_view
        .expect("active view")
        .slots
        .into_iter()
        .find(|slot| slot.required)
        .expect("required slot")
        .slot;
    let unknown = InstanceId::new();
    {
        let mut state = application.state.borrow_mut();
        let session = state.session.as_mut().expect("session");
        session
            .document
            .views
            .iter_mut()
            .find(|view| view.id == view_id)
            .expect("view")
            .charts
            .insert(required_slot, unknown);
        session.mark_document_dirty();
    }

    let error = block_on(application.dispatch(AppIntent::SaveWorkspace))
        .expect_err("unknown durable chart reference is rejected before repository save");
    assert_eq!(error.kind, AppErrorKind::InvalidIntent);
    assert!(error.message.contains("durable references are invalid"));
    let CanonicalResource::WorkspaceDocument(document) =
        block_on(repository.get(demo_ids().workspace))
            .expect("workspace read")
            .expect("workspace exists")
    else {
        panic!("workspace resource")
    };
    assert_eq!(document.revision, Revision::INITIAL);
}

#[test]
fn initialization_separates_structural_from_catalog_referential_validation() {
    let ids = demo_ids();
    let repository = MemoryRepository::default();
    for resource in demo_resources()
        .into_iter()
        .filter(|resource| resource.id() != ids.chart_record_a)
    {
        block_on(repository.create(resource)).expect("structurally valid resource is accepted");
    }
    let application = RealApplication::with_repository_and_policy(
        repository,
        StartupPolicy::OpenWorkspace(ids.workspace),
    );

    let projection = block_on(application.initialize()).expect("error state is projected");
    assert!(matches!(
        projection.status,
        ApplicationStatus::Error(AppError {
            kind: AppErrorKind::Initialization,
            ref message,
        }) if message.contains("referential validation")
            && message.contains("ChartRecord")
            && message.contains(&ids.chart_record_a.to_string())
    ));
}

#[test]
fn resolved_required_view_slots_are_referential_application_invariants() {
    let ids = demo_ids();
    let repository = MemoryRepository::default();
    for resource in demo_resources() {
        let resource = match resource {
            CanonicalResource::WorkspaceDocument(mut workspace) => {
                workspace.payload.views[0].charts.clear();
                workspace
                    .payload
                    .domain_validate()
                    .expect("one-object structural validation does not resolve slot requirements");
                CanonicalResource::WorkspaceDocument(workspace)
            }
            resource => resource,
        };
        block_on(repository.create(resource)).expect("structurally valid resource is accepted");
    }
    let application = RealApplication::with_repository_and_policy(
        repository,
        StartupPolicy::OpenWorkspace(ids.workspace),
    );

    let projection = block_on(application.initialize()).expect("error state is projected");
    assert!(matches!(
        projection.status,
        ApplicationStatus::Error(AppError {
            kind: AppErrorKind::Initialization,
            ref message,
        }) if message.contains("referential validation")
            && message.contains("required slot is not assigned")
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn chart_draft_previews_then_atomically_creates_record_and_definition() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    let initial = ready(&application);
    let view_id = initial.workspace.active_view.expect("active view");
    let required_slot = initial
        .active_view
        .as_ref()
        .expect("active view projection")
        .slots
        .iter()
        .find(|slot| slot.required)
        .expect("required slot")
        .slot
        .clone();
    let mut fixture =
        current_transits_session(946_728_000_000, StartupCalculationProfile::Baseline);
    let draft = fixture
        .draft_charts
        .pop()
        .expect("current transits provides a draft")
        .draft;

    let started = block_on(application.dispatch(AppIntent::StartChartDraft {
        draft: Box::new(draft),
    }))
    .expect("draft starts");
    let instance_id = started.workspace.active_chart.expect("draft is active");
    assert_eq!(repository.current_count(), 7);
    assert!(matches!(
        started
            .workspace
            .charts
            .last()
            .map(|chart| &chart.persistence),
        Some(ChartPersistence::Ephemeral)
    ));
    assert!(started.availability(AppAction::SaveChartDraft).is_enabled());

    let previewing = block_on(application.dispatch(AppIntent::AssignChartSlot {
        view_id,
        slot: required_slot.clone(),
        chart: Some(instance_id),
    }))
    .expect("draft can be assigned for preview");
    let preview = block_on(application.wait_for_update(previewing.version))
        .expect("draft preview calculation settles");
    assert!(preview.active_view.as_ref().is_some_and(|view| {
        view.scene.is_some() && view.computation == ViewComputationState::Fresh
    }));
    let preview_slot = preview
        .active_view
        .as_ref()
        .and_then(|view| view.slots.iter().find(|slot| slot.slot == required_slot))
        .expect("draft slot projection");
    assert_eq!(preview_slot.draft_chart, Some(instance_id));
    assert!(matches!(
        preview_slot.source,
        SlotAssignmentSource::Draft {
            instance_id: draft_id,
            promotion: crate::DraftAssignmentPromotion::RequiresChartSave,
        } if draft_id == instance_id
    ));
    assert!(preview_slot.options.iter().any(|option| {
        option.chart == Some(instance_id)
            && option.persistence == Some(ChartPersistence::Ephemeral)
            && option.enabled
    }));
    assert!(preview_slot.options.iter().any(|option| {
        option.chart.is_none()
            && !option.enabled
            && option.disabled_reason.as_deref() == Some("This slot requires a chart")
    }));
    {
        let state = application.state.borrow();
        let session = state.session.as_ref().expect("session");
        assert_ne!(
            session.document.views[0].charts.get(&required_slot),
            Some(&instance_id),
            "draft preview does not mutate the durable document"
        );
        assert_eq!(
            session.effective_chart_assignment(view_id, &required_slot),
            Some(instance_id)
        );
    }

    let saving = block_on(application.dispatch(AppIntent::SaveChartDraft { instance_id }))
        .expect("atomic chart create is accepted");
    assert!(!saving.is_settled());
    let saved = settle(&application, saving);
    let saved_chart = saved
        .workspace
        .charts
        .iter()
        .find(|chart| chart.instance_id == instance_id)
        .expect("saved chart remains in the session");
    let ChartPersistence::Saved { definition_id } = saved_chart.persistence else {
        panic!("saved draft projects a canonical definition")
    };
    let saved_slot = saved
        .active_view
        .as_ref()
        .and_then(|view| view.slots.iter().find(|slot| slot.slot == required_slot))
        .expect("saved slot projection");
    assert_eq!(saved_slot.draft_chart, None);
    assert_eq!(saved_slot.durable_chart, Some(instance_id));
    assert!(matches!(
        saved_slot.source,
        SlotAssignmentSource::Saved {
            instance_id: saved_id,
            definition_id: saved_definition,
        } if saved_id == instance_id && saved_definition == definition_id
    ));
    assert!(saved.workspace.document_dirty);
    {
        let state = application.state.borrow();
        let session = state.session.as_ref().expect("session");
        assert_eq!(
            session.document.views[0].charts.get(&required_slot),
            Some(&instance_id),
            "saving the draft promotes its slot assignment"
        );
        assert!(session.draft_chart_assignments.is_empty());
    }
    assert_eq!(repository.current_count(), 9);
    assert_eq!(repository.revision_count(), 9);
    let CanonicalResource::ChartDefinition(definition) = block_on(repository.get(definition_id))
        .expect("definition read")
        .expect("definition exists")
    else {
        panic!("saved identity is a ChartDefinition")
    };
    let ChartSource::Radix { record } = definition.payload.source else {
        panic!("saved draft is a radix definition")
    };
    assert!(matches!(
        block_on(repository.get(record)).expect("record read"),
        Some(CanonicalResource::ChartRecord(_))
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn typed_new_chart_authoring_retains_last_valid_preview_and_saves_atomically() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    let initial = ready(&application);
    let initial_count = repository.current_count();
    let initial_scene = initial
        .active_view
        .as_ref()
        .and_then(|view| view.scene.clone());

    let started = block_on(application.dispatch(AppIntent::BeginNewChart))
        .expect("typed chart editor begins");
    let instance_id = started.workspace.active_chart.expect("new chart is active");
    let editor = started.chart_editor.as_ref().expect("editor projection");
    assert_eq!(editor.fields.title, "Untitled Chart");
    assert_eq!(editor.fields.event_kind, EventKind::Birth);
    assert_eq!(editor.fields.houses, HouseSystem::NoHouses);
    assert_eq!(editor.fields.coordinates, CoordinateSystem::Geocentric);
    assert!(editor.validation.is_empty());
    assert_eq!(repository.current_count(), initial_count);
    let preview = settle(&application, started);
    assert!(preview.active_view.as_ref().is_some_and(|view| {
        view.slots
            .iter()
            .any(|slot| slot.required && slot.chart == Some(instance_id))
    }));

    let incomplete = block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetLocationEnabled(true),
    )))
    .expect("incomplete location is accepted as editor state");
    assert!(incomplete.is_settled());
    assert_eq!(
        incomplete
            .chart_editor
            .as_ref()
            .expect("editor")
            .validation
            .len(),
        3
    );
    assert_eq!(
        incomplete
            .active_view
            .as_ref()
            .and_then(|view| view.scene.clone()),
        preview
            .active_view
            .as_ref()
            .and_then(|view| view.scene.clone())
            .or(initial_scene),
        "incomplete fields retain the last valid Scene"
    );
    assert!(
        !incomplete
            .availability(AppAction::SaveChartEditor)
            .is_enabled()
    );

    block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetLocationName("Baltimore".into()),
    )))
    .expect("location name");
    block_on(
        application.dispatch(AppIntent::ApplyChartMutation(ChartMutation::SetLatitude(
            Some(Latitude::from_degrees(39.29).expect("latitude")),
        ))),
    )
    .expect("latitude");
    let complete = block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetLongitude(Some(Longitude::from_degrees(-76.61).expect("longitude"))),
    )))
    .expect("complete location refreshes preview");
    let complete = settle(&application, complete);
    assert!(
        complete
            .chart_editor
            .as_ref()
            .expect("editor")
            .validation
            .is_empty()
    );
    assert!(
        complete
            .authoring
            .house_systems
            .iter()
            .any(|option| { option.value == HouseSystem::Equal && option.enabled })
    );

    let houses = block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetHouseSystem(HouseSystem::Equal),
    )))
    .expect("location-backed Equal houses are supported");
    settle(&application, houses);
    let saving = block_on(application.dispatch(AppIntent::SaveChartEditor))
        .expect("typed editor save is accepted");
    assert!(!saving.is_settled());
    assert_eq!(
        saving.chart_editor.as_ref().map(|editor| editor.state),
        Some(ChartEditorState::Saving)
    );
    let saved = settle(&application, saving);
    assert!(saved.chart_editor.is_none());
    assert_eq!(repository.current_count(), initial_count + 2);
    assert!(matches!(
        saved
            .workspace
            .charts
            .iter()
            .find(|chart| chart.instance_id == instance_id)
            .map(|chart| &chart.persistence),
        Some(ChartPersistence::Saved { .. })
    ));
}

#[test]
fn typed_new_chart_cancel_writes_nothing() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    ready(&application);
    let initial_count = repository.current_count();
    let started = block_on(application.dispatch(AppIntent::BeginNewChart)).expect("begin chart");
    let instance_id = started.workspace.active_chart.expect("new chart");
    settle(&application, started);
    let canceled =
        block_on(application.dispatch(AppIntent::CancelChartEditor)).expect("cancel chart editor");
    assert!(canceled.chart_editor.is_none());
    assert!(
        canceled
            .workspace
            .charts
            .iter()
            .all(|chart| chart.instance_id != instance_id)
    );
    assert_eq!(repository.current_count(), initial_count);
}

#[test]
fn saved_chart_definition_only_edit_checks_record_without_revising_it() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    let initial = ready(&application);
    let ids = demo_ids();
    let initial_revision_count = repository.revision_count();

    let opened = block_on(application.dispatch(AppIntent::BeginSavedChartEdit {
        instance_id: ids.chart_instance_a,
    }))
    .expect("saved editor opens");
    let editor = opened
        .chart_editor
        .as_ref()
        .expect("saved editor projection");
    assert!(matches!(
        editor.target,
        crate::ChartEditorTarget::Saved {
            record_id,
            definition_id,
            record_base_revision: Revision::INITIAL,
            definition_base_revision: Revision::INITIAL,
            ..
        } if record_id == ids.chart_record_a && definition_id == ids.chart_definition_a
    ));
    settle(&application, opened);
    let changed = block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetTitle("Definition-only title".into()),
    )))
    .expect("title mutation");
    settle(&application, changed);
    let saving = block_on(application.dispatch(AppIntent::SaveChartEditor))
        .expect("saved edit begins observable save");
    assert!(matches!(
        saving.activity.pending_operations.as_slice(),
        [PendingOperationReadModel::ChartSave { definition_id }]
            if *definition_id == ids.chart_definition_a
    ));
    let saved = settle(&application, saving);
    assert!(saved.chart_editor.is_none());
    assert_eq!(repository.revision_count(), initial_revision_count + 1);
    let record = block_on(repository.get(ids.chart_record_a))
        .expect("record read")
        .expect("record exists");
    assert_eq!(record.revision(), Revision::INITIAL);
    assert!(
        block_on(
            repository.get_revision(ids.chart_record_a, Revision::new(2).expect("revision two"))
        )
        .expect("history read")
        .is_none()
    );
    let definition = block_on(repository.get(ids.chart_definition_a))
        .expect("definition read")
        .expect("definition exists");
    assert_eq!(definition.revision().get(), 2);
    assert_eq!(definition.title(), "Definition-only title");
    assert_eq!(
        initial.workspace.document_revision, saved.workspace.document_revision,
        "editing chart resources does not invent a workspace revision"
    );
}

#[test]
fn saved_chart_cancel_restores_canonical_preview_without_writes() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    ready(&application);
    let ids = demo_ids();
    let revision_count = repository.revision_count();
    let opened = block_on(application.dispatch(AppIntent::BeginSavedChartEdit {
        instance_id: ids.chart_instance_a,
    }))
    .expect("saved editor opens");
    settle(&application, opened);
    let changed = block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetTitle("Canceled local title".into()),
    )))
    .expect("local edit");
    settle(&application, changed);

    let canceling =
        block_on(application.dispatch(AppIntent::CancelChartEditor)).expect("saved edit cancels");
    let canceled = settle(&application, canceling);
    assert!(canceled.chart_editor.is_none());
    assert_eq!(repository.revision_count(), revision_count);
    assert_eq!(
        block_on(repository.get(ids.chart_definition_a))
            .expect("definition read")
            .expect("definition exists")
            .title(),
        "Example Natal"
    );
}

#[test]
fn saved_chart_batch_reports_both_component_conflicts_and_retains_local_editor() {
    let repository = MemoryRepository::default();
    let first = demo_application(repository.clone());
    let second = demo_application(repository.clone());
    ready(&first);
    ready(&second);
    let ids = demo_ids();

    for application in [&first, &second] {
        let opened = block_on(application.dispatch(AppIntent::BeginSavedChartEdit {
            instance_id: ids.chart_instance_a,
        }))
        .expect("saved editor opens");
        settle(application, opened);
    }
    let first_title = block_on(first.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetTitle("First local title".into()),
    )))
    .expect("first title");
    settle(&first, first_title);
    let first_record = block_on(first.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetSubjectName(Some("First local subject".into())),
    )))
    .expect("first factual edit");
    settle(&first, first_record);

    let second_title = block_on(second.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetTitle("Second remote title".into()),
    )))
    .expect("second title");
    settle(&second, second_title);
    let second_record = block_on(second.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetSubjectName(Some("Second remote subject".into())),
    )))
    .expect("second factual edit");
    settle(&second, second_record);
    let second_save =
        block_on(second.dispatch(AppIntent::SaveChartEditor)).expect("second save begins");
    settle(&second, second_save);

    let first_save =
        block_on(first.dispatch(AppIntent::SaveChartEditor)).expect("first stale save begins");
    let conflicted = settle(&first, first_save);
    let editor = conflicted
        .chart_editor
        .as_ref()
        .expect("local editor retained");
    assert_eq!(editor.state, ChartEditorState::Conflict);
    assert_eq!(editor.fields.title, "First local title");
    assert_eq!(editor.conflicts.len(), 2);
    assert!(
        !conflicted
            .availability(AppAction::SaveChartEditor)
            .is_enabled()
    );
    assert!(editor.conflicts.iter().any(|conflict| {
        conflict.component == crate::ChartConflictComponent::Record
            && conflict.resource_id == ids.chart_record_a
    }));
    assert!(editor.conflicts.iter().any(|conflict| {
        conflict.component == crate::ChartConflictComponent::Definition
            && conflict.resource_id == ids.chart_definition_a
    }));
    assert_eq!(
        first
            .state
            .borrow()
            .catalog
            .chart_definition(ids.chart_definition_a)
            .expect("refreshed definition")
            .title,
        "Second remote title"
    );
    let canceling = block_on(first.dispatch(AppIntent::CancelChartEditor))
        .expect("conflicted editor remains cancelable");
    let reopened_head = settle(&first, canceling);
    assert!(reopened_head.chart_editor.is_none());
    assert_eq!(
        reopened_head
            .inspector
            .active_chart
            .expect("active chart")
            .title,
        "Second remote title"
    );
}

#[test]
fn shared_chart_record_blocks_factual_edits_but_allows_definition_edits() {
    let repository = MemoryRepository::default();
    ensure_demo(&repository);
    let ids = demo_ids();
    let shared_definition_id = ResourceId::new();
    block_on(repository.create(CanonicalResource::ChartDefinition(
        ResourceEnvelope::with_id(
            shared_definition_id,
            "Alternate definition",
            ChartDefinition {
                source: ChartSource::Radix {
                    record: ids.chart_record_a,
                },
                calculation: CalculationSpec::default(),
            },
            Timestamp::from_unix_millis(2),
        ),
    )))
    .expect("shared definition fixture");
    let application = RealApplication::with_repository_and_policy(
        repository,
        StartupPolicy::OpenWorkspace(ids.workspace),
    );
    ready(&application);
    let opened = block_on(application.dispatch(AppIntent::BeginSavedChartEdit {
        instance_id: ids.chart_instance_a,
    }))
    .expect("shared record editor opens");
    let editor = opened.chart_editor.as_ref().expect("editor");
    assert!(!editor.factual_mutations_enabled);
    assert!(editor.factual_mutations_disabled_reason.is_some());
    settle(&application, opened);
    let factual = block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetSubjectName(Some("Blocked".into())),
    )));
    assert!(matches!(
        factual,
        Err(AppError {
            kind: AppErrorKind::Unavailable,
            ref message,
        }) if message.contains("shared") && message.contains("copy/detach")
    ));
    let definition_only = block_on(application.dispatch(AppIntent::ApplyChartMutation(
        ChartMutation::SetTitle("Allowed definition title".into()),
    )))
    .expect("definition-only edit remains allowed");
    assert_eq!(
        definition_only
            .chart_editor
            .as_ref()
            .expect("editor")
            .fields
            .title,
        "Allowed definition title"
    );
}

#[test]
fn failed_atomic_chart_save_retains_draft_and_cancel_creates_nothing() {
    let repository = SaveFailureRepository::with_atomic_create_failure();
    let application = RealApplication::with_repository(repository.clone());
    let ready = ready(&application);
    let instance_id = ready
        .workspace
        .active_chart
        .expect("current transits draft");

    let saving = block_on(application.dispatch(AppIntent::SaveChartDraft { instance_id }))
        .expect("failed atomic save is first accepted as observable work");
    let retained = settle(&application, saving);
    let notice = retained
        .notice
        .as_ref()
        .expect("failure notice is projected");
    assert_eq!(notice.kind, AppNoticeKind::Warning);
    assert!(notice.message.contains("Could not atomically save"));
    assert!(matches!(
        retained
            .workspace
            .charts
            .first()
            .map(|chart| &chart.persistence),
        Some(ChartPersistence::Ephemeral)
    ));
    assert_eq!(repository.inner.current_count(), 0);

    let canceled = block_on(application.dispatch(AppIntent::CancelChartDraft { instance_id }))
        .expect("retained draft can be canceled");
    assert!(canceled.workspace.charts.is_empty());
    assert_eq!(repository.inner.current_count(), 0);
    assert_eq!(repository.inner.revision_count(), 0);
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
    let tight =
        block_on(application.wait_for_update(tight.version)).expect("Aspect Set refresh settles");
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

    let saving =
        block_on(application.dispatch(AppIntent::SaveWorkspace)).expect("workspace save starts");
    let saved = settle(&application, saving);
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
    let unsupported = block_on(application.dispatch(AppIntent::SetTemporaryPointHidden {
        point_id: PointId::new("pluto").expect("point ID"),
        hidden: true,
    }))
    .expect_err("unsupported point is rejected");
    assert_eq!(unsupported.kind, AppErrorKind::InvalidIntent);

    let temporary = block_on(application.dispatch(AppIntent::SetTemporaryPointHidden {
        point_id: sun.clone(),
        hidden: true,
    }))
    .expect("temporary override succeeds");
    assert!(!temporary.workspace.document_dirty);
    assert!(temporary.workspace.has_temporary_display_override);
    assert!(!point_visibility(&temporary, "sun").visible);
    assert!(point_visibility(&temporary, "sun").durable_visible);
    assert_eq!(
        point_visibility(&temporary, "sun").temporary_visible,
        Some(false)
    );
    block_on(application.wait_for_update(temporary.version)).expect("temporary preview settles");
    let canonical = block_on(repository.get(ids.workspace))
        .expect("workspace read")
        .expect("workspace exists");
    assert_eq!(canonical.revision(), Revision::INITIAL);

    let promoted = block_on(application.dispatch(AppIntent::PromoteTemporaryDisplay))
        .expect("promotion succeeds");
    assert!(promoted.workspace.document_dirty);
    assert!(!promoted.workspace.has_temporary_display_override);
    assert!(!point_visibility(&promoted, "sun").visible);
    assert!(!point_visibility(&promoted, "sun").durable_visible);
    assert_eq!(point_visibility(&promoted, "sun").temporary_visible, None);
    block_on(application.wait_for_update(promoted.version)).expect("promoted preview settles");

    let saving =
        block_on(application.dispatch(AppIntent::SaveWorkspace)).expect("workspace save starts");
    let saved = settle(&application, saving);
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
fn temporary_display_is_a_complete_replacement_that_can_unhide_durable_points() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    ready(&application);
    let sun = PointId::new("sun").expect("point ID");

    let hidden = block_on(application.dispatch(AppIntent::SetTemporaryPointHidden {
        point_id: sun.clone(),
        hidden: true,
    }))
    .expect("temporary hide");
    settle(&application, hidden);
    let promoted =
        block_on(application.dispatch(AppIntent::PromoteTemporaryDisplay)).expect("hide promotes");
    settle(&application, promoted);
    assert!(
        !point_visibility(&block_on(application.snapshot()).expect("snapshot"), "sun")
            .durable_visible
    );

    let visible = block_on(application.dispatch(AppIntent::SetTemporaryPointHidden {
        point_id: sun,
        hidden: false,
    }))
    .expect("temporary unhide");
    assert!(point_visibility(&visible, "sun").visible);
    assert!(!point_visibility(&visible, "sun").durable_visible);
    assert_eq!(
        point_visibility(&visible, "sun").temporary_visible,
        Some(true)
    );
    settle(&application, visible);
    let promoted = block_on(application.dispatch(AppIntent::PromoteTemporaryDisplay))
        .expect("unhide promotes");
    assert!(point_visibility(&promoted, "sun").visible);
    assert!(point_visibility(&promoted, "sun").durable_visible);
    assert_eq!(point_visibility(&promoted, "sun").temporary_visible, None);
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
    let replacement = block_on(application.dispatch(AppIntent::BeginNewAspectSet))
        .expect_err("dirty editor cannot be replaced");
    assert_eq!(replacement.kind, AppErrorKind::Unavailable);
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

    let canceled = block_on(application.dispatch(AppIntent::CancelDraft)).expect("cancel succeeds");
    assert!(matches!(editor_state(&canceled), DraftState::Clean { .. }));
    block_on(application.wait_for_update(canceled.version)).expect("cancel refresh settles");
    assert_eq!(calls.get(), 1);

    block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
        AspectSetDraftMutation::SetTitle("Standard Revised".into()),
    )))
    .expect("saved title update succeeds");

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
    assert!(matches!(editor_state(&saved), DraftState::Clean { revision } if revision.get() == 2));
    assert!(
        saved.library.aspect_sets.iter().any(|summary| {
            summary.resource_id == standard && summary.title == "Standard Revised"
        })
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn new_and_duplicate_aspect_sets_preserve_full_rows_and_bind_the_workspace() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    ready(&application);

    let opened =
        block_on(application.dispatch(AppIntent::BeginNewAspectSet)).expect("new Aspect Set opens");
    let draft = opened
        .resource_editor
        .aspect_set
        .as_ref()
        .expect("new editor is projected");
    assert!(matches!(draft.state, DraftState::New));
    assert_eq!(draft.resource_id, None);
    assert_eq!(
        draft
            .aspects
            .iter()
            .map(|aspect| aspect.aspect_id.as_str())
            .collect::<Vec<_>>(),
        vec!["conjunction", "square"]
    );

    block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
        AspectSetDraftMutation::SetTitle("Research Orbs".into()),
    )))
    .expect("title changes");
    block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
        AspectSetDraftMutation::SetEnabled {
            aspect_id: mirabile_core::AspectId::new("conjunction").expect("aspect ID"),
            enabled: false,
        },
    )))
    .expect("Conjunction changes");
    block_on(application.dispatch(AppIntent::UpdateAspectSetDraft(
        AspectSetDraftMutation::SetOrb {
            aspect_id: mirabile_core::AspectId::new("square").expect("aspect ID"),
            maximum: angle(4.5),
        },
    )))
    .expect("Square changes");
    let creating = block_on(application.dispatch(AppIntent::SaveDraft)).expect("create starts");
    assert!(matches!(editor_state(&creating), DraftState::Creating));
    let created = settle(&application, creating);
    let editor = created
        .resource_editor
        .aspect_set
        .as_ref()
        .expect("created editor remains open");
    let created_id = editor.resource_id.expect("canonical identity is projected");
    assert!(
        matches!(editor.state, DraftState::Clean { revision } if revision == Revision::INITIAL)
    );
    assert_eq!(editor.title, "Research Orbs");
    assert_eq!(aspect_row(editor, "square").maximum_orb, angle(4.5));
    assert!(!aspect_row(editor, "conjunction").enabled);
    assert_eq!(created.inspector.active_aspect_set, Some(created_id));
    assert!(created.workspace.document_dirty);

    let CanonicalResource::AspectSet(created_resource) = block_on(repository.get(created_id))
        .expect("created resource reads")
        .expect("created resource exists")
    else {
        panic!("created resource is an Aspect Set")
    };
    assert_eq!(created_resource.title, "Research Orbs");
    assert_eq!(created_resource.payload.aspects.len(), 2);

    let standard_id = demo_ids().aspect_set_standard;
    let CanonicalResource::AspectSet(standard) = block_on(repository.get(standard_id))
        .expect("source reads")
        .expect("source exists")
    else {
        panic!("source is an Aspect Set")
    };
    let duplicated = block_on(application.dispatch(AppIntent::DuplicateAspectSet {
        resource_id: standard_id,
    }))
    .expect("duplicate opens");
    let duplicate = duplicated
        .resource_editor
        .aspect_set
        .as_ref()
        .expect("duplicate editor");
    assert!(matches!(duplicate.state, DraftState::New));
    assert_eq!(duplicate.title, format!("{} Copy", standard.title));
    assert_eq!(duplicate.aspects.len(), standard.payload.aspects.len());
    let creating = block_on(application.dispatch(AppIntent::SaveDraft)).expect("duplicate creates");
    let duplicated = settle(&application, creating);
    let duplicate_id = duplicated
        .resource_editor
        .aspect_set
        .as_ref()
        .and_then(|editor| editor.resource_id)
        .expect("duplicate canonical identity");
    assert_ne!(duplicate_id, standard_id);
    let CanonicalResource::AspectSet(duplicate) = block_on(repository.get(duplicate_id))
        .expect("duplicate reads")
        .expect("duplicate exists")
    else {
        panic!("duplicate is an Aspect Set")
    };
    assert_eq!(duplicate.payload, standard.payload);
}

#[test]
fn canceling_a_new_aspect_set_writes_nothing() {
    let repository = MemoryRepository::default();
    let application = demo_application(repository.clone());
    ready(&application);
    let before =
        block_on(repository.list(Some(ResourceKind::AspectSet))).expect("Aspect Sets list");
    block_on(application.dispatch(AppIntent::BeginNewAspectSet)).expect("new editor opens");
    let canceled = block_on(application.dispatch(AppIntent::CancelDraft)).expect("cancel succeeds");
    assert!(canceled.resource_editor.aspect_set.is_none());
    let after = block_on(repository.list(Some(ResourceKind::AspectSet))).expect("Aspect Sets list");
    assert_eq!(after, before);
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
    assert_eq!(aspect_row(&draft, "conjunction").maximum_orb, angle(9.0));
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

    let canceled =
        block_on(application.dispatch(AppIntent::CancelDraft)).expect("conflict cancel succeeds");
    let canceled_draft = canceled.resource_editor.aspect_set.expect("editor remains");
    assert!(matches!(
        canceled_draft.state,
        DraftState::Clean { revision } if revision.get() == 2
    ));
    assert_eq!(
        aspect_row(&canceled_draft, "conjunction").maximum_orb,
        angle(5.0)
    );
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
            .aspects
            .iter()
            .find(|aspect| aspect.aspect_id.as_str() == "conjunction")
            .expect("conjunction row")
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

    let canceled =
        block_on(application.dispatch(AppIntent::CancelDraft)).expect("cancel remains available");
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
    let retried =
        block_on(application.wait_for_update(retry_saving.version)).expect("retry save settles");
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
            .aspects
            .iter()
            .find(|aspect| aspect.aspect_id.as_str() == "conjunction")
            .expect("conjunction row")
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
    let saved = block_on(application.wait_for_update(retry.version)).expect("retry save succeeds");
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
    let original_semantic = initial.semantic_output.clone();
    assert!(!original_semantic.points.is_empty());
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
    let failed =
        block_on(application.wait_for_update(refreshing.version)).expect("failure is projected");
    let failed_view = failed.active_view.expect("active view");
    assert_eq!(failed_view.scene, Some(original));
    assert_eq!(failed.semantic_output, original_semantic);
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
    let request_a_state =
        block_on(application.dispatch(AppIntent::RefreshActiveView)).expect("request A accepted");
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
    let request_c_state =
        block_on(application.dispatch(AppIntent::RefreshActiveView)).expect("request C accepted");
    application.state.borrow_mut().cache.clear();
    let request_d_state =
        block_on(application.dispatch(AppIntent::RefreshActiveView)).expect("request D accepted");
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
    let saving = block_on(first.dispatch(AppIntent::SaveWorkspace)).expect("workspace save starts");
    settle(&first, saving);
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

    let followed = resolve_typed_binding(
        &ResourceBinding::<PointSet>::Follow { id },
        &catalog,
        ConfigurationLayer::Workspace,
    )
    .expect("follow resolves");
    let pinned = resolve_typed_binding(
        &ResourceBinding::<PointSet>::Pinned {
            id,
            revision: Revision::INITIAL,
        },
        &catalog,
        ConfigurationLayer::Workspace,
    )
    .expect("pin resolves");
    let inline = resolve_typed_binding(
        &ResourceBinding::Inline {
            value: PointSet { points: Vec::new() },
        },
        &catalog,
        ConfigurationLayer::Workspace,
    )
    .expect("inline resolves");

    assert_eq!(followed.layer, ConfigurationLayer::Workspace);
    assert_eq!(pinned.layer, ConfigurationLayer::Workspace);
    assert_eq!(inline.layer, ConfigurationLayer::Workspace);
    assert_eq!(
        followed.source,
        ValueSource::Follow {
            resource_id: id,
            revision: second.revision,
        }
    );
    assert_eq!(
        pinned.source,
        ValueSource::Pinned {
            resource_id: id,
            revision: Revision::INITIAL,
        }
    );
    assert_eq!(inline.source, ValueSource::Inline);
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
