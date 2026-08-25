# Mirabile Workbench, Local Authoring, Automation, and Agentic Control

## Goal state

- Base: `892bbcb8a44118de21b1715348cc2905e3716dbb` (`origin/main` verified 2026-08-24)
- Branch: `goal/mirabile-workbench-authoring`
- Worktree: `/home/emmy/worktrees/mirabile-workbench-authoring`
- Current phase: 9 - full Aspect Set authoring
- Delivery: unmerged goal branch; push and remote-SHA verification required at handoff
- Baseline: `./scripts/check.sh` passes 118 tests, strict Clippy, formatting, and diff checks

## Frozen boundaries

- Preserve the consolidated domain/application architecture and the single authoritative `AppReadModel`.
- Extend existing application, dispatcher, repository, browser, and standard-library CDP boundaries; add no framework, hosted CI, remote-control server, or astrology breadth.
- Keep `ChartRecord` factual and `ChartDefinition` computational. Preserve Follow/Pinned/Inline bindings and complete revision history.
- Expose only provider-neutral capability truth. Browser presentation never chooses XALEN-specific correction flags.
- Automation is compile-time non-default, query-enabled, local Chromium CDP only, semantically whitelisted, and isolated from the normal `mirabile` database.
- Macros contain typed intents and symbolic bindings, never selectors, coordinates, raw JavaScript, or invalid local buffers.
- Machine snapshots exclude full `Scene` contents.

## Resumption matrix

| Phase | State | Durable result | Validation | Commit |
| --- | --- | --- | --- | --- |
| 1. Isolation and settlement | Complete | Dedicated worktree; living goal; authoritative activity projection; observable chart/workspace/resource writes | `./scripts/check.sh` passes 119 tests, strict Clippy, formatting, and diff checks | Phase 1 commit |
| 2. Semantic observation | Complete | Validated ControlId/ControlAddress, manifest/snapshot DTOs, bounded trace, calculation diagnostics, FIFO coordinator | `./scripts/check.sh` passes 125 tests, strict Clippy, formatting, and diff checks | Phase 2 commit |
| 3. Workbench controls | Complete | Native panel/disclosure/field/select/toggle/action/status palette; buffered text/number/date/time semantics; semantic DOM instrumentation | Native and WASM checks; `./scripts/check.sh` passes 127 tests, strict Clippy, formatting, and diff checks | Phase 3 commit |
| 4. Automation tooling | Complete | Non-default query-gated bridge; isolated IndexedDB override; loopback-only standard-library CDP client/CLI; scenario and failure artifacts | Native/WASM feature checks; Python contract tests; live semantic smoke and expected-failure artifact smoke | Phase 4 commit |
| 5. Authoring capabilities | Complete | Provider-neutral zodiac/coordinate/point/house support and default correction metadata; contextual app options with disabled reasons | `./scripts/check.sh` passes 129 Rust tests plus 4 Python tests; native/WASM feature checks | Phase 5 commit |
| 6. New chart authoring | Complete | Typed application-owned editor/defaults; partial-location validation; last-valid preview; native controls; cancel/no-write; atomic create and slot promotion | `./scripts/check.sh` passes 133 Rust tests plus 5 Python tests; native/WASM automation checks; live semantic and actual-control XALEN journeys | Phase 6 commit |
| 7. Saved chart editing | Complete | Atomic compare-only/write CAS batch; separate Record/Definition bases; saved preview/cancel/save; structured two-component conflicts; shared-record protection | `./scripts/check.sh` passes 139 Rust tests plus 5 Python tests; native/WASM checks; IndexedDB rollback contract; live visible-control saved edit/cancel/save journey | Phase 7 commit |
| 8. Workspace management | Complete | Workspace summaries/title metadata; typed new/open/rename/save/discard; loss-reasoned Save/Discard/Stay switching; explicit idempotent atomic demo loading | `./scripts/check.sh` passes 145 Rust tests plus 5 Python tests; native/WASM checks; live IndexedDB/Worker/XALEN visible-control lifecycle | Phase 8 commit |
| 9. Aspect Set authoring | Pending | Full-row lifecycle editor | Pending | Pending |
| 10. Session and slots | Pending | Inspectable temporary/durable display and slot state | Pending | Pending |
| 11. Diagnostics UI | Pending | Dense diagnostics, trace, deliberate JSON export | Pending | Pending |
| 12. Macros | Pending | Record/import/export/replay with bindings and failures | Pending | Pending |
| 13. Level A scenarios | Pending | Shared Mock/Real semantic conformance | Pending | Pending |
| 14. Level B browser semantics | Pending | Real IndexedDB/Worker/XALEN/peer workflows | Pending | Pending |
| 15. Level C control E2E | Pending | Native-control golden journeys | Pending | Pending |
| 16. Cockpit composition | Pending | Single dense accessible workbench | Pending | Pending |
| 17. Handoff | Pending | Docs, audit, verification, push, clean remote equality | Pending | Pending |

## Required acceptance evidence

- Fast checks remain `./scripts/check.sh`; full handoff remains `./scripts/verify.sh`.
- `scripts/test-browser.sh` retains its low-level purpose and covers atomic `save_batch`.
- `scripts/test-workbench-e2e.sh` covers semantic and native-control levels with unique profile/database identities.
- Every UI failure preserves the required screenshot, DOM, controls, application, trace, log, scenario, and summary artifacts.
- Final audit checks authority bypasses, selector/pixel automation, invalid-buffer leakage, partial saves, silent workspace loss, draft leakage, sleeps, accessibility, dependencies, astrology scope, and notices/licenses.

## Implemented decisions

- `AppReadModel::is_settled()` is the only public settlement predicate. Its cohesive activity projection identifies initialization, current view computation, chart creation, chart saving, workspace saving, resource saving, and demo loading.
- Current authoritative view expectations determine projected calculation work. Superseded runtime requests can remain internally drainable without keeping the projection unsettled.
- ChartDraft atomic creation and WorkspaceDocument persistence are accepted as pending work and complete through `wait_for_update()`, matching the existing Aspect Set save lifecycle.
- Repository success, conflict, and failure all advance `ProjectionVersion`, publish a settled recoverable projection, and keep `ApplicationStatus::Ready`. Presentation settlement loops no longer reverse-engineer pending state from component fields.
- `ControlId` uses validated lowercase dotted segments; `ControlAddress` adds sorted semantic qualifiers and rejects duplicates or invalid JSON input. Manifest descriptors carry authoritative value, optional local buffer, interaction state, availability/options, and entity identity.
- `AutomationSnapshotV1` selects application/workspace/chart/view/calculation state, controls, coordinator state, and trace without serializing `Scene`. Provider-neutral diagnostics retain backend/engine identity, Worker protocol, request and calculation/analysis keys, computation state, and last-good Scene presence.
- `WorkbenchCoordinator` serializes initialization and all semantic actions FIFO, publishes only newer projections, settles through the application predicate, tracks action source/origin/highlight, and records a bounded 256-entry execution trace with accepted/settled projections and pending transitions.
- The shallow native control palette covers panels, disclosure sections, field rows, buffered text/number/date/time fields, enum selects, pickers, toggles, actions, and status. Buffered fields keep invalid strings local, commit only parsed values on Apply/Enter, restore authoritative values on Escape/Cancel, and restore keyboard focus.
- Existing workbench actions now expose stable `data-mirabile-control`, canonical address, and semantic qualifier attributes. Human actions carry their originating `ControlAddress` into the coordinator trace. Aspect orb entry now commits only a valid semantic value, and both visible and keyboard save paths reject an invalid local buffer.
- `workbench-automation` is a non-default web feature and still requires `mirabileAutomation=1` plus a validated `mirabile-workbench-e2e-*` or `mirabile-workbench-dev-*` IndexedDB name. Normal builds continue to open `mirabile`.
- The versioned browser bridge exposes only snapshot, manifest, typed execution, settlement, trace, one-shot action source, and the reserved macro replay entrypoint. The CLI exposes stable JSON commands and native semantic-address interaction without an arbitrary-evaluate command.
- The reusable standard-library CDP client rejects non-loopback endpoints. Live Chromium smoke covers IndexedDB initialization, Worker/XALEN settlement, semantic dispatch, snapshot, controls, and trace; the expected-failure smoke preserves all eight required artifacts.
- Backend descriptors now carry provider-neutral authoring support for zodiac modes and coordinate systems plus the provider's default correction profile; existing point and house capability metadata remains authoritative. The application projects every finite zodiac, coordinate, house, and timezone choice with contextual enabled state and a reason for every disabled choice.
- XALEN truth is Tropical, Geocentric, Sun through Jupiter, NoHouses everywhere, and Equal/Placidus only with complete location. Sidereal, Whole Sign, Topocentric, Heliocentric, named zones, Local Mean Time, and Local Apparent Time remain visibly deferred or unsupported; its all-enabled apparent-place correction profile is application data rather than a browser choice.
- Machine snapshots include the cohesive authoring capability projection. The inaccurate deterministic-provider browser label was replaced with a provider-neutral local-worker description.
- New chart authoring begins from `Untitled Chart`, Birth, the current UTC civil instant, no subject/location, Tropical, Geocentric, NoHouses, and the backend descriptor's correction profile. Every edit is a typed `ChartMutation`; browser buffers parse dates, times, offsets, coordinates, and enums into core values before dispatch.
- The private authoring draft can retain an enabled but incomplete manual location. Structured field validation is projected while the last valid aggregate and Scene remain authoritative; house selection is location-gated and removing a required location cannot leak a runtime-invalid chart into save.
- Beginning a chart creates only session state, assigns its stable instance to the active required slot as a preview overlay, and writes nothing. Cancel removes it without writes. Save creates Record and Definition through one atomic batch, promotes the same slot assignment, updates library/workspace projections, and leaves workspace membership explicitly dirty.
- The feature-gated bridge whitelists the same typed chart actions. Live semantic and native-control journeys exercise XALEN defaults, incomplete application validation, an invalid local numeric buffer, Enter commits, capability-gated Equal houses, settlement, trace origins, atomic save, and a final screenshot using only semantic control addresses.
- `AtomicSaveBatch` carries unique revision expectations and changed canonical resources. Expectations may be compare-only; Memory preflights the complete batch before mutation, and IndexedDB performs expectation reads, current-head writes, and historical insertions in one two-store transaction. Conflicts identify every stale resource, and a forced second history-key failure proves neither current head advances.
- Saved chart editors retain distinct Record and Definition envelopes and revisions plus the complete original factual record as a template. Definition-only edits compare the Record head without creating a meaningless Record revision, while record provenance, calendar/disambiguation, notes, life events, and unchanged atlas metadata remain intact.
- Saved previews are application-owned overlays: cancel drops the overlay and recalculates from the canonical catalog without writes. Save is observable `ChartSave` work, checks both component bases, publishes only changed components, and advances every success, conflict, or failure into a settled recoverable projection.
- Batch conflicts refresh discoverable component heads into the catalog while retaining local fields and projecting Record-versus-Definition conflict details. A ChartRecord referenced by multiple definitions disables factual mutations with an explicit copy/detach explanation while leaving title and calculation-definition edits available.
- The workbench exposes a qualified `chart.edit-saved[instance=...]` native action and the local CLI can resolve an unqualified control ID only when the manifest contains exactly one matching dynamic address. The live Chromium journey creates a chart, cancels a saved edit, reopens it, publishes a definition-only revision, and verifies trace and final snapshot through actual controls.
- Workspace library summaries expose stable ID, envelope title, and revision. `WorkspaceSession::working_title` owns the editable title while canonical metadata remains on the `ResourceEnvelope`; title-only saves therefore create truthful workspace revisions without moving title into `WorkspaceDocument`.
- New workspaces use `Untitled Workspace` and the existing one-primary-wheel Current Transits session shape. Open/New requests switch immediately only when safe; otherwise the authoritative read model projects loss reasons and Save-and-switch, Discard-and-switch, and Stay actions.
- Save-and-switch is enabled only when a workspace save can preserve all outstanding work. Draft charts, dirty chart/resource editors, and temporary display overrides block it, and the application recomputes those blockers when the action executes so a stale prompt cannot silently lose newly created local work.
- Explicit workspace discard restores the current saved envelope or a fresh unsaved session. Successful Save-and-switch first publishes the source workspace revision and only then activates the target; repository failure clears the deferred target and leaves the current application usable.
- Demo loading is never initialization behavior. `LoadDemoBundle` is observable pending work that inspects every stable identity, atomically creates only missing resources, leaves compatible existing heads and histories untouched, is idempotent, and rejects deleted or wrong-kind collisions without partial creation.
- Native workspace controls expose buffered title editing, new/save/discard, stable qualified Open actions, explicit demo loading, and the three switch resolutions. The live control journey proves demo load, Stay, Discard-and-switch, title revision 1 to 2, Save-and-switch, new-workspace defaults, and discard restoration against isolated IndexedDB, the Worker, and XALEN.

## Blockers

- None.
