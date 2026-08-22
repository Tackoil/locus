# Locus

Locus is an agent-friendly constructive language and CLI toolchain for deterministic plane-geometry figures.

## Agent-friendly by design

The core goal of Locus is to let AI agents create, inspect, verify, and revise precise geometry without relying on visual feedback. Agents declare geometric relationships instead of guessing coordinates, use structured diagnostics and stable JSON output to understand failures, and verify intent through persistent assertions. Deterministic output and diffable `.locus` source files make every change reproducible and auditable.

This repository is at the Phase 0 bootstrap boundary. It contains the pinned Rust workspace, crate dependency seams, normative v0.2 specification package, initial machine-readable schemas, and CI quality gates. Language parsing, geometry evaluation, layout, and rendering behavior are intentionally not implemented yet.

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
