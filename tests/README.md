# Shared test data and support

Executable tests live in each Rust crate's `tests/` directory or beside the code
as unit tests. This directory holds data and support shared across crates.

| Path | Purpose |
| --- | --- |
| [`fixtures/errors/`](fixtures/errors/) | Invalid sources and diagnostic contracts |
| [`fixtures/solver/`](fixtures/solver/) | Known solver failures, separate from runnable examples |
| [`fixtures/programs/expansion/`](fixtures/programs/expansion/) | Function expansion and solve preparation |
| [`fixtures/programs/measurement/`](fixtures/programs/measurement/) | Automatic dimensions, measured content, and preset metrics |
| [`fixtures/programs/layout/`](fixtures/programs/layout/) | Containment, collections, clearance, and preference settling |
| [`fixtures/programs/routing/`](fixtures/programs/routing/) | Routed paths, attached labels, and endpoint clipping |
| [`fixtures/golden/`](fixtures/golden/README.md) | Exact demonstration JSON and SVG expectations |
| [`fixtures/text_measurements.json`](fixtures/text_measurements.json) | Bundled-font measurement expectations for the example corpus |
| [`support/stress.rs`](support/stress.rs) | Shared workload discovery and minimum-work contracts |

Runnable sources live in [`examples/`](../examples/README.md). Stress workloads are
registered in [`examples/stress/manifest.tsv`](../examples/stress/manifest.tsv).
Keep Rust snapshot files beside the crate-local tests that own them.

Run the source audit and workspace checks from the repository root:

```sh
python3 scripts/check_examples.py
cargo test --workspace --all-features --all-targets
```

For the focused source and measurement checks:

```sh
cargo test -p locus-syntax --test example_corpus
cargo test -p locus-render --all-features --test corpus_acceptance core_corpus_measurement_fixture_matches_real_font_engines
```

For render dump capture and the standalone viewer:

```sh
cargo test -p locus-cli --features png --test render_dump --test render_options
node --test crates/locus-cli/tests/visualizer.test.cjs
```

The viewer logic tests use only Node's built-in test runner; the generated HTML
requires no Node installation or external assets.

Measure full release exports of all 42 demonstrations with:

```sh
cargo build --release --locked -p locus-cli --features png
python3 scripts/benchmark_render.py --formats svg png tikz --runs 3 --limit 1
```

The benchmark launches a fresh CLI process for every export, disables diagnostic
solver dumps, and verifies each output file. It warms the executable once to exclude
new-binary OS verification. The JSON report records every elapsed time, platform,
and executable hash; any failed export or run over the limit fails the command.
Run it without concurrent builds or tests. The one-second target applies to this
release export workload on the measured machine, rather than arbitrary input sizes.
