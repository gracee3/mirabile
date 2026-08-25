# State and persistence

## Pre-MVP compatibility policy

Until the first explicitly declared Mirabile MVP schema freeze, persisted schemas are development
formats and may change incompatibly. Migration is appropriate when it is cheap and useful to
current development, but compatibility with every development format is not guaranteed. Old data
must fail clearly when incompatible; resetting development IndexedDB data is acceptable. The
current `SchemaVersion::V1` therefore describes a development encoding, not a permanent public
compatibility promise.

The future public compatibility contract begins only with an explicit schema-freeze decision. A
generic migration framework will not be built merely to preserve pre-MVP design choices.

## Authority and lifetime

Authority and lifetime are independent dimensions:

| State | Authority | Scope/lifetime | Persistence |
| --- | --- | --- | --- |
| `ChartRecord` | Canonical | Library | Revisioned resource |
| `WorkspaceDocument` | Canonical | Saved workspace | Revisioned resource |
| active/selected chart and active view | Canonical application state | Session | Not portable |
| Aspect Set or chart draft | Canonical working intent | Draft | Explicit save only |
| `CalculationValue`, analysis, layout, `Scene` | Derived | Cache | Disposable |
| `ProjectionVersion` | Canonical application ordering | Session/runtime | Never persisted |
| `SyncHead`/`SyncOperation` | Canonical sync metadata | Sync metadata | Separate store |

Canonical means authoritative user/application intent; it does not imply every value is portable.
Derived state can have cache or runtime lifetime. Device-local `WorkspaceUiState` and future session
recovery remain outside portable `WorkspaceDocument`.

## Repository contract

`ResourceRepository` provides create, optimistic revision save, atomic create/save batches, live
current read, state-aware head/read-history operations, versioned delete, and typed listing.
`MemoryRepository` retains every live revision and tombstone for deterministic tests.
`IndexedDbRepository` stores current state and revision history in the existing `resources` and
`resource_revisions` object stores and uses the resource's stable ID as the logical key.

`AtomicSaveBatch` contains unique revision expectations, including compare-only expectations, and
the changed envelopes to publish. Implementations validate the complete batch and collect
identity-specific conflicts before publishing anything. Memory preflights before map mutation;
IndexedDB performs every expectation read, current-head write, and history insertion in one
transaction across both stores. Every accepted revision remains in history. Saved-chart editing
always expects both its Record and Definition bases but writes only the changed components.

Deletion writes `ResourceTombstone { id, kind, revision, deleted_at }` as the next revision to both
stores in one transaction. Ordinary `get` and `list` hide tombstones; `get_head` and
`get_revision` expose `ResourceState::Deleted`. Saves and recreation of a deleted stable ID are
rejected, while every earlier live revision remains readable. Existing live v1 resources retain
their raw JSON encoding. Only tombstones use an additive tagged storage envelope, so no IndexedDB
object-store migration or rename is required inside the `mirabile` development database. The
separate product-identity database change intentionally resets the earlier development database.

`RealApplication::initialize()` opens IndexedDB through a lazy repository source, retains the
cloneable repository handle, hydrates whatever canonical resources exist, applies `StartupPolicy`,
creates a `WorkspaceSession`, and queues calculation only when the session has active content.
An empty repository is valid: the default fallback creates an ephemeral, locationless Current
Transits session without writing canonical demo resources. The demo bundle is loaded only by
explicit tests or a future user command. Editor controls cannot accept edits until startup reaches
`Ready`. An open/hydration failure enters `Error` with initialization context and never falls back
to implicit session/in-memory persistence.

Opening/closing saved charts, saved-chart slot assignment, workspace bindings, and promoted display
overrides change the session's durable document projection and mark it dirty. They do not write
immediately. Draft chart slot assignments live in `WorkspaceSession::draft_chart_assignments` and
override the effective view without touching or dirtying `WorkspaceDocument`. Atomic chart save
promotes those assignments only after the same instance becomes a saved chart; cancel removes them.

The working workspace title lives in `WorkspaceSession`; canonical title metadata remains on the
`ResourceEnvelope`, outside `WorkspaceDocument`. For `Unsaved` backing, `SaveWorkspace` creates a
new `WorkspaceDocument` envelope at revision one. Later
saves require a dirty document and write the next revision. A durable-only referential check runs
immediately before persistence and rejects any slot assignment not backed by the document's saved
chart membership. Activating/selecting charts, changing the active view, draft overlays, and
temporary display overrides are session-only. Reload restores only the last explicitly saved
document; navigation and unpromoted overlays are intentionally lost. `ProjectionVersion` and the
calculation cache remain application-instance-local and are never written to IndexedDB.

Creating/opening a workspace cannot silently discard local state. When chart/resource editors,
temporary display, or workspace changes would be lost, the application projects an explicit switch
decision. Save-and-switch writes only the workspace and is disabled if another editor must first be
resolved. Discard restores the saved envelope or a fresh unsaved workspace. The stable demo bundle
is created only by explicit `LoadDemoBundle`; compatible existing heads remain untouched and any
incompatible collision rejects the complete create batch.

Repository writes accept a fully versioned resource and reject missing, duplicate, skipped, or stale revisions. Sync will later append operations after a successful local transaction; ordinary local writes must not wait on a server.

Portable JSON serializes only validated schema-v1 resource envelopes and typed payloads. Import
probes `resource.schema_version` before typed decoding and rejects every non-v1 value without
attempting to decode it as v1. IndexedDB store names, tombstone envelopes, device state, sync heads,
cache indexes, server tokens, and machine paths are not portable fields.

## Draft lifecycle

The application projection transitions through clean, dirty, saving, and conflict states. Editing
clones the canonical payload into a draft. Preview resolution chooses the draft while
dirty/saving/conflicted. Save creates the next canonical revision only after the repository accepts
it. Cancel resolves the current canonical resource again. Repository conflicts refresh the remote
canonical head for library projection but retain the local draft and its original base revision.

Draft recovery in IndexedDB, cross-tab notifications/locks, OPFS caches, cloud synchronization, encrypted envelopes, and archive bundles are documented extension points and are intentionally deferred.

## Browser repository contract

`scripts/test-browser.sh` uses the checked-in feature-gated harness, Trunk, Python, and an isolated
headless Chromium profile. It covers create/get/save/history, two database handles, stale save and
delete conflicts, tombstone reads, permanent stable IDs, and transaction rollback after a forced
history-key collision. Passing requires the machine-readable `MIRABILE_BROWSER_CONTRACT:PASS` DOM
marker; this is local validation, not hosted CI.

The same harness also runs an isolated real-application database. It first saves a fresh Current
Transits session as workspace revision one without leaking its draft assignment, atomically saves
the chart, promotes and saves the assignment as revision two, and reloads it. After explicitly
loading the demo bundle, one `RealApplication` hydrates,
opens and activates Chart B, previews and commits Standard AspectSet revision 2, then is dropped. A
second instance opens the same database and must restore Chart B, the committed AspectSet revision,
the persisted `WorkspaceDocument`, and a newly reconstructed fresh Scene. The unique database name and
temporary Chromium profile isolate this lifecycle from normal user data; profile cleanup removes
the test database after the run.

The workbench E2E harness uses a separate validated `mirabile-workbench-e2e-*` database per run.
The feature-gated peer `RealApplication` can lazily open that same database to prove independent
CAS conflicts through typed application actions. This is test-only behavior and never changes the
normal `mirabile` database.
