# State and persistence

## Authority classes

| Class | Representation now | Persistence rule |
| --- | --- | --- |
| Canonical (C) | Typed `CanonicalResource` envelopes | Revisioned repository; portable JSON |
| Derived (D) | Snapshots, analysis, layouts, scenes, effective values | Recomputed from inputs |
| Workspace (W) | `Workspace` resource | Portable/revisioned independently of chart library |
| Ephemeral (E) | `EditorState<T>` and Leptos interaction signals | Not committed until an explicit command |
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

The milestone browser UI opens IndexedDB once, retains a cloneable repository handle, seeds or
reloads the demonstration AspectSet, and reuses that handle for Save. Editor controls cannot accept
edits until startup reaches `Ready`. An open failure enters `Error` with Retry and never falls back
to implicit session/in-memory persistence. Operation epochs plus base-revision checks prevent an
older startup or save callback from overwriting newer state.

Repository writes accept a fully versioned resource and reject missing, duplicate, skipped, or stale revisions. Sync will later append operations after a successful local transaction; ordinary local writes must not wait on a server.

Portable JSON serializes only validated schema-v1 resource envelopes and typed payloads. Import
probes `resource.schema_version` before typed decoding and rejects every non-v1 value without
attempting to decode it as v1. IndexedDB store names, tombstone envelopes, device state, sync heads,
cache indexes, server tokens, and machine paths are not portable fields.

## Draft lifecycle

`EditorState<T>` transitions through clean, dirty, saving, and conflict states. Editing clones the canonical payload into a draft. Preview resolution chooses the draft while dirty/saving/conflicted. Save creates the next canonical revision only after the repository accepts it. Cancel drops the draft and resolves the canonical resource again.

Draft recovery in IndexedDB, cross-tab notifications/locks, OPFS caches, cloud synchronization, encrypted envelopes, and archive bundles are documented extension points and are intentionally deferred.

## Browser repository contract

`scripts/test-browser.sh` uses the checked-in feature-gated harness, Trunk, Python, and an isolated
headless Chromium profile. It covers create/get/save/history, two database handles, stale save and
delete conflicts, tombstone reads, permanent stable IDs, and transaction rollback after a forced
history-key collision. Passing requires the machine-readable `ASTRA_BROWSER_CONTRACT:PASS` DOM
marker; this is local validation, not hosted CI.
