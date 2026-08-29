# Mirabile Comprehensive Control Cockpit

## Goal state

- Base: `b6712bc9ed5913238bc7535b11a1a2e155b54450` (`origin/main`, verified
  2026-08-28 after squash-merging PR #1)
- Branch: `goal/mirabile-control-cockpit`
- Worktree: `/home/emmy/mirabile` (the existing checkout; no additional worktree or Cargo target)
- Current phase: complete and squash-merged through PR #2
- Delivery: final feature head `bba5f89d3d8fcb1d7585a83be8e30a0aea80d4a4` was squash-merged as
  `8e7b610fb6afd8d72510cb1a0431d946a8c912d8`
- Final feature fast gate: `scripts/check.sh` passed 182 Rust tests and 6 Python tests, strict
  Clippy, formatting, and staged plus unstaged diff checks

## Frozen boundaries

- Preserve `AppReadModel` as the single application authority. The web layer dispatches typed
  intents and never writes repositories, accepts field paths or arbitrary JSON, or infers
  provider-specific behavior.
- Cover the ten real `CanonicalResource` variants. Reserved `ResourceKind` values without a
  canonical payload remain outside this sprint.
- Keep composite ChartRecord plus ChartDefinition writes atomic and retain CAS, IndexedDB history,
  tombstones, Follow/Pinned/Inline bindings, shared-record protection, and last-good output.
- Fold state, section search, Expand All, and Collapse All are local presentation state and are
  never persisted. Every control family remains mounted; unavailable controls carry a reason.
- Atlas/timezone resolution, expanded XALEN, bi-wheels, interpretation, query execution,
  non-wheel ViewObject rendering, and professional-wheel work remain deferred.
- Stable draft item identities for nested editable rows are application-owned and are stripped
  before canonical persistence.
- Automation snapshots may expose bounded semantic calculation tables but never complete Scene
  geometry.

## Delivery matrix

| Phase | State | Scope | Gate / evidence | Commit |
| --- | --- | --- | --- | --- |
| 1. Baseline and goal | Complete | Correct and merge PR #1; capture disk, normal application, console, display, and screenshot baseline | `./scripts/check.sh`; clean fast-forwarded `main`; normal non-automation XALEN initialization | `774d54c` |
| 2. Repository and inventory | Complete | Present/deleted heads; selected-resource revisions; inventories for all ten canonical resource types | 157 Rust tests; WASM check; IndexedDB browser contract; `./scripts/check.sh` | `85bfead` |
| 3. General typed drafts | Complete | Typed mutations, lifecycle/conflict projections, one draft per type, stable nested item IDs | 161 Rust tests; 5 Python tests; strict Clippy; `./scripts/check.sh` | `4aae865` |
| 4. Complete editors | Complete | Every modeled field is accounted for in the field ledger; typed nested projections, mutations, validation, macro actions, persistence, and visible journeys are implemented | Native and browser authoring coverage; `mirabile-control-cockpit-fields.md` | `8b746bb` through `6ceacf2` |
| 5. Bindings and outputs | Complete | Writable Follow/Pinned/Inline bindings, provider-neutral tables, provenance, parameter status, and last-good retention | Focused binding and last-good regressions | `8b2b44e` |
| 6. Cockpit composition | Complete | Document-height eight-section cockpit, sticky navigation/search/fold controls, and semantic addresses | Cockpit manifest plus four responsive captures | `534a277`, `057c5e3` |
| 7. History and deletion | Complete | Revision inspection, reference-aware two-step deletion, tombstones, stale-delete conflicts, and reload | Store/application tests plus browser reload journey | `8b2b44e`, `057c5e3` |
| 8. Handoff | Complete | Exact-head full gate, normal browser acceptance, evidence update, and authorized squash merge | T14 acceptance matrix; PR #2 merge `8e7b610` | `bba5f89`, PR #2 |

## Phase records

### Phase 1 - baseline and goal

- PR #1 was corrected on `chore/t14-baseline-hygiene` at `189a079`: the normal Trunk command
  clears `NO_COLOR`, and the PR body separates fresh validation from inherited workbench handoff
  evidence. Fresh `./scripts/check.sh` passed 155 Rust tests and 5 Python tests with strict Clippy,
  formatting, and both diff checks.
- PR #1 was squash-merged without deleting its branch. `origin/main` and local `main` were verified
  at `b6712bc9ed5913238bc7535b11a1a2e155b54450`; the remote hygiene branch remained at `189a079`.
- Host display snapshot: T14 internal `eDP-1`, 1920x1080 at scale 1. The normal, non-automation URL
  initialized Current Transits through the local Worker/XALEN path in a fresh Chromium profile.
- Before screenshot: `target/control-cockpit-before/normal-app-1920x1080.png` (captured viewport
  1920x937; intentionally untracked). DOM, Chromium, and server evidence are beside it.
- Console state: zero console errors or exceptions. Chromium reported one preload-integrity
  informational message, one Worker initialization deprecation message, and host-level Chromium
  OAuth/sandbox warnings.
- Disk checkpoint: 28 GiB free on `/`; the shared existing `target/` was 6.4 GiB.
- Elapsed: the warm fast gate completed in about 1.2 seconds; the normal build and capture completed
  in about 5.6 seconds.
- Blockers: none.
- Next action: add repository head/history queries and authoritative inventories for all ten
  canonical resource kinds.

### Phase 2 - repository and inventory

- SHA: `85bfeada7a155c40e17dddcda0451a5bc9722d80`.
- `ResourceRepository` now lists ordered present/deleted heads and complete ordered history for one
  stable identity. Memory and IndexedDB adapters implement the same contract; ordinary `list`
  continues to hide tombstones.
- `CanonicalResource::KINDS` is the authoritative ten-variant set and explicitly excludes the five
  reserved `ResourceKind` values without canonical payloads. Resource metadata accessors support
  uniform inventory projection.
- `AppReadModel` now carries ten inventory groups (including empty groups), metadata summaries,
  repository heads, and selected revision history. `SelectRepositoryResource` is a typed
  application intent; selection loads repository history and never exposes a web repository write.
- Tests: `cargo test --workspace` passed 157 Rust tests; the web WASM check passed; the IndexedDB
  reload/Worker browser contract passed with new head/history assertions; `./scripts/check.sh`
  passed in 9.43 seconds with 157 Rust tests, 5 Python tests, strict Clippy, formatting, and diff
  checks.
- Disk checkpoint: 25 GiB free on `/`; the shared existing `target/` was 8.5 GiB after WASM and
  browser builds.
- Elapsed: the broad native, WASM, and IndexedDB validation sequence completed in about one minute.
- Screenshot paths: Phase 1 before evidence remains current; no Phase 2 visual change.
- Blockers: none.
- Next action: replace the Aspect Set-specific editor contract with generalized typed resource
  drafts, lifecycle/conflict state, and stable nested draft item identities.

### Phase 3 - generalized typed drafts

- SHA: `4aae8653c787e619c9a60b75da2e94baf3b52cb6`.
- `ResourceDraftKind` and `ResourceMutation` cover exactly the ten canonical payload variants.
  Mutations delegate to typed per-resource enums and metadata mutations; no field path, JSON patch,
  or repository handle crosses the application boundary.
- The application maintains at most one draft per resource type. Selecting a different same-kind
  target while dirty requires Save or Cancel. Six independent types support new/edit/save/cancel;
  ChartRecord plus ChartDefinition, Aspect Set, and Workspace route to their existing atomic or
  session workflows instead of creating a second authority.
- Typed drafts project New/Creating/Clean/Dirty/Saving/Conflict state and structured CAS conflicts.
  Save is observable pending work; conflict and adapter failure retain the local draft.
- Notes, life events, wheel rings, view objects, and query nodes receive application-owned draft
  item identities. Reorder/update tests prove identities remain stable while canonical materialized
  values and JSON omit every draft ID.
- Tests: `./scripts/check.sh` passed in 27.07 seconds with 161 Rust tests, 5 Python tests, strict
  Clippy, formatting, and both diff checks. Focused tests cover create for each independent payload,
  edit/save/reload, CAS conflict retention, cancel, exact ten-kind coverage, and reordered rows.
- Disk checkpoint: 19 GiB free on `/`; the existing shared `target/` is 9.9 GiB. Heavy browser
  rebuilds are deferred until their phase gate so the final 15 GiB verification threshold remains
  observable.
- Screenshot paths: no visual change in this phase; Phase 1 before evidence remains current.
- Blockers: none.
- Next action: expose complete typed draft values and native editors for every modeled field while
  retaining the composite chart and workspace session paths.

### Cockpit/output checkpoint - phases 4 through 6 in progress

- The normal page is document-height and begins with an expanded eight-section cockpit. Its sticky
  search/navigation bar, Expand All, and Collapse All state remain local to the web adapter. Search
  filters section titles plus canonical resource titles and tags; all ten inventories stay mounted
  when no search is active.
- Always-mounted editors expose consistent title, description, and tags plus native controls for
  analysis switches/limits, theme colors, view page geometry, and query description. Query AST
  execution is explicitly deferred. Remaining per-row list builders and complete chart/workspace
  fields are not yet marked complete.
- `AppReadModel` now owns explicit `Live`, `Persisted`, `ReadOnly`, and reason-bearing `Unavailable`
  parameter coverage. It also projects provider-neutral point, house, angle, aspect, and calculation
  provenance rows. Successful calculation output is retained beside the last-good Scene and remains
  projected after refresh failure.
- Current screenshot: `target/control-cockpit-phase4/normal-app-1600x1000.png` (untracked; captured
  viewport 1600x857). Initial shell capture reported zero console errors.
- Focused validation: typed resource lifecycle and last-good semantic retention tests pass; WASM
  and strict Clippy are rerun at this checkpoint before commit.
- Blockers: none. Next action: complete reference-aware deletion, writable binding controls, and
  remaining typed payload builders before responsive browser journeys.

### Binding and deletion checkpoint - phases 5 and 7 in progress

- Every projected workspace binding has native resource, mode, and revision controls with stable
  slot-qualified addresses. Typed intents support Follow, exact-revision Pinned, and Inline-copy
  transitions for all three Point Set roles, Aspect Set, Analysis Profile, Theme, Wheel Template,
  and the active View Document. A focused application test preserves all three semantics and marks
  the workspace dirty without introducing another state authority.
- Repository selection now projects application-level references, active-editor blockers, the
  expected head revision, deletion availability, and confirmation state. Deletion requires two
  distinct typed actions; the second rechecks blockers and repository CAS before publishing a
  tombstone. Successful deletion keeps all revisions selected, while referenced and stale deletes
  are rejected in focused tests.
- Automation snapshots include bounded semantic point, house, angle, aspect, and provenance rows
  and still omit Scene geometry.
- The E2E runner accepts and range-checks `--viewport WIDTHxHEIGHT`. The new
  `cockpit-manifest-control` journey passed at 1366x768 and covers the cockpit navigation, all ten
  canonical resource families, and disabled deletion reasons.
- Screenshot: `target/workbench-e2e-cockpit-manifest-control.png` (untracked).
- Next action: complete remaining typed list builders, add history/delete browser reload coverage,
  capture the final responsive matrix, and run the final repository gates.

### Final handoff checkpoint

- SHA before the final evidence-only documentation commit:
  `057c5e353fd81b3c55e17ea6fbf3e1f83fa452cb`. The feature branch is pushed to
  `origin/goal/mirabile-control-cockpit`.
- `./scripts/check.sh` passes 164 Rust tests and 5 Python tests with formatting, strict Clippy,
  staged and unstaged diff checks. The focused XALEN suite includes the independent JPL/Swiss
  known-answer radix and passes as part of that gate.
- Browser journeys passing: `cockpit-manifest-control`, `resource-authoring-control`,
  `history-delete-control` including IndexedDB reload, `diagnostics-control`, and
  `workspace-lifecycle-control`. Existing journeys remain in `scripts/verify.sh`; the three new
  journeys are now part of that script.
- Normal non-automation acceptance initialized the Worker/XALEN application, waited for settlement,
  loaded the demo bundle through the native `workspace.load-demo` control, and reported zero
  console errors or uncaught exceptions. A data favicon prevents a browser-generated 404 from
  polluting console acceptance.
- Final untracked captures:
  - `target/control-cockpit-final/cockpit-1600x1000.png` (captured 1600x857)
  - `target/control-cockpit-final/cockpit-1366x768.png` (captured 1366x625)
  - `target/control-cockpit-final/cockpit-t14-1920x1080.png` (captured 1920x937)
  - `target/control-cockpit-final/cockpit-800x700.png` (captured 800x557)
  - `target/control-cockpit-final/normal-demo-t14-1920x1080.png`
- Disk checkpoint: 11,262,078,976 bytes free (10.49 GiB displayed from the exact byte count) and
  `target/` is 13,618,344,201 bytes. The required greater-than-15-GiB threshold is short by at least
  4,844,048,384 bytes (4.51 GiB), so `scripts/verify.sh` was intentionally not run and its complete
  matrix is not claimed.
- Persisted-only/deferred boundary: Query Definitions are authorable and preserve typed ASTs, but
  execution remains deferred. Non-wheel View Objects and derived chart recipes remain canonical
  persisted data without runtime rendering/calculation. The generalized contracts and stable nested
  IDs are implemented, while the cockpit currently exposes native builders for metadata, direct
  Point Set membership, Analysis Profile values, Wheel Template display/geometry, View Document
  page geometry, Theme colors, and Query description; additional nested object/query/recipe row
  builders remain follow-up UI depth and are not represented as runtime support.
- Feature PR: [#2, Build the comprehensive Mirabile control cockpit](https://github.com/gracee3/mirabile/pull/2).
  It is open as a draft, is not merged, targets `main`, and explicitly records the remaining nested
  builder depth instead of overstating phase 4 completeness.

## Acceptance checklist

- Native lifecycle coverage exists for every canonical resource type, including conflicts,
  reordering of stable nested rows, IndexedDB reload, and composite chart atomicity.
- Present/deleted heads, complete selected history, references, tombstones, reference blocking,
  and stale deletion are tested through repository and application boundaries.
- Every finite modeled option is visible. Every unavailable control has a nonempty authoritative
  reason, and every mutation/control family is registered in cockpit coverage.
- Browser journeys cover cockpit manifest, all resource editors with reload, complete chart and
  workspace fields, binding/output tables, history/deletion, diagnostics, macros, and existing
  scenarios.
- Final captures are untracked and include 1600x1000, 1366x768, the actual T14 viewport, and a
  viewport below 850 px. No pixel-golden correctness test is added.
- Final gates include `scripts/check.sh`, focused XALEN and independent known-answer coverage,
  diagnostics/workspace journeys, new cockpit journeys, normal non-automation initialization and
  demo load with zero console errors, and `scripts/verify.sh` while free space exceeds 15 GiB.

## Professional-wheel reconciliation

PR #2 intentionally delivers the comprehensive control-cockpit sprint in place of, and before,
the separately scoped professional-wheel sprint. It does not claim professional-wheel completion;
that work is explicitly deferred in the frozen boundaries above.

| Professional-wheel acceptance criterion | State in PR #2 | Reconciliation |
| --- | --- | --- |
| Real houses and angles | Data/output only; not professional wheel rendering | XALEN and the provider-neutral read model supply house cusps, Ascendant, and Midheaven when the chart requests a supported house system with a complete location. PR #2 adds cockpit tables for those values but does not add wheel cusp or angle geometry. |
| Zodiac and planet glyphs | Not delivered | The pre-existing Scene contract can carry glyph strings, but the current wheel layout still labels points with identifiers and does not implement the requested zodiac/planet glyph system. |
| Degree and minute labels | Not delivered | PR #2 adds decimal-degree semantic tables and editable Wheel Template display flags. It does not add degree/minute-formatted labels to professional wheel geometry. |
| Retrograde state | Data/output only; not professional wheel rendering | XALEN already calculates retrograde state and PR #2 projects it in the semantic point table. The wheel does not yet render a retrograde marker. |
| Aspect semantics | Partially present; professional presentation not delivered | Applying/orb/separation semantics and aspect segments exist, and PR #2 exposes bounded semantic rows. It does not add the requested professional aspect glyph, line-style, or visual hierarchy treatment. |
| Collision-aware placement | Not delivered | The current layout places every point label at a fixed radial offset from its longitude. It has no clustering, displacement, leader-line, or collision-resolution pass. |
| Wheel-dominant cockpit | Not delivered | PR #2 deliberately replaces the fixed workstation with an eight-section document-height control laboratory. The retained wheel is a preview, not the dominant professional-wheel surface. |

The nested object, query, recipe, chart-fact, workspace-composition, and macro builders are now
complete through typed application interfaces. That closes the cockpit scope while leaving the
professional-wheel criteria above truthfully deferred to the next branch.

Reconciliation snapshot on 2026-08-28:

- PR base and merge base: `b6712bc9ed5913238bc7535b11a1a2e155b54450` (`origin/main`).
- Feature head: `9d4d605d0f5769da991526ce0a5e0b4669e15225`; the local and remote feature
  refs matched before this documentation update, with ten feature commits and no divergence.
- Final implementation worktree was clean before this documentation update. PR #2 remains open,
  draft, and remotely reported `CLEAN`; it has not been merged.
- Architecture boundaries remain preserved: the PR does not modify `mirabile-engine`, the frozen
  architecture documents, or the calculation-provider interface. The web layer remains a typed
  intent adapter; repository access and provider-neutral projection remain in application/store
  layers under the single authoritative `AppReadModel`.
- Current disk snapshot: 17,970,290,688 bytes free on `/` (16.74 GiB), and `target/` is
  13,618,344,201 bytes (12.68 GiB). This newer free-space reading does not alter the historically
  correct final-gate record above: `scripts/verify.sh` was skipped when only 11,262,078,976 bytes
  were free.

## Final verification and merge state

- No implementation blocker or field-ledger gap is known. The completion contract is recorded in
  `mirabile-control-cockpit-fields.md` with projection, semantic address, typed mutation, macro
  selector, persistence, deletion, and browser evidence for every modeled field.
- The final code and browser-contract head before this evidence-only documentation update is
  `aaeeb6b6ed01e1eee0478371659efd37930b26a4`. Focused gates pass 94 `mirabile-app` tests and 22
  `mirabile-web` tests. The exact-head `scripts/check.sh` passes 182 Rust tests and 6 Python tests
  with formatting, strict Clippy, and both diff checks in 18.95 seconds after the narrow disk-gate
  rebuild.
- The successful full-gate preflight measured 18,588,459,008 available bytes and
  16,613,352,636 bytes in `target/`. The threshold was 18,253,611,008 bytes: 15 GiB plus the 2 GiB
  minimum focused-build allowance. The 334,848,000-byte margin passed. Only reproducible
  `mirabile-web` Cargo outputs were cleaned after process/path checks; user browser state and an
  unrelated active checkout were not touched.
- `scripts/verify.sh` passes in 290.06 seconds. It includes native, XALEN known-answer, WASM,
  IndexedDB/Worker, diagnostics, workspace, all completed builder, macro replay/topology failure,
  history/deletion, both CAS-conflict, and intentional expected-failure artifact coverage. The
  post-gate checkpoint was 17,547,632,640 available bytes with 16,898,185,358 bytes in `target/`.
- Normal non-automation acceptance exposed no automation bridge, initialized through Worker/XALEN,
  loaded the demo, opened the ChartRecord/radix, derived ChartDefinition, PointSet, AspectSet,
  AnalysisProfile, WheelTemplate, ViewDocument, Theme, QueryDefinition, and WorkspaceDocument
  builders, and recovered the demo catalog after a page reload. It recorded zero console errors or
  uncaught exceptions.
- DOM measurements report no horizontal viewport overflow at 1600x1000, 1366x768, the T14
  1920x1080 viewport, or 800x700. The intentionally untracked captures and their bounded DOM/log
  evidence are in `target/control-cockpit-final/`: `cockpit-1600x1000.png`,
  `cockpit-1366x768.png`, `cockpit-t14-1920x1080.png`, `cockpit-800x700.png`, and
  `normal-demo-t14-1920x1080.png`.

## Merge closure

- The final field ledger has no in-scope modeled-field gaps. PR #2's exact pushed feature head was
  `bba5f89d3d8fcb1d7585a83be8e30a0aea80d4a4`.
- The exact feature head passed `scripts/check.sh` from a cold target in 99.00 seconds and
  `scripts/verify.sh` in 528.44 seconds, including all native, XALEN, WASM, IndexedDB/Worker,
  nested-builder, workspace, history/deletion, macro, conflict, and artifact journeys.
- The normal non-automation Worker/XALEN session and all four responsive viewport inspections
  passed with zero console errors and no horizontal overflow.
- PR #2 was marked ready and squash-merged without branch deletion or administrative bypass on
  2026-08-29. Its verified merge commit is `8e7b610fb6afd8d72510cb1a0431d946a8c912d8`;
  the remote feature branch remains preserved at `bba5f89d3d8fcb1d7585a83be8e30a0aea80d4a4`.
- The merge became the exact verified base of the separately scoped professional-wheel branch.
