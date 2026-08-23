# Astra

Astra is an architecture-first, local-first astrology application written in Rust and Leptos. The first milestone proves that portable chart inputs, deterministic calculation, configurable analysis, composable layout, browser persistence, and reactive editing can remain separate layers.

The project is intentionally not an astronomical authority yet. Its current calculator is deterministic test/demo infrastructure behind an ephemeris provider boundary; calculated positions are cacheable derived data, never the canonical chart record.

## Workspace

- `crates/astra-core`: portable domain resources, workspace/query/view models, commands, and editor state
- `crates/astra-engine`: calculation keys, deterministic ephemeris, aspect analysis, layout, and scene primitives
- `crates/astra-store`: repository contract, revisioned memory repository, JSON format, and IndexedDB adapter
- `apps/web`: Leptos 0.8 CSR presentation adapter and the reactive aspect-draft demo

Architecture decisions and state semantics live in [`docs/architecture.md`](docs/architecture.md), [`docs/domain-model.md`](docs/domain-model.md), and [`docs/state-and-persistence.md`](docs/state-and-persistence.md).

## Development

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
trunk build apps/web/index.html
```

The browser build requires the `wasm32-unknown-unknown` Rust target. Run the app with:

```bash
trunk serve apps/web/index.html
```

## Status

This repository is at the architecture-foundation milestone. The wheel uses a deterministic fake ephemeris and demonstrates dependency-selective recomputation; it is not suitable for real astrological calculation.

## License

MIT. See [`LICENSE`](LICENSE).
