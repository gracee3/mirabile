# Application interface contract

`mirabile-app` is the shared boundary between presentation adapters and application implementations.
The normal Leptos/WASM adapter uses `RealApplication`; native frontend tests retain
`MockApplication`. Both implement the same frozen `Application` interface and preserve the
semantics below. Read models remain projections of canonical domain and repository state rather
than a replacement for either.

## Authority and dependency boundary

Read models are authoritative UI projections. Components render them and dispatch `AppIntent`;
they do not mutate a local `WorkspaceDocument`, `WorkspaceSession`, `ViewInstance`, or canonical resource and assume it was
accepted. The initial `dispatch` result is a full projection for correctness. Read models may later
be queried independently rather than remaining one permanent, database-sized aggregate.

`mirabile-app` re-exports Mirabile's existing stable IDs and current astrology-free `Scene` primitives.
It does not create parallel frontend identity domains or duplicate `Scene`. The crate now depends
on `mirabile-store` because it owns real orchestration and hydration. Normal UI modules still depend
only on `mirabile-app`; the feature-gated IndexedDB browser-contract harness remains an isolated
test-only exception with direct repository dependencies for its pre-existing adapter checks.

No `AppEvent` stream exists in this slice. If events are added, they announce that authoritative
application state changed; read models remain authoritative. Missing an event must be recoverable
by requesting a new projection.

## Binding projections

`ResourceBindingSummary` contains a display label and a truthful `BindingSourceSummary`:

- `Follow { resource_id, resource_title, revision }` identifies the followed resource and exposes
  the current revision resolved by the application;
- `Pinned { resource_id, resource_title, revision }` identifies the resource and exact pinned
  revision;
- `Inline` carries no `ResourceId`, title, or `Revision`, because canonical inline bindings have no
  independent resource identity.

The application must never fabricate identity or revision metadata to present an inline value.
This projection does not change canonical `ResourceBinding` semantics.

## Chart Definition identity

Chart library and saved-workspace identity always means a canonical `ChartDefinition`, not the
`ChartRecord` referenced by that definition. The contract makes this explicit through
`LibraryChartSummary::definition_id`, `ChartPersistence::Saved { definition_id }`, and
`AppIntent::OpenChart { definition_id }`. These fields continue to use Mirabile's existing
`ResourceId`; no duplicate chart ID type exists. Aspect Sets and other genuinely generic resources
retain `resource_id` naming.

## Projection identity and asynchronous completion

Every `AppReadModel` has a `ProjectionVersion`, a monotonically increasing `u64` sequence scoped to
one `Application` instance. Version zero is the frontend's pre-initialization placeholder. The
version is application projection state: it is not a canonical resource `Revision`, is not derived
from resource revisions, and is not a persistent synchronization token.

Every authoritative state transition that can produce a new read model receives a newer version.
This includes accepted initialization and intents as well as asynchronous transitions such as
`Loading` to `Fresh`, `Fresh` to `Refreshing`, `Refreshing` to `Fresh` or `Failed`, `Dirty` to
`Saving`, and `Saving` to `Clean` or `Conflict`. An immediate read without an intervening transition
keeps the current version.

The application interface is:

```rust
#[async_trait(?Send)]
pub trait Application {
    async fn initialize(&self) -> AppResult<AppReadModel>;
    async fn dispatch(&self, intent: AppIntent) -> AppResult<AppReadModel>;
    async fn snapshot(&self) -> AppResult<AppReadModel>;
    async fn wait_for_update(
        &self,
        after: ProjectionVersion,
    ) -> AppResult<AppReadModel>;
}
```

`snapshot()` is an immediate authoritative read. It neither waits for nor completes worker,
repository, or mock work. Repeated snapshots without a transition return the same version.

`wait_for_update(after)` returns only a projection whose version is strictly newer than `after`.
It may return immediately if that newer projection already exists; otherwise the implementation
awaits a meaningful authoritative transition. `RealApplication` receives one calculation-runtime
result or completes one queued repository/cache transition without a timer. Its version
notification is shared observation, not a single-consumer message: all waiters after version N can
observe N+1 or another version newer than N. If no work is pending, a waiter remains registered for
the next authoritative transition rather than polling or fabricating a version. The runtime
notification primitive is deliberately outside the contract, and no authoritative event stream is
introduced.

The frontend pending lifecycle is:

```text
dispatch
→ publish accepted pending projection N
→ wait_for_update(after=N)
→ publish authoritative projection N+1
→ repeat only while the projection remains pending
```

There is no hidden browser-tick or one-snapshot completion assumption. Loading and Refreshing views
and Saving drafts promise a later authoritative transition to a settled success or error state.

All asynchronous read-model results use one publication rule: accept only
`incoming.version > current.version`. Equal versions are redundant copies and older versions are
stale completions; both are ignored. Therefore a version 11 response can never replace version 12
frontend state.

## Application and view status

`ApplicationStatus::{Initializing, Ready, Error}` describes application hydration. It is
independent from `ViewComputationState`, so a ready application can contain a loading, refreshing,
or failed view without becoming a shell-level error.

`ViewReadModel` combines `scene: Option<Scene>` with:

| Scene | Computation | Meaning |
| --- | --- | --- |
| none | `Loading` | No successful computation exists yet. |
| some | `Fresh` | The Scene is current. |
| some | `Refreshing` | Display the last good Scene while new work runs. |
| some | `Failed(error)` | Keep displaying the last good Scene and surface the scoped error. |
| none | `Failed(error)` | Surface that the view has never computed successfully. |

The real implementation returns accepted intermediate states from `dispatch` and completes
runtime/repository work through `wait_for_update`. A calculation failure changes only the
computation state and notice; it never clears the last successful `Scene`. A newer calculation
intent is accepted while the view is already `Refreshing`; it replaces that view's expected typed
request ID and CalcKey. Stale success and failure results make no authoritative transition and do
not advance `ProjectionVersion`.

## Workspace document and session policy

Active and selected charts are separate contract fields owned by `WorkspaceSession`.
`RealApplication` applies this policy without creating `WorkspaceDocument` revisions:

- activating a chart does not add or remove selection;
- selecting or deselecting a chart does not activate it;
- opening a library chart activates it but preserves selection;
- closing removes only that chart from selection and slot assignments;
- closing the active chart activates the next chart at the same rail position, or the preceding
  chart when there is no next chart;
- a required slot that referenced a closed chart receives the resulting active chart when one
  exists; an optional slot becomes unassigned.

There is deliberately no invariant that the active chart must be selected. Shift-range selection
is deferred. This policy is easy to revise after shared architecture review because the transitions
live in the application adapter and tests, not DOM handlers.

Saved chart membership/order, view instances and slot assignments, workspace bindings, and durable
view overrides are the working `WorkspaceDocument` projection. Such changes mark the session dirty.
Only `AppIntent::SaveWorkspace` writes its next revision. Temporary display overrides stay in the
session until explicitly promoted; promotion updates the durable projection and marks it dirty.

## Intents, drafts, and form buffers

`AppIntent` expresses user-level application intentions. It is not the persistence-oriented
`mirabile_core::Command`; `RealApplication` translates workspace intents into typed core commands,
applies their semantics in `mirabile-app`. Durable mutations mark the `WorkspaceDocument` dirty,
and explicit Save Workspace writes its next revision. Resource repository
rules remain in `mirabile-store`.

Draft mutations are typed. This slice uses `AspectSetDraftMutation::{SetOrb, SetEnabled}` with a
typed `AspectId`, `Angle`, and boolean. String field paths and untyped JSON values are not part of
the application API.

`AspectSetDraftReadModel` projects `Clean`, `Dirty`, `Saving`, and `Conflict` plus base/current or
remote revision metadata. The application owns that resource-level lifecycle. The frontend owns
only HTML mechanics such as focus, input text, temporary invalid syntax, and validation display.
It parses a valid buffer into a typed mutation; it does not create a second resource editor state
machine. Save constructs the next canonical AspectSet revision but publishes it only after the
repository accepts the optimistic write. Cancel resolves the current canonical value without a
write. A repository `Conflict` projects the real base and remote revisions while retaining the
local draft and leaving `ApplicationStatus::Ready`.

Changing a valid Aspect Set draft queues a view refresh while retaining the previous `Scene`. The
real pipeline reuses `CalculationValue` by `CalcKey`, rebuilds `SnapshotContext`, and reruns aspect
analysis and wheel layout with the preview. Save does not recalculate an astronomically identical
chart, and Cancel returns analysis to canonical AspectSet semantics through the same cache path.

## Capabilities and presentation commands

The application projects `CommandCapability { action, availability }`, where availability is
`Enabled`, `Disabled { reason }`, or `Hidden`. Only actions used by this slice exist. Disabled
reasons remain application-owned and are exposed to assistive and visual presentation.

Labels, shortcuts, grouping, keywords, and eventual command-palette presentation live in the web
adapter's command registry. The current shortcuts support primary-modifier Save, Escape Cancel
outside text-entry controls, Alt+1 chart-rail focus, and primary-modifier Refresh outside text-entry
controls. The frontend does not infer executability from draft details; it consumes capabilities.

## Error classes and interchangeability

`AppErrorKind` distinguishes initialization, view computation, conflict, invalid intent, missing
state, and unavailable operations. Initialization errors replace the shell with Retry. View errors
remain scoped to the view and retain its last successful Scene. Draft conflicts remain scoped to
the editor with both revisions visible.

Frontend construction injects an `Application` trait object. Normal shell components know only
`AppIntent`, read models, capabilities, errors, stable IDs, and `Scene`; they do not know whether the
implementation uses a mock, IndexedDB, memory, workers, a calculation engine, or future sync. The
normal WASM shell now constructs `RealApplication::browser_default()` without importing store or
engine crates.

## RealApplication construction and hydration

`RealApplication<R, C>` is the public facade in `crates/mirabile-app/src/real_application/mod.rs`.
Its private sibling modules separate catalog resolution, hydration/startup, workspace commands,
draft editing, calculation scheduling, configuration, projection, and state bookkeeping. `R` is a
cloneable `ResourceRepository`, and `C` implements the application-facing `CalculationRuntime`. Native code
uses `InlineCalculationRuntime<DeterministicBackend>` or a controlled runtime. The WASM constructor
uses `WorkerCalculationRuntime` plus `IndexedDbRepositorySource`, which lazily opens one
`IndexedDbRepository` during initialization and retains its cloneable `Rc<Rexie>` handle for the
application lifetime. The application, not the web shell, owns repository or worker orchestration.

Initialization lists whatever canonical resources exist and loads pinned historical revisions
before applying an application-level `StartupPolicy`. `RestorePreviousSession` is the default;
because session recovery is not implemented yet, it falls back to `CurrentTransits`.
`BlankWorkspace`, `OpenWorkspace(id)`, and the future-facing `OpenWorkspaces(ids)` forms are also
represented. The current UI activates only the first requested workspace.

A genuinely empty repository is valid. Fresh startup creates an unsaved `WorkspaceSession` with
one ephemeral Current Transits `ChartDraft`, a single wheel, system UTC time, tropical geocentric
positions, the currently supported six-point set, no houses, and no asserted location. This path
does not write a `ChartRecord`, `ChartDefinition`, or `WorkspaceDocument`, request geolocation, or
invent coordinates. The XALEN constructor selects its required apparent-place correction profile;
the deterministic test constructor selects the baseline no-correction profile.

The seven-resource example set remains available only through the explicit `demo_resources()` or
`apparent_place_demo_resources()` bundle. Tests and the browser reload contract load that bundle
deliberately and open its saved workspace through `StartupPolicy::OpenWorkspace`. The demo
workspace stores its AspectSet as `Follow(Standard)`; its other bindings are honest inline values.
The resolver calls core `resolve_binding` for Follow, Pinned, and Inline, and pinned history is
hydrated before projection and computation.

`CalculationEngine::resolve` accepts payload-only `ChartRecord` plus `CalculationSpec` semantics.
Canonical preparation adds resource revision context separately; draft preparation attaches a
`SnapshotContext` with no canonical identities. Geocentric no-house calculation accepts absent
location. Topocentric calculation, houses, and local-mean-time resolution fail explicitly when
their required observer assertion is absent.

## Chart draft and atomic create lifecycle

`ChartDraft { title, record, calculation }` is an application aggregate, never a canonical
resource. `WorkspaceSession` holds draft charts beside the saved chart references in its working
document. Starting and canceling a draft do not write the library. A draft can be assigned to a
view and calculated through the payload-only engine seam before it has resource identities.

`SaveChartDraft` creates a new revision-one `ChartRecord` and a distinct revision-one
`ChartDefinition` whose radix source references that record. `ResourceRepository::create_batch`
is the narrow atomicity boundary: MemoryRepository prevalidates and preflights the complete batch
before mutating its maps; IndexedDB writes both current heads and both history entries in one
read-write transaction. Any validation, identity, or storage failure publishes neither resource,
and the application retains the draft. Success replaces the session draft with a saved chart
instance, adds the chart to the library, and marks workspace membership dirty for an explicit
workspace save.

Calculation requests are submitted through `CalculationRuntime` as soon as the authoritative
intent is prepared. Native inline and controlled runtimes use the exact versioned worker protocol.
Normal WASM operation communicates with the Trunk-built Web Worker and linked
`XalenBackend`; backend calculation therefore executes outside the Leptos/UI context.
Optimistic resource saves still await the retained repository. `snapshot()` never receives runtime
results or drives pending work. The deterministic backend remains a demo/test implementation for
native and controlled runtime tests, not real astronomy.
Trunk also copies the XALEN/ERFA/VSOP third-party notices and license texts into
the browser distribution; the browser contract fails if those assets are absent or changed.

Each view tracks its current expected `(CalculationRequestId, CalcKey)`. A runtime result is
accepted only when both match that view and its protocol version is current. Accepted success is
cached, analyzed, and laid out; accepted failure retains the last good Scene. Stale results are
discarded without cache insertion, view mutation, notice replacement, or ProjectionVersion
advance. Cancellation remains an optional optimization.
