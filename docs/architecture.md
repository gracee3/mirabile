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

`astra-core` is portable and has no browser, Leptos, storage, or calculation-provider dependency.
`astra-engine` owns provider-neutral execution contracts, orchestration, content keys, analysis, and
layout; it has no DOM or persistence dependency. `astra-store` owns persistence adapters.
`astra-app` owns `Application`, `RealApplication`, hydration, calculation runtime orchestration,
canonical-to-read-model projection, workspace commands, and drafts. `astra-web` is the CSR
presentation adapter and also packages the calculation-worker binary; normal UI modules still see
only `astra-app`.

## Calculation ownership and execution boundary

`astra_core::CalculationSpec` remains canonical orchestration configuration. It records the user's
zodiac, houses, coordinates, node, Black Moon, fortune-formula, and correction choices. It is not a
backend request and contains no XALEN or Swiss types.

`CalculationEngine::prepare` resolves a radix `ChartRecord`, effective `CalculationSpec`, and the
union of concrete displayed/aspected points into `ResolvedCalculationRequest` from
`astra-engine/src/contract.rs`. Point categories must be expanded before this boundary. The
resolved request contains only calculation facts:

```text
ResolvedCalculationRequest
├── CalculationContext { ResolvedTime, NumericLocation }
├── ZodiacCalculationRequest
├── CelestialPositionsRequest
│   ├── explicitly requested PointIds
│   ├── CoordinateSystem
│   ├── CorrectionSpec
│   └── node and Black Moon model choices
├── Option<HouseCalculationRequest>
│   ├── HouseSystem
│   └── zodiac/ayanamsa semantics
└── DerivedPointsRequest
    └── explicitly requested astrology-specific formulas
```

`NoHouses` produces no house request. Part of Fortune, when explicitly requested, becomes a
derived formula request and adds its Sun/Moon celestial prerequisites; the deterministic backend
then truthfully reports that derived formulas are unsupported. No new astrology mathematics is
implemented. Node and Black Moon choices remain with the celestial/model capability in this slice
because that is where a future initial adapter is expected to calculate them; the contract does
not claim a permanent philosophical classification.

Results preserve the same responsibility split as `CelestialPositionsResult`, optional
`HouseCalculationResult { cusps, angles }`, and `DerivedPointsResult`. `CalculationEngine::complete`
validates result point sets, component presence, implementation identity, model identity, and
configuration provenance before creating `CalculationValue`. The value keeps celestial and
derived positions separate while exposing an internal combined lookup to existing analysis and
layout code.

No `ResourceEnvelope`, `ChartRecord`, `ChartDefinition`, `ResourceRevisionRef`, or `Workspace`
crosses the backend/worker boundary. Current resource revisions and display labels remain in
`SnapshotContext`, which is reconstructed application-side.

## Backend contract, capability, and identity

`astra-engine/src/backend.rs` defines one provider-neutral interface:

```rust
pub trait CalculationBackend {
    fn descriptor(&self) -> BackendDescriptor;
    fn calculate(
        &self,
        request: &ResolvedCalculationRequest,
    ) -> Result<CalculationBackendResult, CalculationBackendError>;
}
```

A single backend may implement any combination of celestial positions, houses/angles, and derived
formulas. `BackendCapabilities` advertises those independently, including supported celestial
points and house systems. Unsupported requested work returns a typed unsupported-capability error;
it is never silently omitted or substituted.

`BackendDescriptor` separates pre-execution selection from result provenance. Its deterministic
`BackendFingerprint` contains:

- the overall `ImplementationIdentity { id, version, revision }`;
- celestial implementation identity plus optional neutral `EphemerisModelIdentity`;
- optional house implementation identity;
- optional derived implementation identity.

`EphemerisModelIdentity` can describe analytic models, numerical ephemerides, tables, or another
model with optional version, revision, and data fingerprint. It does not assume JPL data. A
validation reference is not reported as the underlying model.

`CalculationProvenance` records the Astra calculation-engine identity and timezone-data version,
the selected backend actually used, any material time-scale conversion and model identities,
celestial implementation/model/coordinates/corrections/zodiac, lunar-node and Black Moon model
choices, house implementation/system/zodiac when requested, and derived implementation/formula
identities when requested. Tropical versus sidereal is typed.
Sidereal provenance uses Astra-owned
`AyanamsaConfiguration { id, parameters }`, resolved from the canonical identifier; no provider
enum leaks into Astra.

`DeterministicBackend` satisfies both celestial and house capabilities in one implementation. It
is test/demo-only and not astronomical authority. Its honest capability surface is tropical,
geocentric celestial output with no corrections, its checked-in point catalog, and Equal houses.
It rejects sidereal, topocentric, heliocentric, enabled-correction, Placidus, Whole Sign, derived,
and unknown-point requests rather than echoing unimplemented semantics into provenance. The
bootstrap chart definitions explicitly select Equal houses for this fixture; Astra's canonical
`CalculationSpec` default remains unchanged.

## Content-addressed invalidation

`CalcKey` hashes exactly the resolved calculation request, Astra calculation-engine identity, and
the complete selected backend fingerprint. The request already contains resolved time including
timezone-data identity, numeric location, requested points, coordinate/correction configuration,
zodiac/ayanamsa semantics, optional house request/system, and derived requests/formulas. The
fingerprint adds backend/component implementation versions and revisions plus underlying
model/data identity.

Consequently, materially different backend revisions, models/data, house implementations,
coordinates, corrections, requested point sets, tropical/sidereal modes, or ayanamsas cannot share
a key. Titles, subject display details, notes, source wording, life events, atlas display labels,
resource revisions, and other non-calculation metadata remain absent.

`AnalysisKey`, `LayoutKey`, and `RenderKey` retain their prior consumed-material rules.
`CalculationValue`, not canonical context, is cached by `CalcKey`. Current resources always rebuild
`SnapshotContext`, so cache reuse cannot return stale canonical revisions.

## Runtime and worker protocol

`astra-app/src/runtime.rs` defines the application-facing `CalculationRuntime` with
`backend_descriptor`, `submit`, and asynchronous `receive`. `InlineCalculationRuntime<B>` executes
the same serialized request/result contract for native code and tests. The normal WASM constructor
uses `WorkerCalculationRuntime` from `astra-app/src/web_worker_runtime.rs`.

Trunk builds `apps/web/src/bin/calculation-worker.rs` as the distinct
`calculation-worker.js`/`calculation-worker_bg.wasm` Web Worker. It owns the linked
`XalenBackend` selected by normal browser construction; calculation therefore runs outside the
Leptos/UI execution context. The Worker retains deterministic dispatch for controlled tests. The
frontend continues to construct and consume only `Application`.

The versioned protocol in `astra-engine/src/worker.rs` is:

```text
CalculationWorkerRequest
├── WorkerProtocolVersion
├── CalculationRequestId
├── CalcKey
├── BackendFingerprint
└── ResolvedCalculationRequest

CalculationWorkerResult
├── WorkerProtocolVersion
├── CalculationRequestId
├── CalcKey
└── CalculationOutcome
    ├── Success(CalculationBackendResult)
    └── Failure(CalculationWorkerFailure)
```

Failure categories distinguish invalid input, unsupported capability, backend failure, protocol
mismatch, and internal execution failure. Unsupported protocol requests are rejected explicitly.
Backend-native errors never cross the protocol.

The semantic contract is ordinary serializable Astra data. Although the immediate Worker links
the XALEN and deterministic backends, neither `CalculationBackend` request/result types nor the
worker protocol require in-process or WASM linkage. A separately distributed executable or local service
can implement the same contract.

## Latest-wins application semantics

Every submitted calculation receives a typed, monotonically allocated `CalculationRequestId`.
Each `ViewRuntime` stores its own expected `(request_id, calc_key)` and the application retains the
matching analysis/layout plan by request ID. A result is authoritative only when:

1. its request ID maps to submitted work for that view;
2. that ID equals the view's currently expected ID;
3. its result `CalcKey` equals both the expected and prepared key; and
4. its protocol version is current.

An authoritative success is validated, cached under its own key, analyzed, laid out, and published.
An authoritative typed failure keeps the last good Scene and changes that view to `Failed`. A
CalcKey mismatch is an authoritative runtime-integrity failure and is never attached as a result.

Stale successes and stale failures are discarded before cache insertion or UI mutation. They do
not replace Scenes, notices, current calculation state, or advance `ProjectionVersion`. This slice
chooses not to cache stale successes, keeping the proof simple. Cancellation is optional and is not
required for correctness.

Calculation-changing intents, including refresh, may supersede a running request. The state model
remains:

```text
no prior result + current request running  → None + Loading
prior result + current request running     → old Scene + Refreshing
latest request succeeds                    → new Scene + Fresh
latest request fails                       → old Scene + Failed
```

## Future provider preparation

XALEN is the planned initial/default real implementation, but it is not integrated and is not a
dependency in this slice. Its future adapter maps Astra requests to a pinned XALEN API and maps
results/provenance back; XALEN public types do not define canonical resources, `CalculationValue`,
the worker protocol, read models, or snapshots. No known contract change is required beyond small
provider-neutral capability additions if the pinned API exposes a presently unknown semantic.

Swiss Ephemeris is a first-class optional professional/reference backend, but no Swiss code, data,
dependency, flags, or native constants are present. Its AGPL/professional dual licensing requires
distribution isolation; moving it into another Rust crate is not assumed sufficient. The
serializable contract permits a separately distributed adapter, executable, or local service.

## Intentional exclusions

Real planetary accuracy, XALEN and Swiss adapters, new house/ayanamsa/derived-point mathematics,
derived charts, transits, relationships, temporal/query execution, OPFS, sync, encryption, and new
frontend UX remain out of scope. The implemented deterministic runtime proves the provider and
execution boundary without claiming those capabilities.
