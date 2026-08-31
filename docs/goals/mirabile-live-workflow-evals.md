# Mirabile live-workflow alignment

## Goal state

- Base: `a407a828cfbe2724db8643d8081eb13381946361` on `main`.
- Branch: `goal/mirabile-live-workflow-evals` in its dedicated worktree.
- Scope: create, edit/recalculate, biwheel, per-view display, and save/reopen workflows through
  native controls and the local semantic workflow interface.
- Promotion: pull request only; merging remains separately authorized.

## Delivered contracts

- `WorkflowDocumentV1` is a closed, serializable, validated sequence of named actions. References
  use stable IDs or earlier typed bindings, dirty workspace opening is explicit, unsupported
  provider choices retain the application's human-facing capability reason, and execution uses the
  existing coordinator and settlement path.
- View documents add serde-defaulted titles, per-view profile bindings, per-slot hidden points and
  rings, and aspect-layer visibility. Existing workspace bindings and global hidden points remain
  compatibility fallbacks.
- The active-view runtime independently prepares and caches radix/comparison calculations under one
  generation. It publishes only a complete two-slot scene, rejects stale slot results, and retains
  the last complete scene during refresh and failure.
- Relationship aspects own both endpoint slot and point identity. Radix supplies orientation,
  houses, and angles; comparison is the external ring; cross aspects are the default visible layer.
- Scene metadata and accessible SVG identify rings, point ownership, endpoint ownership, and aspect
  layers. The resolved palette is carried into SVG CSS variables, so theme changes affect rendered
  colors.
- The desktop product surface is a workspace/chart/view rail, wheel-first center, and Chart/Display
  inspector. Narrow layouts retain every control in wheel-first drawers; cockpit diagnostics remain
  in closed Developer Tools disclosures.

## Evidence and boundaries

- Shared fixture: `scripts/workflow-fixtures/live-workflow-v1.json`.
- Semantic browser journey: `scripts/workbench-scenarios/live-workflow-agent.json`, using an isolated
  IndexedDB database, the WASM `RealApplication`, calculation Worker, and pinned XALEN backend.
- The journey creates both oracle charts, edits Placidus to Equal, creates/configures the biwheel,
  saves, reloads, reopens by resource ID, explicitly reactivates the biwheel, waits for both slots,
  and asserts owned rings plus zero browser errors.
- Named-zone inference, atlas lookup, provider expansion, derived charts, interpretation, query
  execution, and non-wheel ViewObject rendering remain out of scope.
- A successful browser workflow proves local product behavior. It does not claim a new production
  service, provider capability, or benchmark certification.

## Verification record

- `scripts/check.sh`: passed with 100 application, 19 engine architecture/layout, 14 runtime
  contract, 8 pinned-XALEN, store, web, Python bridge, workflow assertion, strict Clippy, formatting,
  and diff checks.
- `live-workflow-agent` at 1366x768: passed create/configure/save/reload/reopen with two owned rings,
  radix houses/angles, comparison retrograde Jupiter, per-slot point visibility, layer ownership,
  and zero browser errors.
- Exact-head disk-gated `scripts/verify.sh`, final commit identity, remote branch identity, and PR head
  are recorded at handoff after those gates complete.
