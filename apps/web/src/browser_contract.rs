use astra_core::{
    CanonicalResource, PointId, PointSelector, PointSet, ResourceEnvelope, ResourceId, Revision,
    Timestamp,
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

    Ok(())
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
