# Architecture

## Dependency direction

```text
astra-core
   ↑       ↑
engine   store
   ↑       ↑
   └── web ┘
```

`astra-core` is portable and has no browser, Leptos, storage, or astronomy-provider dependency. `astra-engine` has no DOM or persistence dependency. `astra-store` owns persistence adapters. `astra-web` is a CSR adapter that dispatches intent and renders derived state.

The four packages are intentionally broader than the eventual conceptual layers. Strong module boundaries are enough for the first milestone and avoid an early proliferation of tiny crates.

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

Mutation intent is represented by commands in `astra-core`. The milestone UI keeps separate signals for canonical resources, editor state, source inputs, and presentation theme. Memoized values resolve effective analysis inputs and run downstream pure functions. Components do not calculate positions or mutate browser storage directly.

The AspectSet editor dispatches `SaveResourceDraft` through the resource command handler. The handler applies repository revision rules in one place; the component replaces its canonical signal only after IndexedDB confirms the local transaction. Draft edits and Cancel are ephemeral transitions and therefore never touch the repository. Browser startup is a private `Loading`/`Ready`/`Error` state machine: only `Ready` owns an editor and retained repository handle, while `Error` fails closed and offers Retry.

A production worker is intentionally deferred. The calculation request/result types and `CalculationEngine` boundary are native and serializable so the same calculation can move behind a Web Worker without changing domain or view models.

## Intentional skeletons

Query expressions, derivation recipes, view documents, wheel templates, sync operations, and worker messages establish extension points but only the parts needed by the vertical slice execute today. A skeleton is not claimed as a complete engine.
