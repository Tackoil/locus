# Locus demonstrations and stress tests

The positive `.locus` corpus is split by purpose:

- [`demonstrations/`](demonstrations/) contains eleven compact, readable examples of the
  revised [v0.4 draft](../../documents/specification.md): points and shapes own placement;
  anchors and constructions remain derived. These are the migration targets for the CLI `describe` and `inspect`
  snapshots; the current implementation cannot run their new declarations yet.
- [`stress/`](stress/) contains 17 expanded workloads. These exercise large construction
  graphs, dense annotations, shared constraints, assertions, deterministic layout, and every
  rendering backend.

Invalid-input fixtures remain in [`fixtures/errors/`](../../fixtures/errors/) so they are not
mistaken for runnable examples.

Run the checks from the repository root. The demonstration check currently fails on the
unsupported `locus 3` header and new declarations; it becomes the acceptance
check for implementation migration. The stress corpus still uses v0.3 (`locus 2`).

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
| [sequence_check.locus](demonstrations/sequence_check.locus) | Sequence-diagram participants and messages |
| [tangent_circles.locus](demonstrations/tangent_circles.locus) | Solved tangent circles and common tangents |

### Draft migration findings

All eleven demonstrations omit hinted free-point declarations, coordinate hints, and
absolute placement. Parameterless points, circles, ellipses, rectangles, and diamonds
own placement; relations act directly on points and anchors.
Unique intersections and tangent contact points are derived constructions. Relative
offsets remain useful for message rows, coefficient stems, and the filter time axis.

The angle-bisector and Euler examples choose vertices by angles on a placed circumcircle.
These constructions remain valid, but a circle is no longer necessary to introduce
independent vertices: `point_relations.locus` uses three `point()` declarations and
relative relationships. Each point constructor takes zero arguments. Points derived
from anchors or constructions remain useful when their geometry is already determined.

Geometry validation used temporary copies adapted to the existing CLI: each placed shape
was expanded to a circle/box with a test-only free center, and point-anchor declarations
were rewritten as equivalent existing constructors. The defining relations and all
assertions were preserved. All **45 assertions across the original eight files passed**; the four
relation-driven diagrams converged from perturbed test placements with maximum normalized
residual below `1e-12`. The pentagon check exposed and corrected an existing reversed ratio:
for `P = intersect(seg(A, C), seg(B, E))`, `PC / AP`, not `AP / PC`, equals the golden ratio.

This validates the constructions and relative geometry, **not native support for the new
syntax, automatic initialization, placement DOF reporting, or rendering**. Direct checks
reject the `locus 3` header. Earlier checks using format-2 headers also exposed
`E125` for point-anchor declarations and `E404` for center-free shape constructors. The parser corpus and demonstration snapshot tests remain enabled and will
fail until the implementation is migrated; their snapshots have not been regenerated from
errors or from the temporary adapted copies. The stress workloads are unchanged.

The ellipse and B-spline demonstrations add 13 draft assertions. They cannot be adapted
to the existing geometry engine, which has neither curve kind; they are excluded from
the 45-assertion result above. Their analytic identities and spline parameter conventions
are checked independently, not through the CLI. The initial B-spline form is cubic,
open, non-rational, and clamped, with automatic uniform interior knots. Control points
come from existing anchors. Custom knots/degrees, fitting, general curve intersections,
and tangency remain outside this draft's curve support.

The parameterless-point demonstration adds five assertions. Its relative geometry is
checked using a temporary copy with test-only point seeds for the current CLI. This
does not validate automatic initialization or native `point()` support; the new
constructor still requires parser and solver migration.

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
| [pentagon_golden.locus](stress/pentagon_golden.locus) | Five nested pentagrams, diagonal intersections, equal sides and golden-ratio scaling | 79 | 39 |
| [right_triangle_altitude.locus](stress/right_triangle_altitude.locus) | 14 recursive child triangles, altitude identities, incircle contacts and area checks | 129 | 78 |
| [sankey_energy.locus](stress/sankey_energy.locus) | 28 bands, three sources, four intermediate processes, four outputs, split/merge conservation | 203 | 94 |
| [sequence_check.locus](stress/sequence_check.locus) | Four participants, 26 messages, cache miss, rejected trial, persistence and artifact acknowledgements | 148 | 45 |
| [software_architecture.locus](stress/software_architecture.locus) | 20 components, four boundaries, 32 shared dependency edges, perturbed service anchors | 101 | 39 |
| [tangent_circles.locus](stress/tangent_circles.locus) | Noncollinear three-circle solve feeding seven further contacts and common tangents | 83 | 36 |
| [tangent_internal_branches.locus](stress/tangent_internal_branches.locus) | Eight additional nonlinear internal contacts, radial carriers, both exterior tangent branches | 103 | 51 |
| [xy_parabola.locus](stress/xy_parabola.locus) | 129 connected samples, two nonlinear intersections, 32-cell composite area | 316 | 145 |
| [xy_reciprocal.locus](stress/xy_reciprocal.locus) | 130 samples across a discontinuity, nonlinear fixed points, tangent and secant identities | 282 | 273 |

Object counts include geometry, numbers, and annotations. Purely derived figures can
legitimately take zero solver iterations.

## Automated stress contract

[`stress/stress.tsv`](stress/stress.tsv) sets per-file minimum object, assertion, and
equation counts. Cases with deliberately perturbed hints must perform at least one solver
iteration. Both stress test runners verify the manifest against the discovered files, so a
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
- Plain labels use actual bundled font outlines. TeX segments use `MockEngine` boxes because
  the real TeX backend is unavailable in these tests.

Export all 17 stress previews into the ignored `target/spec-examples` directory:

```sh
LOCUS_EXAMPLE_OUTPUT_DIR="$PWD/target/spec-examples" \
  cargo test -p locus-render --test spec_examples --all-features
```
