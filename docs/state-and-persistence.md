# State and persistence

## Authority classes

| Class | Representation now | Persistence rule |
| --- | --- | --- |
| Canonical (C) | Typed `CanonicalResource` envelopes | Revisioned repository; portable JSON |
| Derived (D) | Snapshots, analysis, layouts, scenes, effective values | Recomputed from inputs |
| Workspace (W) | `Workspace` resource | Portable/revisioned independently of chart library |
| Ephemeral (E) | Application resource draft and Leptos form buffers | Not committed until an explicit command |
| Cache (K) | Keyed engine results; memory in milestone | Disposable; no unique user information |
| Sync (S) | `SyncHead`/`SyncOperation` infrastructure types | Separate store; never serialized into resources |

Device-local `WorkspaceUiState` is not part of a portable `Workspace`.

## Repository contract

`ResourceRepository` provides create, optimistic revision save, live current read, state-aware head/read-history operations, versioned delete, and typed listing. `MemoryRepository` retains every live revision and tombstone for deterministic tests. `IndexedDbRepository` stores current state and revision history in the existing `resources` and `resource_revisions` object stores and uses the resource's stable ID as the logical key.

Deletion writes `ResourceTombstone { id, kind, revision, deleted_at }` as the next revision to both
stores in one transaction. Ordinary `get` and `list` hide tombstones; `get_head` and
`get_revision` expose `ResourceState::Deleted`. Saves and recreation of a deleted stable ID are
rejected, while every earlier live revision remains readable. Existing live v1 resources retain
their raw JSON encoding. Only tombstones use an additive tagged storage envelope, so no IndexedDB
migration or store rename is required.

`RealApplication::initialize()` opens IndexedDB through a lazy repository source, retains the
cloneable repository handle, idempotently ensures the small canonical bootstrap, hydrates the full
library and Workspace, and queues the first view calculation. Editor controls cannot accept edits
until startup reaches `Ready`. An open/hydration failure enters `Error` with initialization context
and never falls back to implicit session/in-memory persistence.

Open/close/activate/select chart, active-view, slot assignment, and AspectSet-binding changes save
the next Workspace revision before replacing the authoritative projection. Reload therefore
restores open charts, active and selected chart IDs, active view, slot assignments, and the
AspectSet binding. ProjectionVersion and calculation cache remain application-instance-local and
are never written to IndexedDB.

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
the persisted Workspace, and a newly reconstructed fresh Scene. The unique database name and
temporary Chromium profile isolate this lifecycle from normal user data; profile cleanup removes
the test database after the run.
