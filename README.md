<p align="center">
  <a href="assets/locus-logo.locus">
    <img src="assets/locus-logo.svg" alt="Locus: deterministic geometry diagrams as code" width="420">
  </a>
</p>

# Locus

Locus is an agent-friendly relational language and CLI toolchain for deterministic plane-geometry figures and technical diagrams.

## Agent-friendly by design

The core goal of Locus is to let AI agents create, inspect, verify, and revise precise geometry without relying on visual feedback. Agents declare geometric relationships instead of guessing coordinates, use structured diagnostics and stable JSON output to understand failures, and verify intent through persistent assertions. Deterministic output and diffable `.locus` source files make every change reproducible and auditable.

The Rust workspace implements both the v0.4 (`locus 3`) language and the compatible v0.3 (`locus 2`) pipeline. Format 3 adds parameterless points, shape-owned placement, explicit circle-tangency branches, ellipses, and B-splines. See the [v0.4 specification](documents/specification.md) and [demonstrations](spec/test/README.md); both demonstrations and stress fixtures use format 3.

## Development checks

```sh
cargo fmt --all -- --check
cargo build --workspace --all-features --all-targets
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features --all-targets
cargo run -p locus-cli -- check spec/test/demonstrations/*.locus
cargo run -p locus-cli -- check spec/test/stress/*.locus
```

See [`spec/README.md`](spec/README.md) for the normative contract and [`documents/structure.md`](documents/structure.md) for the implementation roadmap.
