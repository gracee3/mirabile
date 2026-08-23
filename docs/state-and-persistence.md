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

`ResourceRepository` provides create, optimistic revision save, current read, historical revision read, delete, and typed listing. `MemoryRepository` retains every revision for deterministic tests. `IndexedDbRepository` stores current resources and revision history in separate object stores and uses the resource's stable ID as the logical key.

The milestone browser UI uses that IndexedDB adapter for the demonstration AspectSet. It seeds revision 1 on first use, reloads the current revision on later visits, and commits Save through `Command::SaveResourceDraft`. Chart records, definitions, workspaces, and other resource kinds serialize through the same repository format, but their browser editing workflows are not implemented yet.

Repository writes accept a fully versioned resource and reject missing, duplicate, skipped, or stale revisions. Sync will later append operations after a successful local transaction; ordinary local writes must not wait on a server.

Portable JSON serializes only resource envelopes and typed payloads. IndexedDB store names, device state, sync heads, cache indexes, server tokens, and machine paths are not portable fields.

## Draft lifecycle

`EditorState<T>` transitions through clean, dirty, saving, and conflict states. Editing clones the canonical payload into a draft. Preview resolution chooses the draft while dirty/saving/conflicted. Save creates the next canonical revision only after the repository accepts it. Cancel drops the draft and resolves the canonical resource again.

Draft recovery in IndexedDB, cross-tab notifications/locks, OPFS caches, cloud synchronization, encrypted envelopes, and archive bundles are documented extension points and are intentionally deferred.
