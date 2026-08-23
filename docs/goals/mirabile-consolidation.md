# Mirabile architecture consolidation

## Goal state

- Base: `63f081b3c9b0becf5238bdbe2cf3b6964bf09f85` (`origin/main`)
- Branch: `goal/mirabile-consolidation`
- Worktree: `/home/emmy/worktrees/mirabile-consolidation`
- Current phase: Complete - final workspace/session correction validated for review
- Delivery: unmerged goal branch; external repository rename remains post-merge

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
- Mirabile-owned calculation/cache and Worker/browser marker identities are renamed. Serialized `WorkerProtocolVersion::CURRENT` remains version 3 and the readiness-marker contract remains version 1; branding alone changed neither.
- Third-party XALEN identities, exact Git pin, license text, notice assets, and opaque UUIDs remain unchanged.
- Source metadata will target `gracee3/mirabile`; the external GitHub repository rename is explicitly deferred until after review and merge.

## Implementation notes

- The base checkout was clean, `HEAD` and `origin/main` matched the required SHA, and implementation is isolated from the historical repository path `/home/emmy/astra`. This path is retained because it is the actual primary checkout, not product identity.
- The initial audit covers package/crate names, imports, docs, HTML, accessibility strings, browser/Worker markers, scripts, storage identity, calculation fingerprints, fixtures, notices, comments, and metadata.
- Phase 1 renamed all five packages and four crate directories, all Rust imports and Mirabile-owned provenance types/fields, repository metadata, the default IndexedDB identity, calculation/cache identities, Worker/browser markers, scripts, UI text, notices wrapper prose, and checked-in metadata.
- The serialized Worker protocol remains version 3 because its contract did not change; the project-owned readiness marker remains its existing version 1. Mirabile-owned cache keys intentionally change through the renamed engine/backend identities.
- Final tracked-source residual audit: the only case-insensitive matches are four classified lines in this document. Three describe the old IndexedDB reset boundary or actual primary checkout path `/home/emmy/astra`; the fourth is the old repository selector required by the post-merge rename command. The worktree `.git` administrative file also points to the primary checkout. None is an active product/codename identity; no incomplete rename remains. Third-party XALEN names and opaque UUIDs were unchanged.
- Phase 2 replaced the foundation-era README narrative with the real local-first Application, IndexedDB, Worker, provider-neutral contract, pinned XALEN default, current limitations, and local-only validation story. The original implementation plan is explicitly historical.
- Eight accepted ADRs now cover record/definition separation, revisioned resources, binding modes, application/read-model authority, document/session lifetimes, calculation isolation, XALEN/Swiss distribution boundaries, and pre-MVP compatibility.
- Phase 3 replaced the mixed canonical `Workspace` payload with `WorkspaceDocument`: saved chart-definition membership/order, durable views/slot assignments, workspace bindings, and promoted display overrides only.
- Application-owned `WorkspaceSession` now holds the document working copy plus active/selected chart, active view, draft chart-slot overlays, temporary view overrides, backing revision, and dirty state. Navigation and draft preview never dirty or persist the document.
- Workspace commands target the application-selected session and no longer require a canonical workspace ID. Opening/closing saved charts, saved-chart slot assignment, workspace binding changes, and promotion mark the working document dirty. `SaveWorkspace` creates revision one for `Unsaved` backing and alone writes later canonical revisions.
- Draft chart/view assignments remain strictly session-side and drive effective projection/calculation. Atomic chart save promotes matching assignments only after the instance has a canonical `ChartDefinition`; cancel discards them. A durable-only pre-save validator rejects every unknown or draft chart instance.
- The authority/lifetime model is documented independently: Canonical/Derived authority is separate from Library/Saved workspace/Session/Draft/Cache/Sync metadata lifetime.
- Phase 4 removed automatic canonical demo installation. `StartupPolicy` now models restore, Current Transits, blank, and explicit workspace opening; restore currently falls back to Current Transits because recovery is intentionally deferred.
- Empty memory and IndexedDB repositories initialize successfully and remain canonically empty. The explicit `demo_resources()` bundles are loaded only by tests or an eventual user-facing demo command.
- Fresh Current Transits is an unsaved session `ChartDraft` using the browser/system UTC clock, tropical geocentric positions, the supported Sun-through-Jupiter set, a single wheel, no houses, no geolocation request, and no asserted location.
- `ChartRecord::location`, provider-neutral numeric calculation location, `CalculationValue` location, and snapshot display location are optional. Geocentric no-house positions accept absence; topocentric positions, houses/angles, and local mean time fail with typed location-required errors.
- `CalculationEngine::resolve` prepares payload-only semantics independently of resource identity. Canonical calculations attach resource revisions afterward; draft calculations attach an explicitly non-canonical `SnapshotContext`.
- Phase 5 completed the `ChartDraft` lifecycle: start without persistence, assign to a view for payload-only preview, cancel without writes, or save to distinct revision-one `ChartRecord` and `ChartDefinition` resources.
- `ResourceRepository::create_batch` is the narrow local atomicity primitive. Memory prevalidates and preflights before mutation; IndexedDB uses one current-plus-history read-write transaction. Failure retains the application draft and publishes neither half.
- Successful chart save replaces the session draft with the same instance as a saved definition reference, promotes its session slot assignments, updates the library, preserves the record/definition boundary, and dirties workspace membership for an explicit workspace save. Multiple future definitions may still share one record.
- Phase 6 retained the public `RealApplication<R, C>` facade while splitting private responsibilities into catalog, hydration, workspace, editing, calculation, configuration, projection, and state modules. Existing async observation, latest-wins, last-good Scene, startup, draft, and persistence behavior is unchanged.
- Phase 7 replaced the conflated `ResolutionLayer` with independent `ConfigurationLayer` precedence and `ValueSource` material provenance. Follow/Pinned/Inline semantics and exact resolved revisions remain explicit.
- Core `DomainValidate` remains one-object structural validation. Application referential validation resolves bindings and checks canonical chart sources, session identities, and effective view-slot assignments during hydration and before workspace command state becomes authoritative. Persistence repeats a durable-only referential check with no access to draft overlays.
- Phase 8 added one reusable scenario suite that runs against both `MockApplication` and `RealApplication`: initialize/settle, projection monotonicity, activation/selection, open/close repair, workspace dirty/save, temporary override promotion, and last-good refresh behavior.
- The Leptos source now has explicit shell, async dispatcher, library, workspace rail, view host, and inspector modules. Normal presentation remains dependent on `mirabile-app`; the Real conformance fixture uses `mirabile-store` only as a native dev-dependency.
- Phase 9 added executable `scripts/check.sh` and `scripts/verify.sh`. The fast command runs formatting, workspace tests, strict Clippy, and staged/unstaged diff checks; the full command enumerates every required package, XALEN, native/WASM, Trunk main/Worker, dependency/license, notice, Chromium, and diff check with prerequisite diagnostics.
- Final review correction made fresh unsaved sessions first-saveable, removed canonical workspace identity from session commands, isolated draft slot assignments in session overlays, promoted those overlays only after atomic chart save, and added a durable-only pre-save referential gate. The unimplemented plural startup policy was removed rather than silently truncating requested workspace IDs.

## Validation status

- Baseline identity/worktree checks: passed.
- Phase 1: `cargo fmt --all -- --check`, `cargo test --workspace` (99 tests), strict workspace Clippy, `mirabile-web` WASM check, XALEN dependency guard, browser IndexedDB/Worker contract, and `git diff --check` passed.
- Phase 2: documentation link targets inspected; `cargo fmt --all -- --check`, `cargo test -p mirabile-app` (27 tests), and `git diff --check` passed.
- Phase 3: `cargo test --workspace` (100 tests), strict workspace Clippy, Chromium IndexedDB/Worker reload contract with explicit workspace save, formatting, and `git diff --check` passed.
- Phase 4: package suite (105 native tests, including XALEN-enabled coverage), strict workspace Clippy, engine/web WASM checks, Chromium empty-IndexedDB plus explicit-demo reload/Worker contract, formatting, and `git diff --check` passed.
- Phase 5: store/app/web package tests (56 tests), strict workspace Clippy, Memory batch rollback, application draft-retention failure, IndexedDB forced mid-batch rollback, IndexedDB application chart save, Chromium Worker/reload contract, formatting, and `git diff --check` passed.
- Phase 6: `mirabile-app` checks and 33 tests, strict package Clippy, formatting, workspace tests, Chromium browser contract, and `git diff --check` passed.
- Phase 7: core/app focused tests (52 tests), workspace tests (110 tests), strict workspace Clippy, web WASM check, Chromium IndexedDB/Worker contract, formatting, and `git diff --check` passed.
- Phase 8: shared Mock/Real conformance scenarios, workspace tests (112 tests), strict workspace Clippy, web WASM check, Chromium IndexedDB/Worker contract, formatting, and `git diff --check` passed.
- Phase 9: shell syntax checks, `./scripts/check.sh` (112 tests plus formatting, strict Clippy, and both diff checks), script coverage review, and executable-bit inspection passed.
- Phase 10: current `origin/main` reverified at the required base; package/dependency topology, full branch diff, third-party notice changes, hosted-CI absence, repository metadata, package names, and residual identity were audited. Authenticated `gh repo rename` syntax was verified without executing the rename.
- Final correction focused validation: `mirabile-app` (40 default tests), `mirabile-web` (16 tests), the 118-test XALEN-enabled workspace suite, strict workspace Clippy, formatting, expanded Chromium first-save/promotion/reload contract, and diff checks passed.
- Full local verification after the correction: `./scripts/verify.sh` passed all package tests, the 118-test workspace suite, XALEN-enabled tests and known answers, strict Clippy, provider-neutral and XALEN native/WASM checks, web WASM, Trunk main plus Worker build, XALEN dependency/license guard, notice-asset comparisons, Chromium IndexedDB/reload/atomicity/Worker contract, and staged/unstaged diff checks.

## Deferred work

- GitHub repository rename and local remote update occur only after goal-branch review and merge.
- Full preferences UI, session crash recovery, chart-editor UX, shared-record editing UX, and multi-session UI remain post-goal work.
- Before the MVP schema freeze, revisit the public schema/version promise, supported development-data migration window, startup/session-recovery preference contract, default-location policy, and shared-`ChartRecord` edit/copy UX.

## Post-merge repository transition

After review and a fast-forward merge to `main`, verify `main`, then run:

```bash
gh repo rename mirabile --repo gracee3/astra --yes
git remote set-url origin git@github.com:gracee3/mirabile.git
git fetch origin
git remote -v
gh repo view gracee3/mirabile --json nameWithOwner,url
git ls-remote --exit-code origin refs/heads/main
git rev-parse HEAD origin/main
```

Do not run the external rename before the reviewed branch is merged. GitHub redirects the old URL,
but local remotes should still be updated explicitly and the final main SHA compared.

## Blockers

- None.

## Final architectural state

- Identity: Mirabile owns five packages (`mirabile-core`, `mirabile-engine`, `mirabile-store`, `mirabile-app`, `mirabile-web`), database `mirabile`, and Mirabile calculation/Worker/browser identities. Source metadata targets `gracee3/mirabile`.
- State: canonical `WorkspaceDocument` contains saved chart-definition membership/order, durable views/saved-chart slots, workspace bindings, and promoted display configuration. Application-owned `WorkspaceSession` contains its working document, backing revision, active/selected chart, active view, drafts, draft slot overlays, temporary overrides, and dirty state. Durable pre-save validation cannot see or serialize draft overlays.
- Startup: one Application remains above repositories, catalog, runtime, available documents, and sessions. Empty storage is valid; default restore falls back to an ephemeral locationless Current Transits session and demo resources require explicit loading. An unsaved session can use library charts and creates a new `WorkspaceDocument` revision one through explicit Save Workspace.
- Charts: `ChartDraft` is non-canonical and calculates through payload semantics before identity/context is attached. Atomic `create_batch` saves distinct revision-one record and definition resources or neither, then turns the same session instance into a saved chart, promotes its session slot overlays, and dirties workspace membership.
- Configuration and validation: effective values carry independent `ConfigurationLayer` and `ValueSource`. Core performs structural one-object validation; app hydration and candidate workspace commands perform catalog/session referential validation.
- Application and presentation: public `RealApplication<R, C>` and frozen observation/latest-wins/last-good semantics remain intact behind responsibility modules. Shared scenarios constrain Mock and Real. Web source is separated into shell, dispatcher, library, rail, view host, and inspector modules.
- Development: `scripts/check.sh` is the fast local loop and `scripts/verify.sh` is the complete handoff gate. No hosted CI was added.
