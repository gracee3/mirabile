use astra_app::{
    AppIntent, AppReadModel, Application, AspectSetDraftMutation, ChartPersistence, DraftState,
    IndexedDbRepositorySource, RealApplication, ViewComputationState, WorkerCalculationRuntime,
    bootstrap_ids,
};
use astra_core::{
    Angle, AspectId, CanonicalResource, PointId, PointSelector, PointSet, ResourceEnvelope,
    ResourceId, Revision, Timestamp,
};
use astra_store::{IndexedDbRepository, RepositoryError, ResourceRepository, ResourceState};
use leptos::prelude::*;

#[component]
pub fn BrowserContract() -> impl IntoView {
    let status = RwSignal::new(String::from("running"));
    let detail = RwSignal::new(String::from("IndexedDB contract is running"));

    leptos::task::spawn_local(async move {
        match run_contract().await {
            Ok(()) => {
                status.set("passed".into());
                detail.set("ASTRA_BROWSER_CONTRACT:PASS".into());
            }
            Err(error) => {
                status.set("failed".into());
                detail.set(format!("ASTRA_BROWSER_CONTRACT:FAIL:{error}"));
            }
        }
    });

    view! {
        <main>
            <h1>"Astra IndexedDB browser contract"</h1>
            <p id="browser-contract-result" data-status=move || status.get()>
                {move || detail.get()}
            </p>
        </main>
    }
}

#[allow(clippy::too_many_lines)]
async fn run_contract() -> Result<(), String> {
    let database_name = format!("astra-browser-contract-{}", ResourceId::new());
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

    run_real_application_reload().await?;

    Ok(())
}

async fn run_real_application_reload() -> Result<(), String> {
    let database_name = format!("astra-real-application-contract-{}", ResourceId::new());
    let ids = bootstrap_ids();
    let first_runtime = WorkerCalculationRuntime::xalen();
    let first = RealApplication::indexed_db_with_runtime(&database_name, first_runtime.clone());
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
            .is_some_and(|identity| identity.id == astra_engine::XalenBackend::ID),
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
    drop(first);

    let second_runtime = WorkerCalculationRuntime::xalen();
    let second = RealApplication::indexed_db_with_runtime(&database_name, second_runtime.clone());
    let restored = settle_initialization(&second).await?;
    ensure(
        restored.workspace.active_chart == Some(chart_b),
        "the second RealApplication did not restore Chart B as active",
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
        "the second RealApplication did not restore Chart B as open",
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
            .is_some_and(|identity| identity.id == astra_engine::XalenBackend::ID),
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
    loop {
        let view_pending = model.active_view.as_ref().is_some_and(|view| {
            matches!(
                view.computation,
                ViewComputationState::Loading | ViewComputationState::Refreshing
            )
        });
        let save_pending = model
            .resource_editor
            .aspect_set
            .as_ref()
            .is_some_and(|draft| matches!(draft.state, DraftState::Saving { .. }));
        if !view_pending && !save_pending {
            return Ok(model);
        }
        model = application
            .wait_for_update(model.version)
            .await
            .map_err(message)?;
    }
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
