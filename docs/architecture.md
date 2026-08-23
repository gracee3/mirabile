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
ChartRecord + ChartDefinition + provider identity
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

- `CalcKey` hashes canonical calculation inputs, referenced record revisions, engine identity, ephemeris identity, and timezone-data identity.
- `AnalysisKey` hashes one or more calculation keys plus analytical inputs.
- `LayoutKey` hashes the analysis key and view/wheel inputs.
- `RenderKey` hashes the layout key, theme, and renderer identity.

Keys name derived computations; they are not canonical resources. A changed dependency asks for a new key instead of toggling global dirty flags.

## Commands and reactive UI

Mutation intent is represented by commands in `astra-core`. The milestone UI keeps separate signals for canonical resources, editor state, source inputs, and presentation theme. Memoized values resolve effective analysis inputs and run downstream pure functions. Components do not calculate positions or mutate browser storage directly.

A production worker is intentionally deferred. The calculation request/result types and `CalculationEngine` boundary are native and serializable so the same calculation can move behind a Web Worker without changing domain or view models.

## Intentional skeletons

Query expressions, derivation recipes, view documents, wheel templates, sync operations, and worker messages establish extension points but only the parts needed by the vertical slice execute today. A skeleton is not claimed as a complete engine.
