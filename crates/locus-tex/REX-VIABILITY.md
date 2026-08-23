# ReX viability

Last tested: 2026-08-24

- upstream: <https://github.com/ReTeX/ReX>
- revision: `7bd4efc0b77c6910b79668d32824b464ee0958a3`
- toolchain: Rust `1.88.0`
- command: `cargo +1.88.0 check --lib`
- result: unavailable

The build fails while compiling ReX's bundled `stix2` crate. The
`static_map!` procedural macro panics for the generated glyph, symbol, and
variant tables with `assertion failed: input.starts_with(LEADING)`. Follow-on
errors report that `__static_map__construct_map` cannot be found.

To retest, clone the upstream repository into a temporary directory and run the
command above. If it succeeds on the workspace's pinned toolchain, replace the
feature-gated unavailable implementation with the real backend and update this
record with the tested revision and date.
