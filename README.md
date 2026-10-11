<p align="center">
  <a href="assets/locus-logo.locus">
    <img src="assets/locus-logo.svg" alt="Locus: deterministic geometry diagrams as code" width="420">
  </a>
</p>

# Locus

Locus is an agent-friendly relational language and CLI toolchain for deterministic plane-geometry figures and technical diagrams.

## Agent-friendly by design

The core goal of Locus is to let AI agents create, inspect, verify, and revise precise geometry without relying on visual feedback. Agents declare geometric relationships instead of guessing coordinates, use structured diagnostics and stable JSON output to understand failures, and verify intent through persistent assertions. Deterministic output and diffable `.locus` source files make every change reproducible and auditable.

The project is under active development and supports only the latest draft: `locus 0 beta 5`. Backward compatibility is not a goal yet. This draft uses single-line `meta key = "value"` metadata, scalar seeds directly after `free`, optional alpha in `#RRGGBBAA` colors, parameterless points, shape-owned placement, explicit circle-tangency branches, ellipses, and B-splines. See the [specification](spec/specification.md) and [demonstrations](spec/test/README.md).

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

### Rejected requests in development

Debug/dev CLI builds automatically save each failed invocation as a separate JSON
record in `.locus/rejected-requests/` relative to the working directory. Set
`LOCUS_REJECTED_REQUESTS_DIR` to choose another directory. Release builds compile
out this feature, even when that environment variable is set.

Records include error codes/names, messages, spans, notes and suggestions, the
complete command result, arguments, working directory, timestamp, CLI version,
original file contents, submitted `add` stdin, and candidate/evaluated source.
This preserves rejected edits without changing the original file, for reviewing
possible syntax improvements. Batch checks retain every input and its outcome,
including failed assertions and strict-mode warnings. Unreadable files retain the
read error; non-UTF-8 files retain their original bytes. Argument parsing failures
also snapshot available `.locus` file arguments. Successful commands, help, and
version requests produce no records. Archive write failures warn on stderr and
preserve the command's normal output and exit code. Records are local files;
remove the directory when you no longer need them.

## Editor integration

The [Zed extension](ecosystem/zed/README.md) adds syntax highlighting with the
bundled Tree-sitter grammar and connects `.locus` files to `locus-lsp` for
diagnostics, completion, hover, go-to-definition, and formatting.
See its README for local installation and server configuration.

## Focused feedback

Use `locus check figure.locus --layout --strict --json --compact` to include
annotation placement in validation without exporting an image. `--layout` uses
the same text measurement and placement as rendering. Layout warnings fail the
check with `--strict`; without it they remain non-fatal. Ordinary checks continue
to validate geometry and assertions without measuring text.

Collision diagnostics name the labels and intersecting geometry or annotation,
include source locations, and distinguish text-box overlap, stroke intersection,
and margin violation. Their `collision_details` contain bounds in scene units,
including the stroke-width padding used for intersection tests and the permitted
viewport for margin checks. Unbounded lines and rays omit `object_bounds`.
The same details and placement suggestions appear in render diagnostics.

Start with `locus check figure.locus --json --compact`. Compact JSON uses one
line, summarizes solver rank, remaining DOF and residual, counts assertions,
and retains every failed assertion's actual values and every diagnostic.
Omit `--compact` for the full solver and assertion reports. `--compact` requires
`--json` and is available on all commands.

`inspect` reports diagnostics by default. Request just the measurements needed
to investigate a specific issue:

```sh
locus inspect figure.locus --distance A B --json --compact
locus inspect figure.locus --angle A B C --ratio A M B --json --compact
locus inspect figure.locus --measurements --incidences --json
```

Repeat `--distance`, `--angle`, or `--ratio` for additional tuples. Angles use
the middle point as vertex and return radians; ratios require the middle point
to lie strictly between the endpoints and return `dist(A,M) / dist(M,B)`.
Unknown ids, non-point objects, and undefined measurements return structured
errors with exit 3. `--measurements` opts into all pairwise distances, triangle
angles, and line ratios; it conflicts with direct measurement queries.
`--incidences` independently opts into all detected geometric facts. Both can
produce large output. Checks compute diagnostics without building these tables.

Points declared with `mark: none` remain available to measurements, incidences,
and assertions, but do not cause visual crowding or out-of-view warnings.
Visible marker collisions and solve/construction diagnostics still apply.

## Rendering

Run `locus check figure.locus` first; after it exits 0, render for human review:

```sh
locus render figure.locus -o figure.svg
locus render figure.locus --tikz -o figure.tex
locus render figure.locus --stdout > figure.svg
cargo run -p locus-cli --features png -- render figure.locus --png -o figure.png
cargo run -p locus-cli --features png -- render figure.locus --png --background white --dpi 300 -o figure.png
```

SVG is the default. `--tikz` and feature-gated `--png` select one alternative
format; output extensions do not select a backend. Supply `-o`, `--stdout`, or
both (the file and stdout receive identical bytes). `--stdout` emits only the
artifact, including binary PNG bytes. Warnings go to stderr. Use `--json` with
`-o` for an output-path/format/diagnostics report; `--json` conflicts with
`--stdout`. Rendering independently parses and solves the source and does not
require a prior check or reject a scene merely because an assertion fails.

Use `--background white` for an opaque white background in any output format;
the default is `--background transparent`. For PNG, `--dpi` accepts a positive
integer (default: 96), increases or decreases the pixel dimensions relative to
96 DPI, and writes print-density metadata. For example, 300 DPI produces about
3.125 times as many pixels on each side at the same physical size. PNG exports
are limited to 64M pixels after scaling; `--dpi` requires `--png`.

Plain text uses bundled Noto Sans and Noto Sans CJK SC fonts with actual glyph
measurements. Chinese (Simplified and Traditional), Japanese, and Korean labels
render as outlines in SVG, TikZ, and PNG without installed fonts. Mixed Latin
and CJK text shares a baseline; shared Han glyphs use Simplified Chinese forms
for now. For example, `label name = label(c.center, "中文 日本語 한국어")`.

The default TeX engine is ReX, using bundled STIX Two Math outlines for
fractions, roots, scripts, operators, delimiters, and matrices. SVG, TikZ, and PNG
output needs no installed math font or external TeX process. Invalid or
unsupported TeX returns source-anchored E414 and exit 2; there is no silent
fallback. ReX supports a
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
