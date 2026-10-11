# Locus demonstrations and stress tests

The [one-shot live LLM test](llm/README.md) asks DeepSeek Flash to generate a
Locus file and renders its response, adding only a missing terminal newline.
It is opt-in and requires an API key.

The positive `.locus` corpus is split by purpose:

- [`demonstrations/`](demonstrations/) contains twelve compact, readable examples of the
  [`0 beta 5` language](../specification.md): points and shapes own placement;
  anchors and constructions remain derived.
- [`stress/`](stress/) contains 16 expanded workloads. These exercise large construction
  graphs, dense annotations, shared constraints, assertions, deterministic layout, and every
  rendering backend.

Invalid-input fixtures remain in [`fixtures/errors/`](../../fixtures/errors/) so they are not
mistaken for runnable examples.

Run the checks from the repository root. The demonstrations are the `0 beta 5` acceptance
suite, and all stress sources, including the known solver-limit fixture, use the same draft.

```sh
cargo run -p locus-cli -- check spec/test/demonstrations/*.locus
cargo run -p locus-cli -- check spec/test/stress/*.locus
cargo test -p locus-cli -p locus-render --test spec_examples --all-features
```

## Demonstrations

| File | Demonstrates |
| --- | --- |
| [aec3_filter_alignment.locus](demonstrations/aec3_filter_alignment.locus) | Filter-bank alignment and delay annotations |
| [angle_bisector_ratio.locus](demonstrations/angle_bisector_ratio.locus) | Triangle on a placed circumcircle, derived intersection, and the angle-bisector theorem |
| [bspline_controls.locus](demonstrations/bspline_controls.locus) | Five derived controls, two cubic spans, clamped endpoints, and an interior-knot sample |
| [ellipse_foci.locus](demonstrations/ellipse_foci.locus) | Rotated semiaxes, foci, containment, and area scaling |
| [euler_nine_point.locus](demonstrations/euler_nine_point.locus) | Euler line and nine-point circle constructions |
| [flowchart_agent_loop.locus](demonstrations/flowchart_agent_loop.locus) | Flowchart nodes, connectors, and branches |
| [inscribed_angle.locus](demonstrations/inscribed_angle.locus) | Inscribed-angle geometry and arc marks |
| [pentagon_golden.locus](demonstrations/pentagon_golden.locus) | Regular pentagon and golden-ratio relationships |
| [point_relations.locus](demonstrations/point_relations.locus) | Parameterless points, relative placement, a 3-4-5 triangle, and a derived circumcircle |
| [rounded_rect.locus](demonstrations/rounded_rect.locus) | Rounded boxes, capsules, labels, connectors, and curved boundary assertions |
| [sequence_check.locus](demonstrations/sequence_check.locus) | Sequence-diagram participants and messages |
| [tangent_circles.locus](demonstrations/tangent_circles.locus) | Solved tangent circles and common tangents |

### `0 beta 5` coverage

The twelve demonstrations exercise parameterless points; dimension-only circles, ellipses,
rectangles, rounded rectangles, and diamonds; derived object aliases; explicit external and internal tangency;
and cubic, open, non-rational, clamped B-splines. Together they contain 69 assertions that
run through the native parser, solver, evaluator, inspection, and rendering pipeline.

Public reports intentionally describe resolved relationships and measurements
without exposing Locus-owned initialization coordinates. Stress workloads use parameterless points,
shape-owned placement, relative constructions, and ordered internal circle tangency.
Free numeric lengths, angles, and branch magnitudes retain deliberate perturbations to
exercise convergence without authoring point coordinates or placement hints.

## Stress workloads

| File | Stress coverage | Objects | Assertions |
| --- | --- | ---: | ---: |
| [aec3_filter_alignment.locus](stress/aec3_filter_alignment.locus) | Five filter banks, 64 additional coefficient stems, shared delay estimates and range reduction | 241 | 144 |
| [angle_bisector_ratio.locus](stress/angle_bisector_ratio.locus) | Three solved cevians, Ceva identity, four coupled triangle-center networks | 75 | 29 |
| [euler_nine_point.locus](stress/euler_nine_point.locus) | Five medial levels, Euler and nine-point constructions, Feuerbach contact across shrinking scales | 93 | 32 |
| [flowchart_agent_loop.locus](stress/flowchart_agent_loop.locus) | Authoring loop plus 16 solver states, rejection, damping, retry, assertion, and failure branches | 99 | 28 |
| [gantt_chart.locus](stress/gantt_chart.locus) | 20 solved durations, four parallel teams, five phases, 32 dependencies, progress bars, release join | 204 | 117 |
| [inscribed_angle.locus](stress/inscribed_angle.locus) | 16 shared-endpoint chord fans, opposite arcs, supplementary angles, perpendicular chord bisectors | 116 | 51 |
| [kanban_board.locus](stress/kanban_board.locus) | 37 cards, three teams, five states, cross-team blockers, corner containment and WIP | 115 | 61 |
| [pentagon_golden.locus](stress/pentagon_golden.locus) | Five nested pentagrams, diagonal intersections, equal sides and golden-ratio scaling | 79 | 40 |
| [right_triangle_altitude.locus](stress/right_triangle_altitude.locus) | 14 recursive child triangles, altitude identities, incircle contacts and area checks | 131 | 78 |
| [sankey_energy.locus](stress/sankey_energy.locus) | 28 bands, three sources, four intermediate processes, four outputs, split/merge conservation | 203 | 94 |
| [rounded_rect.locus](demonstrations/rounded_rect.locus) | Rounded boxes, capsules, labels, connectors, and curved boundary assertions |
| [sequence_check.locus](stress/sequence_check.locus) | Four participants, 26 messages, cache miss, rejected trial, persistence and artifact acknowledgements | 148 | 45 |
| [software_architecture.locus](stress/software_architecture.locus) | 20 components, four boundaries, 32 shared dependency edges, placed service anchors | 101 | 39 |
| [tangent_circles.locus](stress/tangent_circles.locus) | Noncollinear three-circle solve feeding seven further contacts and common tangents | 83 | 36 |
| [tangent_internal_branches.locus](stress/tangent_internal_branches.locus) | Eight additional nonlinear internal contacts, radial carriers, both exterior tangent branches | 119 | 51 |
| [xy_parabola.locus](stress/xy_parabola.locus) | 129 connected samples, two nonlinear intersections, 32-cell composite area | 318 | 145 |
| [xy_reciprocal.locus](stress/xy_reciprocal.locus) | 130 samples across a discontinuity, nonlinear fixed points, tangent and secant identities | 284 | 273 |

Object counts include geometry, numbers, and annotations. Purely derived figures can
legitimately take zero solver iterations.

## Automated stress contract

[`stress/stress.tsv`](stress/stress.tsv) sets per-file minimum object, assertion, and
equation counts. Relational cases must perform at least one solver iteration where required by the manifest.
The current draft removes translation-pinning equations from each network. The function intersections construct incidence on the linear function and
solve two nonlinear branch magnitudes instead of four point coordinates. Their equation
minimum is therefore two; object and assertion minimums are retained. Both stress test runners verify the manifest against the discovered files, so a
new workload needs an explicit contract and cannot be silently omitted from rendering.

The renderer test starts from source and repeats **parse → solve → assert → measure → layout
→ SVG/TikZ/PNG** twice per case. It compares the complete solved scene, solver and assertion
reports, layout including diagnostics, and output bytes. Successful solves must retain the
specified residual threshold; assertion tolerances are unchanged. No wall-clock performance
threshold is used.

For a longer run with per-case counts and elapsed times:

```sh
LOCUS_STRESS_PASSES=10 \
  cargo test -p locus-render --test spec_examples --all-features -- --nocapture
```

`LOCUS_STRESS_PASSES` accepts 2–100 passes.

For statistically sampled performance measurements of parsing, solving, and the complete
parse-to-SVG pipeline on a representative nonlinear stress workload:

```sh
cargo bench -p locus-render --bench pipeline
```

Criterion writes detailed reports under `target/criterion/`. The benchmark is intentionally
separate from the correctness suite so ordinary tests retain no machine-speed threshold.

An opt-in [competitor benchmark](../../benchmarks/competitors/README.md) measures complete
source-to-SVG CLI latency for Locus, Mermaid, and D2 on equivalent agent-flowchart and
software-architecture workloads.

The [Penrose feature-coverage benchmark](../../benchmarks/penrose/README.md) audits all 53
figures in Penrose's pinned public gallery and distinguishes native coverage, static
approximation, and blocked central capabilities. Its executable inventory and rendering
contract runs with:

```sh
cargo test -p locus-render --test penrose_features
```

## Known solver limitation

[`stress/known_limits/closed_segment_bisectors.locus`](stress/known_limits/closed_segment_bisectors.locus)
preserves a small failure uncovered while expanding the triangle workload: three simultaneous
closed-segment bisectors stall with `E403`. A separate CLI regression test expects that
source-anchored error and verifies that replacing the three segment carriers with their
supporting lines succeeds with the same `between` assertions.

## Representations and previews

- Function curves are piecewise-linear samples. Their analytically solved intersections are
  checked against the actual function equations, and no reciprocal segment crosses `x = 0`.
- Sankey thickness is vertical at the ports. Routing is explicit. Gantt time is numeric days,
  and Kanban WIP is computed from the explicitly listed active cards.
- Dense geometry and crossing connectors may produce crowding or collision diagnostics.
  Repeated runs must return the same layout and diagnostics.
- Plain labels use bundled Noto Sans outlines. TeX segments use `RexEngine` and
  bundled STIX Two Math outlines, matching the CLI's default math renderer.

Export all 16 stress previews into the ignored `target/spec-examples` directory. SVG,
TikZ, and PNG use an opaque white viewport background; other renderer callers retain
transparent output unless they set `RenderOptions.background`. The export also creates
`index.html`, a gallery linking every full-size vector drawing:

```sh
LOCUS_EXAMPLE_OUTPUT_DIR="$PWD/target/spec-examples" \
  cargo test -p locus-render --test spec_examples --all-features
```
