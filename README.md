<p align="center">
  <a href="assets/locus-logo.locus">
    <img src="assets/locus-logo.svg" alt="Locus: deterministic geometry diagrams as code" width="420">
  </a>
</p>

# Locus

Locus is an agent-friendly relational language and CLI toolchain for deterministic plane-geometry figures and technical diagrams.

## Agent-friendly by design

The core goal of Locus is to let AI agents create, inspect, verify, and revise precise geometry without relying on visual feedback. Agents declare geometric relationships instead of guessing coordinates, use structured diagnostics and stable JSON output to understand failures, and verify intent through persistent assertions. Deterministic output and diffable `.locus` source files make every change reproducible and auditable.

The Rust workspace implements the v0.3 (`locus 2`) pipeline. The current [v0.4 specification](documents/specification.md) and [demonstrations](spec/test/README.md) target `locus 3`, with parameterless points, shape-owned placement, ellipses, and B-splines. Implementation migration is pending; the existing stress fixtures remain on format 2.

## Development checks

```sh
python3 -m pip install -r requirements-dev.txt
python3 scripts/check_spec.py
cargo fmt --all -- --check
cargo build --workspace --all-features --all-targets
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features --all-targets
```

See [`spec/README.md`](spec/README.md) for the normative contract and [`documents/structure.md`](documents/structure.md) for the implementation roadmap.
