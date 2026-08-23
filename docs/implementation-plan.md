# Architecture foundation implementation plan

This plan was written after inspecting the clean README-only repository on 2026-08-22. The
available toolchain is Rust/Cargo 1.97.1 and Trunk 0.21.14. The browser WASM target is now installed
and included in local verification.

1. Bootstrap a four-member Rust workspace and a minimal Leptos 0.8 CSR app using Trunk. Keep Leptos and DOM dependencies in `astra-web` only.
2. Build portable, serializable canonical types in `astra-core`: typed identifiers/units, chart assertions and definitions, typed resource envelopes, bindings, workspaces, views, queries, commands, and draft editor transitions.
3. Build the derived pipeline in `astra-engine`: provider-neutral calculation, deterministic fake ephemeris, content keys, aspect analysis, layout, and astrology-free scene output.
4. Put persistence behind an async repository contract in `astra-store`. Preserve revision history in the deterministic memory adapter and provide a thin IndexedDB adapter using the same portable JSON representation without leaking IndexedDB fields into domain resources.
5. Prove the architecture in `astra-web`: a source chart flows through calculation, analysis, layout, and SVG; an AspectSet draft previews immediately and saves or cancels without mutating canonical state prematurely.
6. Encode the handoff's acceptance behaviors as native Rust tests, then run format, unit tests, clippy, native workspace checks, and the Trunk/WASM build where the installed target permits.

Out of scope for the original foundation milestone: production astronomy, historical timezone
resolution, temporal-query execution, sync transport, encryption, OPFS assets, import bundles,
multi-tab leadership, and professional visual polish. Their boundaries are documented and must
remain additive.

## Outcome

The planned vertical slice is implemented. Native tests exercise domain invariants, resource binding/precedence, draft isolation, revision conflicts/history, portable JSON, calculation and analysis invalidation, presentation-only invalidation, workspace/library separation, and cache reconstruction. The web app persists the demonstration AspectSet and its revisions in IndexedDB; other canonical resource types use the same repository contract but do not yet have UI workflows.

The later calculation-runtime slice replaced `CalcRequest`/`CalcResult` with a versioned typed
protocol carrying only `ResolvedCalculationRequest`. Normal WASM now runs deterministic backend
calculation in a real Web Worker; native tests use the same contract inline. Query expressions,
derivation recipes, composite views, sync transport, vaults, OPFS, and temporal execution remain
honest skeletons or documented boundaries rather than implemented features.

## P0 architecture hardening

The focused P0 pass keeps the same four crates, deterministic provider, schema v1, and UI scope.
It adds semantic projections for calculation/analysis/layout keys; splits cached
`CalculationValue` from current `SnapshotContext`; validates calendars and all canonical payloads;
probes schema versions before v1 import; records deletion as a permanent versioned tombstone; and
makes browser startup fail closed while retaining one IndexedDB handle.

Native tests cover semantic invalidation boundaries, canonical validation, Gregorian/Julian and
Julian Day fixtures, cache/context reuse, and memory-repository tombstones. The feature-gated local
browser contract covers matching IndexedDB behavior and transaction rollback. Migrations, general
atomic multi-resource chart creation, hosted CI, and real astronomy remain out of scope.

## Real application integration

`astra-app` now contains the production facade behind the frozen `Application` contract. It owns
repository acquisition and hydration, deterministic canonical bootstrap, persisted Workspace
commands, binding resolution, calculation/cache/snapshot/analysis/layout orchestration, read-model
projection, capabilities, draft Save/Cancel/conflict, ProjectionVersion, and non-consuming update
notifications. The normal WASM shell constructs this implementation over IndexedDB; native web
tests keep the deterministic mock.

Native tests prove MemoryRepository reload, cache reuse, typed protocol round trips, and per-view
latest-wins behavior under controlled out-of-order completion. The Chromium contract proves both a
real Web Worker calculation and the persisted chart/AspectSet lifecycle across two application
instances in one isolated IndexedDB database. The linked backend remains deterministic/demo-only;
XALEN and Swiss Ephemeris are not integrated.
