<p align="center">
  <a href="assets/locus-banner.locus">
    <img src="assets/locus-banner.svg" alt="Locus: deterministic geometry diagrams as code" width="420">
  </a>
</p>

# Locus

Locus is an agent-friendly relational language and CLI toolchain for deterministic plane-geometry figures and technical diagrams.

## Agent-friendly by design

The core goal of Locus is to let AI agents create, inspect, verify, and revise precise geometry through text feedback. Agents declare geometric relationships, use structured diagnostics and stable JSON output to understand failures, and verify intent through persistent assertions. Deterministic output and diffable `.locus` source files make every change reproducible and auditable.

The project is under active development. The `locus 0 beta 6` frontend supports constructor declarations, reusable functions, ordered groups, and named instances. The beta 6 solver supports measured layout, hard separation/containment/avoidance requirements, continuous routes and path labels, endpoint clipping, bounded solve/layout feedback, and reports and safe edits for instances and groups. The confirmed rules and presets are pinned in the [specification](spec/specification.md); feasible preference settling can remain incomplete and is reported as a strict-check warning. Only `locus 0 beta 6` files are accepted. See the [specification](spec/specification.md) and [demonstrations](examples/README.md).

## Repository layout

| Directory | Contents |
| --- | --- |
| [`crates/`](crates/) | Rust libraries, CLI, language server, and crate-local tests |
| [`spec/`](spec/) | Normative language and toolchain specification and reference diagrams |
| [`examples/`](examples/README.md) | Runnable geometry and diagram demonstrations, plus stress workloads |
| [`tests/`](tests/README.md) | Shared fixtures, expected outputs, and stress-test support |
| [`scripts/`](scripts/) | Repository checks, including the example source audit |
| [`ecosystem/`](ecosystem/) | Tree-sitter grammar and Zed editor integration |
| [`benchmarks/`](benchmarks/) | Agent drawing benchmark harness and fixtures |
| [`skills/locus/`](skills/locus/SKILL.md) | Beta 6 authoring skill and portable language reference |
| [`assets/`](assets/) | Project logo source and artwork |
| [`website/`](website/README.md) | Project introduction, rendered demo gallery, and documentation website |
| [`licenses/`](licenses/) | Supplemental third-party license texts, provenance, and ReX licensing investigation |
| [`vendor/`](vendor/) | Pinned compatibility patches for dependencies |

Local development plans and generated validation output live under the ignored
`.local/plans/` and `.local/artifacts/` directories. Build output stays in `target/`.
Name tests and fixtures after the behavior they cover; format revisions belong in
file headers and compatibility assertions.

## License

Locus's own code is available under either the [MIT license](LICENSE-MIT) or
the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
Third-party dependencies and bundled fonts retain their respective terms;
the [third-party notices](THIRD-PARTY-NOTICES.md) include their license texts
and attributions. Preserve these notices when distributing applicable components.

ReX's upstream license documentation is incomplete, and its generated tables
have additional provenance questions. The CLI includes ReX in ordinary builds.
See the [ReX licensing investigation](licenses/REX-LICENSING.md) for the evidence
and unresolved items before preparing a release.

To refresh the notices after dependency updates:

```sh
cargo fetch --locked
cargo fetch --locked --manifest-path ecosystem/zed/Cargo.toml
python3 scripts/generate_third_party_notices.py
python3 scripts/generate_third_party_notices.py --check
```

The collector reads the locked dependencies across all features and platforms,
including development dependencies. Supplemental upstream texts and checksums
are kept in `licenses/third-party/sources.json`. Missing notice texts cause
generation to fail rather than being assigned an assumed license.

## Development checks

```sh
python3 scripts/check_examples.py
cargo fmt --all -- --check
cargo build --workspace --all-features --all-targets
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features --all-targets
cargo run -p locus-cli -- check examples/demonstrations/*.locus
cargo run -p locus-cli -- check examples/stress/*.locus
```

See the [specification](spec/specification.md) for the normative contract and the [test corpus](examples/README.md) for runnable examples.

### Pipeline timing metrics

Add `--timings` to any CLI command for an indented timing report on stderr:

```sh
cargo run --release -p locus-cli -- render figure.locus -o figure.svg --timings
cargo run --release -p locus-cli -- check figure.locus --json --timings=json 2> timings.json
```

Timings cover file reads, parsing, expansion, text measurement (including each
label), evaluation preparation, solver initialization, continuous candidates,
Levenberg–Marquardt solving, preference settling, solution finalization, repeated
measurement/layout passes, annotation placement, clipping, routing, collision
reports, inspection/assertions, rendering, file writes, and command output as
applicable. Batch checks retain a separate scope for each file. Failed stages
are recorded too, and timing collection preserves command results and exit codes.
Argument parsing and process startup are outside the measured operation.

`--timings=json` writes one JSON report to stderr after the command finishes.
It contains `total_ms` and a `steps` array in start order; each step has `name`,
`parent` (an index into `steps`, or null), `start_ms`, and `duration_ms`.
Command diagnostics may precede the report on stderr; use `--json` for command
output to stdout when capturing a standalone timing JSON file. Each duration is
inclusive wall-clock time in milliseconds, so parent and child durations must
not be added together. Repeated passes and candidates have separate entries.
Use release builds for representative measurements; debug builds also time
rejected-request archiving. Timing reports themselves are outside `total_ms`.

Collection is disabled by default, with no clock reads or timing allocations
at instrumented library stages. Library consumers can wrap a synchronous
operation with `locus_core::metrics::collect(|| ...)` to receive its result and
a serializable timing report, and use `span` or `measure` for custom stages.
Collections are local to the current thread; child threads collect separately.

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

## Authoring skill and drawing benchmark

The [Locus skill](skills/locus/SKILL.md) teaches the current beta 6 authoring and
verification workflow. The [agent drawing benchmark](benchmarks/locus/README.md)
compares it with handwritten native SVG on the same triangle, sequence, and
flowchart tasks. It freezes trial inputs, retains failed attempts, and keeps
image review separate from native syntax checks.

## Editor integration

The [Zed extension](ecosystem/zed/README.md) adds syntax highlighting with the
bundled Tree-sitter grammar and connects `.locus` files to `locus-lsp` for
diagnostics, completion, hover, go-to-definition, and formatting.
See its README for local installation and server configuration.

## Focused feedback

Use `locus check figure.locus --layout --strict --json --compact` to include
annotation placement in validation without exporting an image. `--layout` uses
the same text measurement and placement as rendering. Layout warnings fail the
check with `--strict`; without it they remain non-fatal. Every check measures
declared text with the bundled fonts before solving, so label dimensions and
typography roles participate in layout relations even without `--layout`.
Required routing, path-label placement, and endpoint clipping run in every check,
describe, inspect, render, and edit validation. The engine repeats solving and
layout up to three times to agree on accepted routes and measured label boxes.
Clipping changes visible ends while preserving logical length and bounds. A
required routing or feedback conflict returns E413 and exit 2; ordinary collision
warnings are included in checks with `--layout`.

Reusable instances and groups support top-level `update --id` and dependency-checked `remove --id`. `describe --json` includes normalized collection membership, invocation chains, expanded relations and accepted dimensions, typography and route metadata. Qualified member edits identify the function body and call so you can edit the source deliberately. `fmt --json` preserves source definitions/calls/groups alongside expanded records.

Library consumers can use `locus_layout::measurement::measure_file` before
`locus_layout::solve_layout` for the same measured solve and layout cycle. The
core accepts renderer-independent `EvaluationInputs` and has no font dependency.
Build rendering styles with `StyleSheet::from_file(&scene.file)` after solving;
the accepted scene contains expanded function calls and substituted style classes.

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

Failed assertions include a `failure` object with the exact source expression,
comparisons labeled `actual` (left operand) and `expected` (right operand),
and relevant upstream declarations and solver relations in source order. Every
measured subterm includes its expression and unit: `unitless`, `scene units`,
`scene units squared`, or `rad`. Angle values remain in radians even when the
source uses degrees. Predicate and negated-predicate failures report their
boolean result against `true`. The same context appears in human diagnostics.
For example, `vdist(start, end) == -0.7` shows an actual value of `-0.8` together
with the upstream `rel vdist(start, end) == -0.8` dimension, so a mistaken expectation
can be distinguished from an incorrect construction.

Invalid annotation properties retain their type or domain diagnostic and source
location. Conflicting `text_block` `place` and `anchor` properties return `E209`.

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

### In a browser

The [`locus-wasm` library](crates/locus-wasm/README.md) compiles the same measured
solve/layout and SVG pipeline to WebAssembly, including bundled text and math
fonts. Its JavaScript API renders `.locus` source to SVG with structured
diagnostics and assertion results, without a rendering server. See the library
README for build instructions, integration, and WASM checks.

### From the CLI

Run `locus check figure.locus` first; after it exits 0, render for human review:

```sh
locus render figure.locus -o figure.svg
locus render figure.locus --tikz -o figure.tex
locus render figure.locus --stdout > figure.svg
cargo run -p locus-cli --features png -- render figure.locus --png -o figure.png
cargo run -p locus-cli --features png -- render figure.locus --png --background white --dpi 300 -o figure.png
```

Rendering does not create a dump unless `--dump PATH` is supplied. This opt-in
capture writes a versioned JSON file containing render snapshots. Dump recording
adds runtime and memory overhead, especially for scenes requiring many solver trials.
`--no-dump` explicitly disables capture and conflicts with `--dump`.

```sh
locus render figure.locus -o figure.svg --dump figure.trace.json
locus visualize figure.trace.json -o figure.trace.html
```

Open the generated HTML file in a browser. It works offline and includes the
original source, so the `.locus` file is no longer needed. Browse parsing,
expansion, text measurements, every numerical solver trial (including rejected
trials), preference-settling iterations and evaluations, each solve/layout pass,
and the final export. Geometry steps include SVG previews; other steps display
their structured results. Failed renders retain completed steps and diagnostics.

Hover over a shape or a group boundary to see its original identifier and source
location, including qualified names from function instances. Select two objects
or groups to list authored constraints, declaration dependencies, and group
membership connecting them or their members. Group outlines and names appear
only while hovering. Hover over an object or group to show authored constraints
that explicitly reference it as labeled connectors on the figure. Hover over a listed
relationship to isolate that connection and highlight its participants; moving
the pointer away clears the overlays. `locus visualize dump.json` defaults to `dump.html`.

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
for now. For example, `label name(c.center, "中文 日本語 한국어")`.

The default TeX engine is ReX, using bundled STIX Two Math outlines for
fractions, roots, scripts, operators, delimiters, and matrices. SVG, TikZ, and PNG
output needs no installed math font or external TeX process. Invalid or
unsupported TeX returns source-anchored E414 and exit 2; there is no silent
fallback. ReX supports a
subset of TeX, not a full LaTeX installation; see the
[integration notes](crates/locus-tex/README.md).
For deterministic pipeline testing only, the hidden `--tex-engine=mock` flag
selects `MockEngine` for TeX fragments while retaining real plain-text rendering:

```sh
locus render figure.locus --tex-engine=mock -o figure.svg
```

The mock reserves deterministic TeX boxes but supplies no visible glyphs. It is
suitable for isolated pipeline tests, not finished mathematical typography. See
[the golden suite](tests/fixtures/golden/README.md) for coverage and review notes.
