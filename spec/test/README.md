# Executable stress examples

All 17 top-level `.locus` files are interconnected stress workloads. They contain
75–316 objects each, with deep construction dependencies, dense annotations,
shared constraints, and independent geometric or diagram assertions.

Run the correctness suite from the repository root:

```sh
cargo run -p locus-cli -- check spec/test/*.locus
cargo test -p locus-cli -p locus-render --test spec_examples --all-features
```

## Workloads

| File | Stress coverage | Objects | Assertions |
| --- | --- | ---: | ---: |
| [aec3_filter_alignment.locus](aec3_filter_alignment.locus) | Five filter banks, 64 additional coefficient stems, shared delay estimates and range reduction | 241 | 144 |
| [angle_bisector_ratio.locus](angle_bisector_ratio.locus) | Three solved cevians, Ceva identity, four coupled triangle-center networks | 75 | 29 |
| [euler_nine_point.locus](euler_nine_point.locus) | Five medial levels, Euler and nine-point constructions, Feuerbach contact across shrinking scales | 93 | 32 |
| [flowchart_agent_loop.locus](flowchart_agent_loop.locus) | Authoring loop plus 16 solver states, rejection, damping, retry, assertion, and failure branches | 99 | 28 |
| [gantt_chart.locus](gantt_chart.locus) | 20 solved durations, four parallel teams, five phases, 32 dependencies, progress bars, release join | 204 | 117 |
| [inscribed_angle.locus](inscribed_angle.locus) | 16 shared-endpoint chord fans, opposite arcs, supplementary angles, perpendicular chord bisectors | 116 | 51 |
| [kanban_board.locus](kanban_board.locus) | 37 cards, three teams, five states, cross-team blockers, corner containment and WIP | 115 | 61 |
| [pentagon_golden.locus](pentagon_golden.locus) | Five nested pentagrams, diagonal intersections, equal sides and golden-ratio scaling | 79 | 39 |
| [right_triangle_altitude.locus](right_triangle_altitude.locus) | 14 recursive child triangles, altitude identities, incircle contacts and area checks | 129 | 78 |
| [sankey_energy.locus](sankey_energy.locus) | 28 bands, three sources, four intermediate processes, four outputs, split/merge conservation | 203 | 94 |
| [sequence_check.locus](sequence_check.locus) | Four participants, 26 messages, cache miss, rejected trial, persistence and artifact acknowledgements | 148 | 45 |
| [software_architecture.locus](software_architecture.locus) | 20 components, four boundaries, 32 shared dependency edges, perturbed service anchors | 101 | 39 |
| [tangent_circles.locus](tangent_circles.locus) | Noncollinear three-circle solve feeding seven further contacts and common tangents | 83 | 36 |
| [tangent_internal_branches.locus](tangent_internal_branches.locus) | Eight additional nonlinear internal contacts, radial carriers, both exterior tangent branches | 103 | 51 |
| [xy_parabola.locus](xy_parabola.locus) | 129 connected samples, two nonlinear intersections, 32-cell composite area | 316 | 145 |
| [xy_reciprocal.locus](xy_reciprocal.locus) | 130 samples across a discontinuity, nonlinear fixed points, tangent and secant identities | 282 | 273 |

Object counts include geometry, numbers, and annotations. The workload is a mix of
solver stress, derived-construction stress, and layout/rendering stress; purely
derived figures legitimately take zero solver iterations.

## Automated stress contract

[stress.tsv](stress.tsv) sets per-file minimum object, assertion, and equation
counts. Cases with deliberately perturbed hints must perform at least one solver
iteration. Both test runners verify the manifest against the discovered top-level
files, so a new example needs an explicit workload contract and cannot be silently
omitted from rendering.

The renderer test starts from source and repeats **parse → solve → assert → measure
→ layout → SVG/TikZ/PNG** twice per case. It compares the complete solved scene,
solver and assertion reports, layout including diagnostics, and output bytes.
Successful solves must retain the specified residual threshold; assertion
tolerances are unchanged. No wall-clock performance threshold is used.

For a longer run with per-case counts and elapsed times:

```sh
LOCUS_STRESS_PASSES=10 \
  cargo test -p locus-render --test spec_examples --all-features -- --nocapture
```

`LOCUS_STRESS_PASSES` accepts 2–100 passes. This increases sustained workload while
retaining the same deterministic scene and correctness checks.

## Known solver limitation

[known_limits/closed_segment_bisectors.locus](known_limits/closed_segment_bisectors.locus)
preserves a small failure uncovered while expanding the triangle workload: three
simultaneous closed-segment bisectors stall with `E403`. A separate CLI regression
test expects that source-anchored error and verifies that replacing the three
segment carriers with their supporting lines succeeds with the same `between`
assertions. A future solver fix should update this expected-failure test.

The passing angle-bisector stress scene uses those supporting lines. Tangency
scenes use direct `lerp`, `tangent_line`, and `foot` contact constructions where
distance-only constraints provide insufficient position or angle accuracy.

## Representations and previews

- Function curves are piecewise-linear samples. Their analytically solved
  intersections are checked against the actual function equations, and no
  reciprocal segment crosses `x = 0`.
- Sankey thickness is vertical at the ports. Routing is explicit. Gantt time is
  numeric days, and Kanban WIP is computed from the explicitly listed active cards.
- Dense geometry and crossing connectors may produce crowding or collision
  diagnostics. Repeated runs must return the same layout and diagnostics.
- Plain labels use actual bundled font outlines. **TeX segments in the original
  examples use `MockEngine` boxes**, because the real TeX backend is unavailable.
  These tests cover their layout/rendering integration, not actual TeX typesetting.

Export all 17 previews into the ignored `target/spec-examples` directory:

```sh
LOCUS_EXAMPLE_OUTPUT_DIR="$PWD/target/spec-examples" \
  cargo test -p locus-render --test spec_examples --all-features
```

The outputs are `.svg`, `.tex`, and `.png`. This uses the renderer library directly
because the CLI's `render` command is not yet connected to the backends.
