# Mirabile Workbench, Local Authoring, Automation, and Agentic Control

## Goal state

- Base: `892bbcb8a44118de21b1715348cc2905e3716dbb` (`origin/main` verified 2026-08-24)
- Branch: `goal/mirabile-workbench-authoring`
- Worktree: `/home/emmy/worktrees/mirabile-workbench-authoring`
- Current phase: 2 - semantic controls, observation, and coordinator metadata
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
| 2. Semantic observation | In progress | Control IDs/addresses, manifest DTOs, snapshot, trace, coordinator metadata | Pending | Pending |
| 3. Workbench controls | Pending | Native shallow components and buffered edit semantics | Pending | Pending |
| 4. Automation tooling | Pending | Feature-gated bridge, isolated database, CDP CLI, artifacts | Pending | Pending |
| 5. Authoring capabilities | Pending | Provider-neutral supported/default options | Pending | Pending |
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

## Blockers

- None.
