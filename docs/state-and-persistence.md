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

`ResourceRepository` provides create, optimistic revision save, live current read, state-aware head/read-history operations, versioned delete, and typed listing. `MemoryRepository` retains every live revision and tombstone for deterministic tests. `IndexedDbRepository` stores current state and revision history in the existing `resources` and `resource_revisions` object stores and uses the resource's stable ID as the logical key.

Deletion writes `ResourceTombstone { id, kind, revision, deleted_at }` as the next revision to both
stores in one transaction. Ordinary `get` and `list` hide tombstones; `get_head` and
`get_revision` expose `ResourceState::Deleted`. Saves and recreation of a deleted stable ID are
rejected, while every earlier live revision remains readable. Existing live v1 resources retain
their raw JSON encoding. Only tombstones use an additive tagged storage envelope, so no IndexedDB
migration or store rename is required.

`RealApplication::initialize()` opens IndexedDB through a lazy repository source, retains the
cloneable repository handle, idempotently ensures the current explicit demo bundle, hydrates the
full library and `WorkspaceDocument`, creates a `WorkspaceSession`, and queues the first view calculation. Editor controls cannot accept edits
until startup reaches `Ready`. An open/hydration failure enters `Error` with initialization context
and never falls back to implicit session/in-memory persistence.

Opening/closing saved charts, slot assignment, workspace bindings, and promoted display overrides
change the session's durable document projection and mark it dirty. They do not write immediately.
`SaveWorkspace` explicitly saves the next `WorkspaceDocument` revision. Activating/selecting charts,
changing the active view, and temporary display overrides are session-only and never dirty the
document. Promotion copies a temporary override into the durable view configuration and marks the
document dirty. Reload restores only the last explicitly saved document; current navigation and
unpromoted overrides are intentionally lost. `ProjectionVersion` and calculation cache remain
application-instance-local and are never written to IndexedDB.

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

The same harness also runs an isolated real-application database. One `RealApplication` hydrates,
opens and activates Chart B, previews and commits Standard AspectSet revision 2, then is dropped. A
second instance opens the same database and must restore Chart B, the committed AspectSet revision,
the persisted `WorkspaceDocument`, and a newly reconstructed fresh Scene. The unique database name and
temporary Chromium profile isolate this lifecycle from normal user data; profile cleanup removes
the test database after the run.
