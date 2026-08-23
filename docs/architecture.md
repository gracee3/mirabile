# Architecture

## Dependency direction

```text
astra-core
   ↑       ↑
engine   store
   ↑       ↑
   └─ astra-app
         ↑
        web
```

`astra-core` is portable and has no browser, Leptos, storage, or astronomy-provider dependency.
`astra-engine` has no DOM or persistence dependency. `astra-store` owns persistence adapters.
`astra-app` owns `Application`, `RealApplication`, hydration, canonical-to-read-model projection,
workspace command semantics, draft orchestration, and the real derived pipeline. `astra-web` is a
CSR presentation adapter that imports only `astra-app` in normal source.

The five workspace packages are intentionally broader than the eventual conceptual layers. Strong
module boundaries avoid an early proliferation of tiny crates.

## Pipeline

```text
ChartRecord + ChartDefinition + provider/timezone identities
                         ↓
                  CalculationValue
                         + current SnapshotContext
                         ↓
                   ChartSnapshot
                         ↓
PointSet + AspectSet + AnalysisProfile
                         ↓
                    ChartAnalysis
                         ↓
ViewDocument + WheelTemplate
                         ↓
                    WheelLayout
                         ↓
                       Scene
                         ↓
                   Leptos SVG
```

The fake ephemeris implements the same provider contract intended for a future astronomical provider. No Swiss Ephemeris code, data, identifiers, or schema assumptions are present.

## Content-addressed invalidation

- `CalcKey` hashes only the asserted civil time/calendar/zone, numeric coordinates, concrete calculation specification, engine identity, ephemeris identity, and timezone-data identity.
- `AnalysisKey` hashes sorted calculation keys and resolved point IDs, enabled aspect IDs and numeric rules, the profile fields the analyzer consumes, and the analyzer version.
- `LayoutKey` hashes resolved displayed-point longitudes, displayed aspect endpoint pairs, and the wheel radii actually consumed by layout.
- `RenderKey` hashes the layout key, theme, and renderer identity.

Keys name derived computations; they are not canonical resources. A changed dependency asks for a new key instead of toggling global dirty flags.

Titles, subject details, notes, provenance wording, atlas labels, aspect labels/classification,
resource revisions, and unused profile/view fields are intentionally absent from semantic key
material. Point and aspect material is sorted before hashing, and unresolved point categories fail
at engine boundaries.

`CalculationValue` contains only resolved time, numeric location, positions, houses/angles, and
calculation provenance. The computation cache stores that value by `CalcKey`. `SnapshotContext`
holds the current definition/record revision references and location display label, so a cached
value can be reused without returning stale resource context.

## Commands and reactive UI

User intent crosses the UI boundary as `AppIntent`. `RealApplication` translates canonical
Workspace changes into typed `astra_core::Command` values. Open, close, activation, selection,
active-view, chart-slot, and AspectSet-binding semantics are applied in `astra-app`; repository
revision validation stays in `astra-store`. Components do not calculate positions, own canonical
resources, or mutate browser storage directly.

The AspectSet editor is application-owned. A typed mutation changes only its draft and queues an
analysis/layout refresh. Save publishes `Saving`, then asks the repository to accept the next
revision; success replaces canonical state and conflict retains the draft with both revisions.
Cancel performs no repository write. Browser startup is an application
`Initializing`/`Ready`/`Error` state machine and retains one IndexedDB handle.

`RealApplication` runs the deterministic pipeline as queued pending work: ChartDefinition and
ChartRecord produce `CalcKey`; `ComputationCache` yields or stores `CalculationValue`; current
resource revisions rebuild `SnapshotContext`; analysis consumes effective point/aspect/profile
bindings; layout consumes displayed points and wheel; `Scene::from_wheel` produces the frontend
boundary. A production worker is intentionally deferred. The pending executor can move behind a
Web Worker without changing the application contract.

## Intentional skeletons

Query expressions, derivation recipes, view documents, wheel templates, sync operations, and worker messages establish extension points but only the parts needed by the vertical slice execute today. A skeleton is not claimed as a complete engine.
