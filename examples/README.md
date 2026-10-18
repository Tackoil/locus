# Locus examples

The positive `.locus` corpus is split by purpose:

- [`demonstrations/`](demonstrations/README.md) contains 43 readable examples of the
  [`0 beta 6` specification](../spec/specification.md): geometry, measured content,
  reusable functions, and relational layouts spanning 31 diagram types.
- [`stress/`](stress) contains 14 expanded workloads. These exercise large construction
  graphs, dense annotations, shared constraints, assertions, deterministic layout, and every
  rendering backend.

Invalid-input fixtures remain in [`tests/fixtures/errors/`](../tests/fixtures/errors) so they are not
mistaken for runnable examples.

All 57 runnable examples use `0 beta 6`. The source audit also covers the historical solver-limit fixture in `tests/fixtures/solver/`.
All 43 demonstrations and 14 stress examples participate in the beta 6 measured-solver, assertion, and SVG/TikZ/PNG parity checks with bundled fonts. The historical solver-limit fixture remains a negative acceptance contract. Feasible preference settling may remain incomplete and produces a strict-check warning.

Run the available source audit from the repository root:

```sh
python3 scripts/check_examples.py
python3 scripts/check_examples.py --json
```

The [audit](../scripts/check_examples.py) checks headers, declaration forms, constructor arities,
name/reference order, nested-function expansion counts, group member references,
relation vocabulary, assert-only restrictions, and stress-manifest membership/minimum
object and assertion counts. It is not a full expression parser, dimensional type checker,
solver, or renderer; its success does not establish numerical assertions or layout
feasibility. Qualified-member resolution, overload typing, actual geometry, and rendering
are checked by the native runtime acceptance tests.

Run numerical and rendering acceptance with:

```sh
cargo run -p locus-cli -- check examples/demonstrations/*.locus
cargo run -p locus-cli -- check examples/stress/*.locus
cargo test -p locus-cli -p locus-render --test stress_examples --all-features
cargo test -p locus-render --test corpus_acceptance --all-features
```

## Demonstrations

The [demonstration catalog](demonstrations/README.md) lists all 43 figures: twelve
geometry and technical examples plus 31 diagram types, including the canonical
reusable agent-loop and checkout examples.

### `0 beta 6` coverage

The demonstrations exercise parameterless points; dimension-only circles, ellipses,
rectangles, rounded rectangles, and diamonds; derived object aliases; explicit external and internal tangency;
and cubic, open, non-rational, clamped B-splines. The unified demonstration corpus
contains 164 assertion statements and 1,313 declared objects after expansion,
as counted by the source audit.

Same-kind constructors use `kind id(args)`; aliases and differently named constructions
retain `= rhs`, such as `point M = midpoint(A, B)` and `arc m = angle_arc(...)`.
Point alignment and coincidence use `align_center_y`, `align_center_x`, and `center`.
Equal segment lengths use numeric equality.
Measured relationships, explicit dimensions, numeric seeds, construction branches, styles,
and the original mathematical assertions are preserved.

The measured-layout panels additionally exercise functions with object/text/class arguments,
nested calls and explicit style overrides, atomic instance members in ordered groups,
nested-group flattening/deduplication, automatic rectangle/diamond/rounded-box sizing,
standalone and path-attached labels, typography roles, page-relative label preferences,
orthogonal and curved routes, logical length after clipping, `cover_x`/`cover_y`, symbolic
flush insets, rectangle `edge` values, `short_length`/`narrow_width`, and member-wise avoidance.
The stress flowchart creates sixteen measured node pairs, so template reuse is exercised
as an expanded workload rather than counted as only one template body.

Public reports describe resolved relationships and measurements.
Stress workloads use parameterless points,
shape-owned placement, relative constructions, and ordered internal circle tangency.
Free numeric lengths, angles, and branch magnitudes retain deliberate perturbations to
exercise convergence.

## Stress workloads

| File | Stress coverage | Objects | Assertions |
| --- | --- | ---: | ---: |
| [aec3_filter_alignment.locus](stress/aec3_filter_alignment.locus) | Five filter banks, 64 additional coefficient stems, shared delay estimates and range reduction | 241 | 149 |
| [angle_bisector_ratio.locus](stress/angle_bisector_ratio.locus) | Three solved cevians, Ceva identity, four coupled triangle-center networks | 75 | 29 |
| [euler_nine_point.locus](stress/euler_nine_point.locus) | Five medial levels, Euler and nine-point constructions, Feuerbach contact across shrinking scales | 93 | 32 |
| [flowchart_agent_loop.locus](stress/flowchart_agent_loop.locus) | Original authoring/solver-state graph plus 16 nested measured pairs, repeated groups, orthogonal returns, and clipped forward paths | 225 | 125 |
| [gantt_chart.locus](stress/gantt_chart.locus) | 20 solved durations, four parallel teams, five phases, 32 dependencies, progress bars, release join | 216 | 117 |
| [inscribed_angle.locus](stress/inscribed_angle.locus) | 16 shared-endpoint chord fans, opposite arcs, supplementary angles, perpendicular chord bisectors | 116 | 51 |
| [kanban_board.locus](stress/kanban_board.locus) | 37 cards, three teams, five states, cross-team blockers, corner containment and WIP | 113 | 61 |
| [pentagon_golden.locus](stress/pentagon_golden.locus) | Five nested pentagrams, diagonal intersections, equal sides and golden-ratio scaling | 79 | 40 |
| [right_triangle_altitude.locus](stress/right_triangle_altitude.locus) | 14 recursive child triangles, altitude identities, incircle contacts and area checks | 131 | 78 |
| [sankey_energy.locus](stress/sankey_energy.locus) | 28 bands, three sources, four intermediate processes, four outputs, split/merge conservation | 202 | 94 |
| [sequence_check.locus](stress/sequence_check.locus) | Original four-participant/26-message graph plus a composed measured sequence, divided frame, legend, narrow overlay, and clipping | 181 | 58 |
| [software_architecture.locus](stress/software_architecture.locus) | 20 components, four boundaries, 32 shared dependency edges, placed service anchors | 100 | 39 |
| [tangent_circles.locus](stress/tangent_circles.locus) | Noncollinear three-circle solve feeding seven further contacts and common tangents | 83 | 36 |
| [tangent_internal_branches.locus](stress/tangent_internal_branches.locus) | Eight additional nonlinear internal contacts, radial carriers, both exterior tangent branches | 119 | 51 |

Counts include geometry, numbers, and annotations after function expansion; group and
instance namespaces contribute no extra objects. The fourteen positive stress sources
contain 1,974 declared objects and 960 intended assertion statements. Purely derived figures and layouts fully initialized from their relations can
legitimately take zero solver iterations.

## Automated stress contract

[`stress/manifest.tsv`](stress/manifest.tsv) retains the per-file minimum object, assertion,
equation, and numerical-attempt counts for the original workloads. The source audit checks
discovery and expanded object/assertion minima; equation/attempt counts require the native
runtime and are not inferred by the audit. No original assertion was removed or workload
threshold lowered. The 43 demonstrations and 14 stress sources contain 1,124
assertion statements after expansion; the audit also checks three assertions in
the historical solver fixture. Existing stress placement, nonlinear branch magnitudes,
and free numeric seeds retain their original values.

The native stress runners already verify manifest discovery, solve reports, and rendering.
These checks run against the current parser, evaluator, and measured-layout runtime.

The renderer test starts from source and repeats **parse → expand → measure → solve/layout
feedback → assert → SVG/TikZ/PNG** twice per case. It compares the complete solved scene, solver and assertion
reports, layout including diagnostics, and output bytes. Successful solves must retain the
specified residual threshold; assertion tolerances are unchanged. No wall-clock performance
threshold is used.

For a longer run with per-case counts and elapsed times:

```sh
LOCUS_STRESS_PASSES=10 \
  cargo test -p locus-render --test stress_examples --all-features -- --nocapture
```

`LOCUS_STRESS_PASSES` accepts 2–100 passes.

For statistically sampled performance measurements of parsing, solving, and the complete
parse-to-SVG pipeline on a representative nonlinear stress workload:

```sh
cargo bench -p locus-render --bench pipeline
```

Criterion writes detailed reports under `target/criterion/`. The benchmark is intentionally
separate from the correctness suite so ordinary tests retain no machine-speed threshold.

## Specification pressure points

- A function instance remains one item in row/column groups. Expanding it into separate
  captions and shapes would break the node-pair and composed-sequence layouts.
- Namespace bounds include owned visible members, not external guides supplied as arguments;
  group reuse must neither duplicate drawing objects nor add placements.
- Text dimensions must be measured before containment and shared-size equalities resolve.
  Typography, automatic sizing preferences, and preset metrics follow §8.7 of the specification.
- `short_length` must be compatible with automatic point separation. `narrow_width` participates
  in guide spacing as a shared preset length.
- Clipping must preserve logical `.length` and endpoint incidence, while path captions and
  arrowheads follow the retained visible route. Clearance, clipping, and measurement dependencies
  need the joint-layout behavior and numerical contract identified in Part 12 of the specification.

These are acceptance requirements exposed by the source corpus, not claims that the source
audit proves geometry, collision-free placement, preset suitability, or solver convergence.

## Historical solver limitation

[`stress/known_limits/closed_segment_bisectors.locus`](../tests/fixtures/solver/closed_segment_bisectors.locus)
preserves a small failure uncovered in the beta 5 runtime: three simultaneous closed-segment
bisectors stall with `E403`. The unchanged mathematical case now uses beta 6 syntax and is
verified by the current negative acceptance test. The native corpus test checks the source-anchored convergence error and a passing supporting-line variant with the same `between` assertions.

## Representations and previews

- Sankey thickness is vertical at the ports. Routing is explicit. Gantt time is numeric days,
  and Kanban WIP is computed from the explicitly listed active cards.
- Dense geometry and crossing connectors may produce crowding or collision diagnostics.
  Repeated runs must return the same layout and diagnostics.
- Plain labels use bundled Noto Sans outlines. TeX segments use `RexEngine` and
  bundled STIX Two Math outlines, matching the CLI's default math renderer.

Export all 14 stress previews into the ignored
`target/spec-examples` directory. SVG,
TikZ, and PNG use an opaque white viewport background; other renderer callers retain
transparent output unless they set `RenderOptions.background`. The export also creates
`index.html`, a gallery linking every full-size vector drawing:

```sh
LOCUS_EXAMPLE_OUTPUT_DIR="$PWD/target/spec-examples" \
  cargo test -p locus-render --test stress_examples --all-features
```
