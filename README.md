# Mirabile

Mirabile is a local-first astrology workspace written in Rust and Leptos. Its real `Application`
facade owns authoritative state, locally revisioned resources, IndexedDB persistence, calculation
runtime orchestration, and UI read-model projection. The normal browser build performs calculation
in a Web Worker through a provider-neutral contract.

The current real browser backend is the exactly pinned XALEN adapter. XALEN types remain private to
the engine adapter and are converted to Mirabile-owned request, result, and provenance types. The
adapter deliberately supports only its documented narrow point, coordinate, correction, and house
surface. Unsupported semantics fail explicitly. The deterministic backend remains test/demo
infrastructure; Swiss Ephemeris is not linked or distributed.

Calculated positions, analysis, layout, and scenes are disposable derived state. Canonical
`ChartRecord` source assertions remain separate from `ChartDefinition` calculation semantics.

## Workspace

- `crates/mirabile-core`: portable canonical domain, configuration, view, and workspace types
- `crates/mirabile-engine`: provider-neutral calculation contract, XALEN adapter, keys, analysis,
  layout, and scene primitives
- `crates/mirabile-store`: revisioned repository contract, memory adapter, portable JSON, and
  IndexedDB adapter
- `crates/mirabile-app`: `Application` contract and real hydration, session, draft, calculation,
  persistence, and projection orchestration
- `apps/web`: Leptos 0.8 CSR presentation and the calculation Worker binary

Architecture and current constraints are documented in [architecture](docs/architecture.md),
[domain model](docs/domain-model.md), [state and persistence](docs/state-and-persistence.md), and
the concise [decision records](docs/decisions/README.md). The original foundation plan is retained
as a historical record, not as the current product description.

## Development

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p mirabile-web --target wasm32-unknown-unknown
(
  cd apps/web
  env -u NO_COLOR trunk build
)
./scripts/check-xalen-dependencies.sh
./scripts/test-browser.sh
```

All validation is local. This repository intentionally has no hosted CI. The browser contract
builds both the main app and Worker, validates distributed notice assets, then exercises IndexedDB,
application reload, and Worker calculation in an isolated headless Chromium profile.

The WASM build requires the `wasm32-unknown-unknown` Rust target. Run the app with:

```bash
cd apps/web
trunk serve
```

## Current limitations

Mirabile is pre-MVP. Its persisted schemas are development formats and may change incompatibly
until an explicit MVP schema freeze. Historical timezone resolution, location discovery, session
recovery, sync, encryption, import/archive workflows, query/temporal execution, Swiss integration,
and broad astrology features are not implemented. Current chart rendering is an architectural
vertical slice, not a claim of complete professional calculation or wheel-layout coverage.

## License

MIT. See [LICENSE](LICENSE). Third-party distribution terms and required notices are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
