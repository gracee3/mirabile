# Mirabile Workbench, Local Authoring, Automation, and Agentic Control

## Goal state

- Base: `892bbcb8a44118de21b1715348cc2905e3716dbb` (`origin/main` verified 2026-08-24)
- Branch: `goal/mirabile-workbench-authoring`
- Worktree: `/home/emmy/worktrees/mirabile-workbench-authoring`
- Current phase: 5 - provider-neutral authoring capabilities
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
| 5. Authoring capabilities | In progress | Provider-neutral supported/default options | Pending | Pending |
| 6. New chart authoring | Pending | Typed editor, preview/cancel/atomic create | Pending | Pending |
| 7. Saved chart editing | Pending | Atomic CAS batch, conflict projection, shared-record guard | Pending | Pending |
| 8. Workspace management | Pending | Library/new/open/rename/save/discard/switch and explicit demos | Pending | Pending |
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

## Blockers

- None.
