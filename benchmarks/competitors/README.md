# Locus, Mermaid, and D2 CLI speed comparison

This benchmark compares the user-visible command-line path from a source file to an SVG file.
Every measured iteration starts a fresh process, parses the source, performs layout and rendering,
and overwrites an SVG output. Process and browser startup are deliberately included.

The two workloads preserve equivalent semantic topology across the three languages:

| Workload | Structure | Locus source |
| --- | --- | --- |
| Agent flowchart | 22 nodes and 27 directed connections | `spec/test/stress/flowchart_agent_loop.locus` |
| Software architecture | 20 components, four groups, and 32 directed connections | `spec/test/stress/software_architecture.locus` |

The implementations are not feature-equivalent. Locus also resolves authored geometry and checks
persistent assertions. Mermaid and D2 perform automatic graph layout. Results therefore represent
complete CLI workflow latency for each language, not an isolated layout-algorithm comparison.

Install `mmdc` and `d2` using their official instructions, then run:

```sh
cargo build -p locus-cli --release
CARGO_TARGET_DIR="$PWD/target" cargo bench -p locus-cli --bench competitors
```

The explicit target directory prevents Criterion's runtime metadata lookup from contending with
the parent Cargo process on toolchains that retain the build lock while a benchmark is running.

The benchmark prints all three tool versions before measurement and writes Criterion reports to
`target/criterion/competitors/`. Override executable discovery when needed:

```sh
LOCUS_BIN=/path/to/locus \
LOCUS_MMDC=/path/to/mmdc \
LOCUS_D2=/path/to/d2 \
  CARGO_TARGET_DIR="$PWD/target" cargo bench -p locus-cli --bench competitors
```

Generated comparison SVGs are written to the ignored `target/competitor-bench/` directory.
