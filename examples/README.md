# Locus examples

The positive `.locus` corpus is split by purpose:

- [`demonstrations/`](demonstrations) contains twelve readable examples of the
  [`0 beta 6` specification](../spec/specification.md): canonical constructor declarations,
  derived geometry, measured content, and relational diagram layout.
- [`composition/`](composition) contains the reusable agent-loop and checkout sequence examples.
- [`stress/`](stress) contains 14 expanded workloads. These exercise large construction
  graphs, dense annotations, shared constraints, assertions, deterministic layout, and every
  rendering backend.

Invalid-input fixtures remain in [`tests/fixtures/errors/`](../tests/fixtures/errors) so they are not
mistaken for runnable examples.

All 28 runnable examples use `0 beta 6`. The source audit also covers the historical solver-limit fixture in `tests/fixtures/solver/`.
The 26 supported demonstration/stress sources and both composition examples pass the beta 6 measured solver, assertions and SVG/TikZ/PNG parity checks with bundled fonts. The historical solver-limit fixture remains a negative acceptance contract. Feasible preference settling may remain incomplete and produces a strict-check warning.

Run the available source audit from the repository root:

```sh
python3 scripts/check_examples.py
python3 scripts/check_examples.py --json
```

The [audit](../scripts/check_examples.py) checks headers, declaration forms, constructor arities,
name/reference order, nested-function expansion counts, group member references,
removed relation names, assert-only restrictions, and stress-manifest membership/minimum
object and assertion counts. It is not a full expression parser, dimensional type checker,
solver, or renderer; its success does not establish numerical assertions or layout
feasibility. Qualified-member resolution, overload typing, actual geometry, and rendering
are checked by the native runtime acceptance tests.

Run numerical and rendering acceptance with:

```sh
cargo run -p locus-cli -- check examples/demonstrations/*.locus
cargo run -p locus-cli -- check examples/stress/*.locus
cargo run -p locus-cli -- check examples/composition/*.locus
cargo test -p locus-cli -p locus-render --test stress_examples --all-features
cargo test -p locus-render --test corpus_acceptance --all-features
```

## Demonstrations

| File | Demonstrates |
| --- | --- |
| [aec3_filter_alignment.locus](demonstrations/aec3_filter_alignment.locus) | Filter-bank alignment and delay annotations |
| [angle_bisector_ratio.locus](demonstrations/angle_bisector_ratio.locus) | Triangle on a placed circumcircle, derived intersection, and the angle-bisector theorem |
| [bspline_controls.locus](demonstrations/bspline_controls.locus) | Five derived controls, two cubic spans, clamped endpoints, and an interior-knot sample |
| [ellipse_foci.locus](demonstrations/ellipse_foci.locus) | Rotated semiaxes, foci, containment, and area scaling |
| [euler_nine_point.locus](demonstrations/euler_nine_point.locus) | Euler line and nine-point circle constructions |
| [flowchart_agent_loop.locus](demonstrations/flowchart_agent_loop.locus) | Explicit flowchart geometry plus nested measured nodes, duplicate groups, title typography, orthogonal returns, and clipping |
| [inscribed_angle.locus](demonstrations/inscribed_angle.locus) | Inscribed-angle geometry and arc marks |
| [pentagon_golden.locus](demonstrations/pentagon_golden.locus) | Regular pentagon and golden-ratio relationships |
| [point_relations.locus](demonstrations/point_relations.locus) | Parameterless points, relative placement, a 3-4-5 triangle, and a derived circumcircle |
| [rounded_rect.locus](demonstrations/rounded_rect.locus) | Explicit rounded boxes/capsules plus an automatically sized diamond, measured caption, and curved connector |
| [sequence_check.locus](demonstrations/sequence_check.locus) | Explicit message geometry plus generic function instances, frame/tab layout, finite edges, preset dimensions, and clipped paths |
| [tangent_circles.locus](demonstrations/tangent_circles.locus) | Solved tangent circles and common tangents |

### `0 beta 6` coverage

The twelve demonstrations exercise parameterless points; dimension-only circles, ellipses,
rectangles, rounded rectangles, and diamonds; derived object aliases; explicit external and internal tangency;
and cubic, open, non-rational, clamped B-splines. The original 69 demonstration assertion
statements are retained. Function expansion and added layout cases bring the intended
demonstration total to 93 assertions and 365 declared objects, as counted by the source audit.

Same-kind constructors use `kind id(args)`; aliases and differently named constructions
retain `= rhs`, such as `point M = midpoint(A, B)` and `arc m = angle_arc(...)`.
`horizontal`, `vertical`, and `coincident` are replaced by `align_center_y`,
`align_center_x`, and `center`. Segment congruence becomes numeric length equality.
Measured relationships, explicit dimensions, numeric seeds, construction branches, styles,
and the original mathematical assertions are preserved.

The measured-layout panels additionally exercise functions with object/text/class arguments,
nested calls and explicit style overrides, atomic instance members in ordered groups,
nested-group flattening/deduplication, automatic rectangle/diamond/rounded-box sizing,
standalone and path-attached labels, typography roles, page-relative label preferences,
orthogonal and curved routes, logical length after clipping, `cover_x`/`cover_y`, symbolic
flush insets, rectangle `edge` values, `short_length`/`narrow_width`, and member-wise avoidance.
The larger flowchart creates sixteen measured node pairs, so template reuse is exercised
as an expanded workload rather than counted as only one template body.

Public reports intentionally describe resolved relationships and measurements
without exposing Locus-owned initialization coordinates. Stress workloads use parameterless points,
shape-owned placement, relative constructions, and ordered internal circle tangency.
Free numeric lengths, angles, and branch magnitudes retain deliberate perturbations to
exercise convergence without authoring point coordinates or placement hints.

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
threshold lowered. Across both directories, all 1,335 original assertion statements remain;
the additional expanded cases bring the total to 1,467, including the three historical-limit
assertions. Existing relative placement, nonlinear branch magnitudes, and free numeric
seeds retain their original values.

The native stress runners already verify manifest discovery, solve reports, and rendering.
These checks run against the current parser, evaluator, and measured-layout runtime.

The renderer test starts from source and repeats **parse → solve → assert → measure → layout
→ SVG/TikZ/PNG** twice per case. It compares the complete solved scene, solver and assertion
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

An opt-in [competitor benchmark](../benchmarks/competitors/README.md) measures complete
source-to-SVG CLI latency for Locus, Mermaid, and D2 on equivalent agent-flowchart and
software-architecture workloads.

## Specification pressure points

- A function instance remains one item in row/column groups. Expanding it into separate
  captions and shapes would break the node-pair and composed-sequence layouts.
- Namespace bounds include owned visible members, not external guides supplied as arguments;
  group reuse must neither duplicate drawing objects nor add placements.
- Text dimensions must be measured before containment and shared-size equalities resolve.
  Typography, automatic sizing preferences, and preset metrics follow §8.7 of the specification.
- `short_length` must be compatible with automatic point separation. `narrow_width` participates
  in guide spacing and must not be treated as an authored coordinate or a domain-specific width.
- Clipping must preserve logical `.length` and endpoint incidence, while path captions and
  arrowheads follow the retained visible route. Clearance, clipping, and measurement dependencies
  need the joint-layout behavior and numerical contract identified in Part 12 of the specification.

These are acceptance requirements exposed by the source corpus, not claims that the source
audit proves geometry, collision-free placement, preset suitability, or solver convergence.

## Historical solver limitation

[`stress/known_limits/closed_segment_bisectors.locus`](../tests/fixtures/solver/closed_segment_bisectors.locus)
preserves a small failure uncovered in the beta 5 runtime: three simultaneous closed-segment
bisectors stall with `E403`. The unchanged mathematical case now uses beta 6 syntax and is
verified by the current negative acceptance test. Its header/syntax rejection by the old parser
is not the historical solver failure. The native corpus test checks the source-anchored convergence error and a passing supporting-line variant with the same `between` assertions.

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
