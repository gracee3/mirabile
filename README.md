# Astra

Astra is an architecture-first, local-first astrology application written in Rust and Leptos. The first milestone proves that portable chart inputs, deterministic calculation, configurable analysis, composable layout, browser persistence, and reactive editing can remain separate layers.

The project is intentionally not an astronomical authority yet. Its current calculator is deterministic test/demo infrastructure behind an ephemeris provider boundary; calculated positions are cacheable derived data, never the canonical chart record.

## Workspace

- `crates/astra-core`: portable domain resources, workspace/query/view models, commands, and editor state
- `crates/astra-engine`: calculation keys, deterministic ephemeris, aspect analysis, layout, and scene primitives
- `crates/astra-store`: validated repository contract, versioned tombstones, portable JSON, and IndexedDB adapter
- `crates/astra-app`: frozen application boundary plus real hydration, persistence, draft, and derived-pipeline orchestration
- `apps/web`: Leptos 0.8 CSR presentation adapter; normal WASM uses `RealApplication`, while native frontend tests use the mock

Architecture decisions and state semantics live in [`docs/architecture.md`](docs/architecture.md), [`docs/domain-model.md`](docs/domain-model.md), and [`docs/state-and-persistence.md`](docs/state-and-persistence.md).

## Development

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p astra-web --target wasm32-unknown-unknown
cd apps/web && trunk build
./scripts/test-browser.sh
```

These checks are intentionally local; this repository does not enable hosted CI. The browser
script builds a feature-gated IndexedDB contract app, serves it from `target/`, and requires a
passing DOM marker from an isolated headless Chromium profile.

The browser build requires the `wasm32-unknown-unknown` Rust target. Run the app with:

```bash
cd apps/web
trunk serve
```

## Status

This repository has a persisted real-application foundation. The wheel still uses a deterministic
fake ephemeris and in-thread pending execution; it is not suitable for real astrological
calculation.

## License

MIT. See [`LICENSE`](LICENSE).
