<p align="center">
  <a href="assets/locus-logo.locus">
    <img src="assets/locus-logo.svg" alt="Locus: deterministic geometry diagrams as code" width="420">
  </a>
</p>

# Locus

Locus is an agent-friendly relational language and CLI toolchain for deterministic plane-geometry figures and technical diagrams.

## Agent-friendly by design

The core goal of Locus is to let AI agents create, inspect, verify, and revise precise geometry without relying on visual feedback. Agents declare geometric relationships instead of guessing coordinates, use structured diagnostics and stable JSON output to understand failures, and verify intent through persistent assertions. Deterministic output and diffable `.locus` source files make every change reproducible and auditable.

The project is under active development and supports only the latest draft: `locus 0 beta 4`. Backward compatibility is not a goal yet. This draft uses parameterless points, shape-owned placement, explicit circle-tangency branches, ellipses, and B-splines. See the [specification](spec/specification.md) and [demonstrations](spec/test/README.md).

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

## Editor integration

The [Zed extension](ecosystem/zed/README.md) adds syntax highlighting with the
bundled Tree-sitter grammar and connects `.locus` files to `locus-lsp` for
diagnostics, completion, hover, go-to-definition, and formatting.
See its README for local installation and server configuration.

## Rendering

Run `locus check figure.locus` first; after it exits 0, render for human review:

```sh
locus render figure.locus -o figure.svg
locus render figure.locus --tikz -o figure.tex
locus render figure.locus --stdout > figure.svg
cargo run -p locus-cli --features png -- render figure.locus --png -o figure.png
```

SVG is the default. `--tikz` and feature-gated `--png` select one alternative
format; output extensions do not select a backend. Supply `-o`, `--stdout`, or
both (the file and stdout receive identical bytes). `--stdout` emits only the
artifact, including binary PNG bytes. Warnings go to stderr. Use `--json` with
`-o` for an output-path/format/diagnostics report; `--json` conflicts with
`--stdout`. Rendering independently parses and solves the source and does not
require a prior check or reject a scene merely because an assertion fails.

Plain text uses the bundled font and actual glyph measurements. The default
TeX engine is ReX, using bundled STIX Two Math outlines for fractions, roots,
scripts, operators, delimiters, and matrices. SVG, TikZ, and PNG output needs no
installed math font or external TeX process. Invalid or unsupported TeX returns
source-anchored E414 and exit 2; there is no silent fallback. ReX supports a
subset of TeX, not a full LaTeX installation; see the
[integration notes](crates/locus-tex/REX-VIABILITY.md).
For deterministic pipeline testing only, the hidden `--tex-engine=mock` flag
selects `MockEngine` for TeX fragments while retaining real plain-text rendering:

```sh
locus render figure.locus --tex-engine=mock -o figure.svg
```

The mock reserves deterministic TeX boxes but supplies no visible glyphs. It is
suitable for isolated pipeline tests, not finished mathematical typography. See
[the golden suite](fixtures/golden/README.md) for coverage and review notes.
