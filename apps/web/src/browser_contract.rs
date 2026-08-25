use leptos::prelude::*;
use mirabile_app::{
    AppIntent, AppReadModel, Application, AspectSetDraftMutation, ChartPersistence, DraftState,
    IndexedDbRepositorySource, RealApplication, StartupPolicy, ViewComputationState,
    WorkerCalculationRuntime, apparent_place_demo_resources, demo_ids,
};
use mirabile_core::{
    Angle, AspectId, CanonicalResource, PointId, PointSelector, PointSet, ResourceEnvelope,
    ResourceId, Revision, Timestamp,
};
use mirabile_store::{
    AtomicSaveBatch, IndexedDbRepository, RepositoryError, ResourceRepository, ResourceState,
    RevisionExpectation,
};

#[component]
pub fn BrowserContract() -> impl IntoView {
    let status = RwSignal::new(String::from("running"));
    let detail = RwSignal::new(String::from("IndexedDB contract is running"));

    leptos::task::spawn_local(async move {
        match run_contract().await {
            Ok(()) => {
                status.set("passed".into());
                detail.set("MIRABILE_BROWSER_CONTRACT:PASS".into());
            }
            Err(error) => {
                status.set("failed".into());
                detail.set(format!("MIRABILE_BROWSER_CONTRACT:FAIL:{error}"));
            }
        }
    });

    view! {
        <main>
            <h1>"Mirabile IndexedDB browser contract"</h1>
            <p id="browser-contract-result" data-status=move || status.get()>
                {move || detail.get()}
            </p>
        </main>
    }
}

#[allow(clippy::too_many_lines)]
async fn run_contract() -> Result<(), String> {
    let database_name = format!("mirabile-browser-contract-{}", ResourceId::new());
    let first = IndexedDbRepository::open(&database_name)
        .await
        .map_err(message)?;
    let second = IndexedDbRepository::open(&database_name)
        .await
        .map_err(message)?;

    let initial = point_resource("Browser contract");
    let id = initial.id();
    first.create(initial.clone()).await.map_err(message)?;
    ensure(
        second.get(id).await.map_err(message)? == Some(initial.clone()),
        "second handle could not read the created resource",
    )?;

    let revision_two = next_point(&initial, "Revision two", 2)?;
    second
        .save(Revision::INITIAL, revision_two.clone())
        .await
        .map_err(message)?;
    ensure(
        matches!(
            first
                .get_revision(id, Revision::INITIAL)
                .await
                .map_err(message)?,
            Some(ResourceState::Present(ref resource)) if resource == &initial
        ),
        "revision-one history was not preserved",
    )?;

    let revision_three = next_point(&revision_two, "Revision three", 3)?;
    first
        .save(revision_two.revision(), revision_three.clone())
        .await
        .map_err(message)?;
    let stale_revision_three = next_point(&revision_two, "Stale revision three", 4)?;
    ensure(
        matches!(
            second
                .save(revision_two.revision(), stale_revision_three)
                .await,
            Err(RepositoryError::Conflict { actual, .. }) if actual == revision_three.revision()
        ),
        "stale save was not rejected",
    )?;
    ensure(
        matches!(
            second
                .delete(id, revision_two.revision(), Timestamp::from_unix_millis(5))
                .await,
            Err(RepositoryError::Conflict { actual, .. }) if actual == revision_three.revision()
        ),
        "stale delete was not rejected",
    )?;

    let tombstone = second
        .delete(
            id,
            revision_three.revision(),
            Timestamp::from_unix_millis(6),
        )
        .await
        .map_err(message)?;
    ensure(
        tombstone.revision.get() == 4,
        "deletion did not create revision four",
    )?;
    ensure(
        first.get(id).await.map_err(message)?.is_none(),
        "ordinary get exposed a tombstone",
    )?;
    ensure(
        first.list(None).await.map_err(message)?.is_empty(),
        "ordinary list exposed a tombstone",
    )?;
    ensure(
        matches!(
            first.get_head(id).await.map_err(message)?,
            Some(ResourceState::Deleted(ref value)) if value == &tombstone
        ),
        "head did not expose the tombstone",
    )?;
    ensure(
        matches!(
            first
                .get_revision(id, tombstone.revision)
                .await
                .map_err(message)?,
            Some(ResourceState::Deleted(ref value)) if value == &tombstone
        ),
        "historical tombstone was not retrievable",
    )?;
    ensure(
        matches!(
            first.create(initial).await,
            Err(RepositoryError::AlreadyExists(value)) if value == id
        ),
        "deleted stable ID was recreated",
    )?;

    let collision_initial = point_resource("Rollback fixture");
    let collision_id = collision_initial.id();
    first
        .create(collision_initial.clone())
        .await
        .map_err(message)?;
    let collision_revision = Revision::new(2).map_err(message)?;
    first
        .force_history_key_collision(collision_id, collision_revision)
        .await
        .map_err(message)?;
    let collision_next = next_point(&collision_initial, "Must roll back", 2)?;
    ensure(
        first.save(Revision::INITIAL, collision_next).await.is_err(),
        "forced history collision unexpectedly succeeded",
    )?;
    ensure(
        matches!(
            second.get_head(collision_id).await.map_err(message)?,
            Some(ResourceState::Present(ref resource))
                if resource.revision() == Revision::INITIAL
                    && resource.title() == "Rollback fixture"
        ),
        "failed transaction changed the current head",
    )?;

    let batch_first = point_resource("Atomic batch first");
    let batch_second = point_resource("Atomic batch second");
    let batch_first_id = batch_first.id();
    let batch_second_id = batch_second.id();
    first
        .force_initial_history_collision(batch_second_id)
        .await
        .map_err(message)?;
    ensure(
        first
            .create_batch(vec![batch_first, batch_second])
            .await
            .is_err(),
        "forced atomic create-batch collision unexpectedly succeeded",
    )?;
    ensure(
        first.get(batch_first_id).await.map_err(message)?.is_none()
            && first.get(batch_second_id).await.map_err(message)?.is_none(),
        "failed atomic create batch left a partially created resource",
    )?;

    let save_first = point_resource("Atomic save first");
    let save_second = point_resource("Atomic save second");
    let save_first_id = save_first.id();
    let save_second_id = save_second.id();
    first
        .create_batch(vec![save_first.clone(), save_second.clone()])
        .await
        .map_err(message)?;
    let save_first_next = next_point(&save_first, "Atomic save first next", 7)?;
    let save_second_next = next_point(&save_second, "Atomic save second next", 7)?;
    first
        .force_history_key_collision(save_second_id, save_second_next.revision())
        .await
        .map_err(message)?;
    ensure(
        first
            .save_batch(AtomicSaveBatch {
                expectations: vec![
                    RevisionExpectation {
                        id: save_first_id,
                        expected_revision: Revision::INITIAL,
                    },
                    RevisionExpectation {
                        id: save_second_id,
                        expected_revision: Revision::INITIAL,
                    },
                ],
                changes: vec![save_first_next, save_second_next],
            })
            .await
            .is_err(),
        "forced atomic save-batch collision unexpectedly succeeded",
    )?;
    ensure(
        matches!(
            second.get_head(save_first_id).await.map_err(message)?,
            Some(ResourceState::Present(ref resource))
                if resource.revision() == Revision::INITIAL
                    && resource.title() == "Atomic save first"
        ) && matches!(
            second.get_head(save_second_id).await.map_err(message)?,
            Some(ResourceState::Present(ref resource))
                if resource.revision() == Revision::INITIAL
                    && resource.title() == "Atomic save second"
        ),
        "failed atomic save batch changed at least one current head",
    )?;

    run_real_application_reload().await?;

    Ok(())
}

async fn run_real_application_reload() -> Result<(), String> {
    let database_name = format!("mirabile-real-application-contract-{}", ResourceId::new());
    let ids = demo_ids();
    let empty_repository = IndexedDbRepository::open(&database_name)
        .await
        .map_err(message)?;
    let fresh_runtime = WorkerCalculationRuntime::xalen();
    let fresh = RealApplication::indexed_db_with_runtime(&database_name, fresh_runtime.clone());
    let fresh_ready = settle_initialization(&fresh).await?;
    ensure(
        fresh_ready.library.charts.is_empty()
            && fresh_ready.library.aspect_sets.is_empty()
            && fresh_ready.workspace.document_id.is_none()
            && fresh_ready.workspace.charts.len() == 1
            && fresh_ready.workspace.charts[0].persistence == ChartPersistence::Ephemeral,
        "empty IndexedDB did not start an ephemeral Current Transits session",
    )?;
    ensure(
        empty_repository
            .list(None)
            .await
            .map_err(message)?
            .is_empty(),
        "Current Transits startup polluted the canonical IndexedDB library",
    )?;
    let current_transits_instance = fresh_ready
        .workspace
        .active_chart
        .ok_or_else(|| "Current Transits draft was not active".to_owned())?;
    let first_workspace_saving = fresh
        .dispatch(AppIntent::SaveWorkspace)
        .await
        .map_err(message)?;
    let first_workspace_save = settle_pending(&fresh, first_workspace_saving).await?;
    let current_transits_workspace = first_workspace_save
        .workspace
        .document_id
        .ok_or_else(|| "first workspace save did not assign an identity".to_owned())?;
    ensure(
        first_workspace_save.workspace.document_revision == Some(Revision::INITIAL)
            && !first_workspace_save.workspace.document_dirty,
        "fresh IndexedDB session did not create WorkspaceDocument revision one",
    )?;
    let CanonicalResource::WorkspaceDocument(first_document) = empty_repository
        .get(current_transits_workspace)
        .await
        .map_err(message)?
        .ok_or_else(|| "first WorkspaceDocument was not persisted".to_owned())?
    else {
        return Err("first workspace save persisted the wrong resource kind".into());
    };
    ensure(
        first_document.payload.chart_instances.is_empty()
            && first_document.payload.views.iter().all(|view| {
                !view
                    .charts
                    .values()
                    .any(|chart| *chart == current_transits_instance)
            }),
        "first WorkspaceDocument leaked its Current Transits draft assignment",
    )?;
    let saving_current_transits = fresh
        .dispatch(AppIntent::SaveChartDraft {
            instance_id: current_transits_instance,
        })
        .await
        .map_err(message)?;
    let saved_current_transits = settle_pending(&fresh, saving_current_transits).await?;
    ensure(
        saved_current_transits.library.charts.len() == 1
            && saved_current_transits.workspace.charts.iter().any(|chart| {
                chart.instance_id == current_transits_instance
                    && matches!(chart.persistence, ChartPersistence::Saved { .. })
            }),
        "IndexedDB ChartDraft save did not publish a saved library chart",
    )?;
    ensure(
        saved_current_transits.workspace.document_dirty,
        "saving the draft did not promote membership and slot assignment into the working document",
    )?;
    ensure(
        empty_repository
            .list(Some(mirabile_core::ResourceKind::ChartRecord))
            .await
            .map_err(message)?
            .len()
            == 1
            && empty_repository
                .list(Some(mirabile_core::ResourceKind::ChartDefinition))
                .await
                .map_err(message)?
                .len()
                == 1,
        "IndexedDB ChartDraft save did not atomically create one record and one definition",
    )?;
    let promoting_workspace = fresh
        .dispatch(AppIntent::SaveWorkspace)
        .await
        .map_err(message)?;
    let promoted_workspace = settle_pending(&fresh, promoting_workspace).await?;
    ensure(
        promoted_workspace
            .workspace
            .document_revision
            .map(Revision::get)
            == Some(2)
            && !promoted_workspace.workspace.document_dirty,
        "promoted Current Transits workspace was not saved as revision two",
    )?;
    let CanonicalResource::WorkspaceDocument(promoted_document) = empty_repository
        .get(current_transits_workspace)
        .await
        .map_err(message)?
        .ok_or_else(|| "promoted WorkspaceDocument was not persisted".to_owned())?
    else {
        return Err("promoted workspace save persisted the wrong resource kind".into());
    };
    ensure(
        promoted_document.payload.chart_instances.len() == 1
            && promoted_document
                .payload
                .views
                .iter()
                .flat_map(|view| view.charts.values())
                .any(|chart| *chart == current_transits_instance),
        "saved ChartDraft assignment was not promoted into the durable WorkspaceDocument",
    )?;
    drop(fresh);

    let promoted_runtime = WorkerCalculationRuntime::xalen();
    let promoted_reload = RealApplication::indexed_db_with_runtime_and_policy(
        &database_name,
        promoted_runtime,
        StartupPolicy::OpenWorkspace(current_transits_workspace),
    );
    let promoted_restored = settle_initialization(&promoted_reload).await?;
    ensure(
        promoted_restored.workspace.charts.iter().any(|chart| {
            chart.instance_id == current_transits_instance
                && matches!(chart.persistence, ChartPersistence::Saved { .. })
        }) && promoted_restored.active_view.as_ref().is_some_and(|view| {
            view.slots
                .iter()
                .any(|slot| slot.chart == Some(current_transits_instance))
        }),
        "IndexedDB reload did not restore the promoted chart and slot assignment",
    )?;
    drop(promoted_reload);

    for resource in apparent_place_demo_resources() {
        empty_repository.create(resource).await.map_err(message)?;
    }
    let first_runtime = WorkerCalculationRuntime::xalen();
    let first = RealApplication::indexed_db_with_runtime_and_policy(
        &database_name,
        first_runtime.clone(),
        StartupPolicy::OpenWorkspace(ids.workspace),
    );
    let first_ready = settle_initialization(&first).await?;
    ensure(
        first_ready.active_view.as_ref().is_some_and(|view| {
            view.scene.is_some() && view.computation == ViewComputationState::Fresh
        }),
        "first RealApplication did not calculate its initial Scene",
    )?;
    ensure(
        first_runtime.completed_results() > 0,
        "first RealApplication did not receive a Web Worker calculation result",
    )?;
    ensure(
        first_runtime
            .last_backend_identity()
            .is_some_and(|identity| identity.id == mirabile_engine::XalenBackend::ID),
        "Web Worker result did not identify the XALEN backend",
    )?;

    let opened = first
        .dispatch(AppIntent::OpenChart {
            definition_id: ids.chart_definition_b,
        })
        .await
        .map_err(message)?;
    let chart_b = opened
        .workspace
        .active_chart
        .ok_or_else(|| "opening Chart B did not activate it".to_owned())?;
    ensure(
        opened.workspace.charts.iter().any(|chart| {
            chart.instance_id == chart_b
                && matches!(
                    chart.persistence,
                    ChartPersistence::Saved { definition_id }
                        if definition_id == ids.chart_definition_b
                )
        }),
        "Chart B was not projected as an open saved ChartDefinition",
    )?;

    first
        .dispatch(AppIntent::BeginAspectSetEdit {
            resource_id: ids.aspect_set_standard,
        })
        .await
        .map_err(message)?;
    let dirty = first
        .dispatch(AppIntent::UpdateAspectSetDraft(
            AspectSetDraftMutation::SetOrb {
                aspect_id: AspectId::new("conjunction").map_err(message)?,
                maximum: Angle::from_degrees(6.5).map_err(message)?,
            },
        ))
        .await
        .map_err(message)?;
    let preview = settle_pending(&first, dirty).await?;
    ensure(
        matches!(
            preview
                .resource_editor
                .aspect_set
                .as_ref()
                .map(|draft| &draft.state),
            Some(DraftState::Dirty { .. })
        ),
        "Aspect Set preview did not remain dirty after view refresh",
    )?;
    let saving = first
        .dispatch(AppIntent::SaveDraft)
        .await
        .map_err(message)?;
    let saved = settle_pending(&first, saving).await?;
    ensure(
        saved.library.aspect_sets.iter().any(|summary| {
            summary.resource_id == ids.aspect_set_standard
                && summary.revision.get() == 2
                && summary.conjunction_orb == Angle::from_degrees(6.5).expect("finite angle")
        }),
        "Aspect Set revision two was not committed by the first RealApplication",
    )?;
    let workspace_saving = first
        .dispatch(AppIntent::SaveWorkspace)
        .await
        .map_err(message)?;
    let workspace_saved = settle_pending(&first, workspace_saving).await?;
    ensure(
        !workspace_saved.workspace.document_dirty
            && workspace_saved
                .workspace
                .document_revision
                .map(Revision::get)
                == Some(2),
        "dirty WorkspaceDocument was not explicitly saved as revision two",
    )?;
    drop(first);

    let second_runtime = WorkerCalculationRuntime::xalen();
    let second = RealApplication::indexed_db_with_runtime_and_policy(
        &database_name,
        second_runtime.clone(),
        StartupPolicy::OpenWorkspace(ids.workspace),
    );
    let restored = settle_initialization(&second).await?;
    ensure(
        restored.workspace.active_chart == Some(ids.chart_instance_a),
        "the second RealApplication did not create fresh session navigation from the saved document",
    )?;
    ensure(
        restored.workspace.charts.iter().any(|chart| {
            chart.instance_id == chart_b
                && matches!(
                    chart.persistence,
                    ChartPersistence::Saved { definition_id }
                        if definition_id == ids.chart_definition_b
                )
        }),
        "the second RealApplication did not restore Chart B in saved document membership",
    )?;
    ensure(
        restored.library.aspect_sets.iter().any(|summary| {
            summary.resource_id == ids.aspect_set_standard
                && summary.revision.get() == 2
                && summary.conjunction_orb == Angle::from_degrees(6.5).expect("finite angle")
        }),
        "the second RealApplication did not hydrate Aspect Set revision two",
    )?;
    ensure(
        restored.active_view.as_ref().is_some_and(|view| {
            view.scene.is_some() && view.computation == ViewComputationState::Fresh
        }),
        "the second RealApplication did not reconstruct the persisted workspace view",
    )?;
    ensure(
        second_runtime.completed_results() > 0,
        "reloaded RealApplication did not calculate through a Web Worker",
    )?;
    ensure(
        second_runtime
            .last_backend_identity()
            .is_some_and(|identity| identity.id == mirabile_engine::XalenBackend::ID),
        "reloaded Web Worker result did not identify the XALEN backend",
    )?;
    Ok(())
}

async fn settle_initialization(
    application: &RealApplication<IndexedDbRepositorySource, WorkerCalculationRuntime>,
) -> Result<AppReadModel, String> {
    let model = application.initialize().await.map_err(message)?;
    settle_pending(application, model).await
}

async fn settle_pending(
    application: &RealApplication<IndexedDbRepositorySource, WorkerCalculationRuntime>,
    mut model: AppReadModel,
) -> Result<AppReadModel, String> {
    while !model.is_settled() {
        model = application
            .wait_for_update(model.version)
            .await
            .map_err(message)?;
    }
    Ok(model)
}

fn point_resource(title: &str) -> CanonicalResource {
    CanonicalResource::PointSet(ResourceEnvelope::new(
        title,
        PointSet {
            points: vec![PointSelector::Point(
                PointId::new("sun").expect("fixture identifier is valid"),
            )],
        },
        Timestamp::from_unix_millis(1),
    ))
}

fn next_point(
    resource: &CanonicalResource,
    title: &str,
    modified_at: i64,
) -> Result<CanonicalResource, String> {
    let CanonicalResource::PointSet(envelope) = resource else {
        return Err("fixture changed resource kind".into());
    };
    let mut next = envelope
        .next_with_payload(
            envelope.payload.clone(),
            Timestamp::from_unix_millis(modified_at),
        )
        .map_err(message)?;
    next.title = title.into();
    Ok(CanonicalResource::PointSet(next))
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
