# Mirabile architecture consolidation

## Goal state

- Base: `63f081b3c9b0becf5238bdbe2cf3b6964bf09f85` (`origin/main`)
- Branch: `goal/mirabile-consolidation`
- Worktree: `/home/emmy/worktrees/mirabile-consolidation`
- Current phase: 3 - authority/lifetime model and WorkspaceDocument/WorkspaceSession
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

## Validation status

- Baseline identity/worktree checks: passed.
- Phase 1: `cargo fmt --all -- --check`, `cargo test --workspace` (99 tests), strict workspace Clippy, `mirabile-web` WASM check, XALEN dependency guard, browser IndexedDB/Worker contract, and `git diff --check` passed.
- Phase 2: documentation link targets inspected; `cargo fmt --all -- --check`, `cargo test -p mirabile-app` (27 tests), and `git diff --check` passed.
- Phase-focused checks: phase 3 onward pending.
- Full local verification: pending.

## Deferred work

- GitHub repository rename and local remote update occur only after goal-branch review and merge.
- Full preferences UI, session crash recovery, chart-editor UX, shared-record editing UX, and multi-session UI remain post-goal work.

## Blockers

- None.

## Final architectural state

Pending implementation. This section will summarize the delivered workspace/session, startup, draft/save, application decomposition, configuration provenance, validation, conformance, and web-module state.
