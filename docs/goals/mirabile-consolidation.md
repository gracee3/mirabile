# Mirabile architecture consolidation

## Goal state

- Base: `63f081b3c9b0becf5238bdbe2cf3b6964bf09f85` (`origin/main`)
- Branch: `goal/mirabile-consolidation`
- Worktree: `/home/emmy/worktrees/mirabile-consolidation`
- Current phase: 8 - application conformance and web modularization
- Delivery: unmerged goal branch; push only after the complete validation phase

## Frozen architecture

- `ChartRecord` is a factual/source assertion; `ChartDefinition` owns calculation semantics.
- Canonical resources are revisioned and locally authoritative; derived computation is separate.
- Resource bindings remain Follow, Pinned, or Inline.
- `CalculationValue` remains separate from `SnapshotContext`.
- One `Application` owns shared repositories, resources, runtime, preferences, workspaces, and sessions; it accepts typed `AppIntent` and projects read models plus `Scene`.
- `ProjectionVersion` is instance-local ordering. `snapshot()` does not execute work; `wait_for_update()` is non-consuming observation.
- Calculation is latest-wins and last-good `Scene` survives refresh/failure behind the provider-neutral Worker/runtime contract.
- XALEN is adapted to Mirabile-owned types. Swiss Ephemeris remains optional and distribution-isolated.
- No astrology feature expansion belongs to this goal.

## Completed decisions

- Until an explicit Mirabile MVP schema freeze, persisted formats are development schemas and may change incompatibly. Old development data may fail clearly or be reset; `SchemaVersion::V1` is not a public compatibility promise.
- The IndexedDB identity will change from historical `astra` to `mirabile`. This classified historical reference documents the reset boundary. No permanent legacy migration will be added.
- Mirabile-owned calculation/cache and Worker/browser marker identities will be renamed without changing the Worker protocol version solely for branding.
- Third-party XALEN identities, exact Git pin, license text, notice assets, and opaque UUIDs remain unchanged.
- Source metadata will target `gracee3/mirabile`; the external GitHub repository rename is explicitly deferred until after review and merge.

## Implementation notes

- The base checkout was clean, `HEAD` and `origin/main` matched the required SHA, and implementation is isolated from the historical repository path `/home/emmy/astra`. This path is retained because it is the actual primary checkout, not product identity.
- The initial audit covers package/crate names, imports, docs, HTML, accessibility strings, browser/Worker markers, scripts, storage identity, calculation fingerprints, fixtures, notices, comments, and metadata.
- Phase 1 renamed all five packages and four crate directories, all Rust imports and Mirabile-owned provenance types/fields, repository metadata, the default IndexedDB identity, calculation/cache identities, Worker/browser markers, scripts, UI text, notices wrapper prose, and checked-in metadata.
- The Worker protocol remains V1 because the serialized contract did not change. Mirabile-owned cache keys intentionally change through the renamed engine/backend identities.
- Residual name audit after phase 1: only the two classified historical references in this document remain (`astra` as the old IndexedDB name and `/home/emmy/astra` as the actual primary checkout path). Third-party XALEN names and opaque UUIDs were unchanged.
- Phase 2 replaced the foundation-era README narrative with the real local-first Application, IndexedDB, Worker, provider-neutral contract, pinned XALEN default, current limitations, and local-only validation story. The original implementation plan is explicitly historical.
- Eight accepted ADRs now cover record/definition separation, revisioned resources, binding modes, application/read-model authority, document/session lifetimes, calculation isolation, XALEN/Swiss distribution boundaries, and pre-MVP compatibility.
- Phase 3 replaced the mixed canonical `Workspace` payload with `WorkspaceDocument`: saved chart-definition membership/order, durable views/slot assignments, workspace bindings, and promoted display overrides only.
- Application-owned `WorkspaceSession` now holds the document working copy plus active/selected chart, active view, temporary view overrides, backing revision, and dirty state. Navigation never dirties or persists the document.
- Opening/closing saved charts, slot assignment, workspace binding changes, and promotion mark the working document dirty. `SaveWorkspace` alone writes the next canonical revision. A temporary hidden-point override proves session-only behavior and promotion into durable configuration.
- The authority/lifetime model is documented independently: Canonical/Derived authority is separate from Library/Saved workspace/Session/Draft/Cache/Sync metadata lifetime.
- Phase 4 removed automatic canonical demo installation. `StartupPolicy` now models restore, Current Transits, blank, and explicit workspace opening; restore currently falls back to Current Transits because recovery is intentionally deferred.
- Empty memory and IndexedDB repositories initialize successfully and remain canonically empty. The explicit `demo_resources()` bundles are loaded only by tests or an eventual user-facing demo command.
- Fresh Current Transits is an unsaved session `ChartDraft` using the browser/system UTC clock, tropical geocentric positions, the supported Sun-through-Jupiter set, a single wheel, no houses, no geolocation request, and no asserted location.
- `ChartRecord::location`, provider-neutral numeric calculation location, `CalculationValue` location, and snapshot display location are optional. Geocentric no-house positions accept absence; topocentric positions, houses/angles, and local mean time fail with typed location-required errors.
- `CalculationEngine::resolve` prepares payload-only semantics independently of resource identity. Canonical calculations attach resource revisions afterward; draft calculations attach an explicitly non-canonical `SnapshotContext`.
- Phase 5 completed the `ChartDraft` lifecycle: start without persistence, assign to a view for payload-only preview, cancel without writes, or save to distinct revision-one `ChartRecord` and `ChartDefinition` resources.
- `ResourceRepository::create_batch` is the narrow local atomicity primitive. Memory prevalidates and preflights before mutation; IndexedDB uses one current-plus-history read-write transaction. Failure retains the application draft and publishes neither half.
- Successful chart save replaces the session draft with the same instance as a saved definition reference, updates the library, preserves the record/definition boundary, and dirties workspace membership for an explicit workspace save. Multiple future definitions may still share one record.
- Phase 6 retained the public `RealApplication<R, C>` facade while splitting private responsibilities into catalog, hydration, workspace, editing, calculation, configuration, projection, and state modules. Existing async observation, latest-wins, last-good Scene, startup, draft, and persistence behavior is unchanged.
- Phase 7 replaced the conflated `ResolutionLayer` with independent `ConfigurationLayer` precedence and `ValueSource` material provenance. Follow/Pinned/Inline semantics and exact resolved revisions remain explicit.
- Core `DomainValidate` remains one-object structural validation. Application referential validation now resolves bindings and checks canonical chart sources, session identities, and resolved view-slot assignments during hydration and before workspace command state becomes authoritative.

## Validation status

- Baseline identity/worktree checks: passed.
- Phase 1: `cargo fmt --all -- --check`, `cargo test --workspace` (99 tests), strict workspace Clippy, `mirabile-web` WASM check, XALEN dependency guard, browser IndexedDB/Worker contract, and `git diff --check` passed.
- Phase 2: documentation link targets inspected; `cargo fmt --all -- --check`, `cargo test -p mirabile-app` (27 tests), and `git diff --check` passed.
- Phase 3: `cargo test --workspace` (100 tests), strict workspace Clippy, Chromium IndexedDB/Worker reload contract with explicit workspace save, formatting, and `git diff --check` passed.
- Phase 4: package suite (105 native tests, including XALEN-enabled coverage), strict workspace Clippy, engine/web WASM checks, Chromium empty-IndexedDB plus explicit-demo reload/Worker contract, formatting, and `git diff --check` passed.
- Phase 5: store/app/web package tests (56 tests), strict workspace Clippy, Memory batch rollback, application draft-retention failure, IndexedDB forced mid-batch rollback, IndexedDB application chart save, Chromium Worker/reload contract, formatting, and `git diff --check` passed.
- Phase 6: `mirabile-app` checks and 33 tests, strict package Clippy, formatting, workspace tests, Chromium browser contract, and `git diff --check` passed.
- Phase 7: core/app focused tests (52 tests), workspace tests (110 tests), strict workspace Clippy, web WASM check, Chromium IndexedDB/Worker contract, formatting, and `git diff --check` passed.
- Phase-focused checks: phase 8 onward pending.
- Full local verification: pending.

## Deferred work

- GitHub repository rename and local remote update occur only after goal-branch review and merge.
- Full preferences UI, session crash recovery, chart-editor UX, shared-record editing UX, and multi-session UI remain post-goal work.

## Blockers

- None.

## Final architectural state

Pending implementation. This section will summarize the delivered workspace/session, startup, draft/save, application decomposition, configuration provenance, validation, conformance, and web-module state.
