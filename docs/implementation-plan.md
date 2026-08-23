# Architecture foundation implementation plan

This plan was written after inspecting the clean README-only repository on 2026-08-22. The available toolchain is Rust/Cargo 1.97.1 and Trunk 0.21.14; the browser WASM target is not installed on the host.

1. Bootstrap a four-member Rust workspace and a minimal Leptos 0.8 CSR app using Trunk. Keep Leptos and DOM dependencies in `astra-web` only.
2. Build portable, serializable canonical types in `astra-core`: typed identifiers/units, chart assertions and definitions, typed resource envelopes, bindings, workspaces, views, queries, commands, and draft editor transitions.
3. Build the derived pipeline in `astra-engine`: provider-neutral calculation, deterministic fake ephemeris, content keys, aspect analysis, layout, and astrology-free scene output.
4. Put persistence behind an async repository contract in `astra-store`. Preserve revision history in the deterministic memory adapter and provide a thin IndexedDB adapter using the same portable JSON representation without leaking IndexedDB fields into domain resources.
5. Prove the architecture in `astra-web`: a source chart flows through calculation, analysis, layout, and SVG; an AspectSet draft previews immediately and saves or cancels without mutating canonical state prematurely.
6. Encode the handoff's acceptance behaviors as native Rust tests, then run format, unit tests, clippy, native workspace checks, and the Trunk/WASM build where the installed target permits.

Out of scope for this milestone: production astronomy, historical timezone resolution, a real calculation worker, temporal-query execution, sync transport, encryption, OPFS assets, import bundles, multi-tab leadership, and professional visual polish. Their boundaries are documented now and must remain additive.

## Outcome

The planned vertical slice is implemented. Native tests exercise domain invariants, resource binding/precedence, draft isolation, revision conflicts/history, portable JSON, calculation and analysis invalidation, presentation-only invalidation, workspace/library separation, and cache reconstruction. The web app persists the demonstration AspectSet and its revisions in IndexedDB; other canonical resource types use the same repository contract but do not yet have UI workflows.

`CalcRequest`/`CalcResult` establish the worker protocol, but calculation still runs synchronously in the milestone UI. Query expressions, derivation recipes, composite views, sync transport, vaults, OPFS, and temporal execution remain honest skeletons or documented boundaries rather than implemented features.

## P0 architecture hardening

The focused P0 pass keeps the same four crates, deterministic provider, schema v1, and UI scope.
It adds semantic projections for calculation/analysis/layout keys; splits cached
`CalculationValue` from current `SnapshotContext`; validates calendars and all canonical payloads;
probes schema versions before v1 import; records deletion as a permanent versioned tombstone; and
makes browser startup fail closed while retaining one IndexedDB handle.

Native tests cover semantic invalidation boundaries, canonical validation, Gregorian/Julian and
Julian Day fixtures, cache/context reuse, and memory-repository tombstones. The feature-gated local
browser contract covers the matching IndexedDB behavior and transaction rollback. Persisted chart
lifecycle resources, real astronomy, a calculation Web Worker, `astra-app`, migrations, and hosted
CI remain out of scope.
