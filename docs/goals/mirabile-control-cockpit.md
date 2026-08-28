# Mirabile Comprehensive Control Cockpit

## Goal state

- Base: `b6712bc9ed5913238bc7535b11a1a2e155b54450` (`origin/main`, verified
  2026-08-28 after squash-merging PR #1)
- Branch: `goal/mirabile-control-cockpit`
- Worktree: `/home/emmy/mirabile` (the existing checkout; no additional worktree or Cargo target)
- Current phase: 4 - complete modeled fields and editors
- Delivery: focused commits pushed regularly; open a feature PR but do not merge it
- Fast baseline: `./scripts/check.sh` passes 155 Rust tests and 5 Python tests, strict
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
| 4. Complete editors | In progress | Metadata plus initial native typed payload fields; chart/workspace and typed list-builder completion remains | Native and browser authoring coverage | Pending |
| 5. Bindings and outputs | In progress | Writable Follow/Pinned/Inline bindings, provider-neutral tables, provenance, parameter status, and last-good retention | Focused binding and last-good regressions | Pending |
| 6. Cockpit composition | In progress | Document-height eight-section cockpit, sticky search/fold controls, and semantic addresses | WASM build and responsive captures | Pending |
| 7. History and deletion | In progress | Revision inspection plus reference-aware two-step deletion, tombstones, and stale-delete conflicts; browser reload journey remains | Store/application deletion tests | Pending |
| 8. Handoff | Pending | Viewport captures, docs, full gates, pushed feature PR left open | T14 acceptance matrix and remote verification | Pending |

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

## Blockers

- None.
