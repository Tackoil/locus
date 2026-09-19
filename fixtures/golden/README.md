# Demonstration goldens

Each `spec/test/demonstrations/*.locus` file has three committed expectations:
`<name>.describe.json`, `<name>.inspect.json`, and `<name>.svg`. The CLI tests
discover all demonstrations in sorted order, including newly added files.
Paths in reports are repository-relative because the CLI runs from the workspace
root; no floats, spans, geometry, or diagnostics are normalized away.

`crates/locus-cli/tests/golden.rs` compares exact bytes. A separate determinism
test starts two independent CLI render processes per fixture, each reading and
parsing the original source, evaluating/solving, measuring text, placing labels,
and rendering SVG. Every render is preceded by a successful `check`.
Both operating systems in CI compare against these same expected files.

The sources currently use format 3 (the repository's v0.4 specification).
Plain fragments use `PlaintextEngine`; TeX uses `RexEngine` through the explicit
`--tex-engine=rex` flag. Both emit bundled font outlines, so the SVG expectations
cover visible mathematical typography as well as geometry and measured layout.

## Updating

From the workspace root, after reviewing the source or implementation change:

```sh
LOCUS_UPDATE_GOLDENS=1 cargo test -p locus-cli --test golden demonstration_goldens
cargo test -p locus-cli --test golden
```

Inspect every changed SVG and JSON diff before committing. Never set the update
environment variable in CI. Tests do not create missing expectations unless
explicitly run in update mode.

## Visual review

All 11 figures pass their in-file assertions. Tangent circles and the angle
bisector construction look consistent with their asserted geometry. Ellipse
foci, spline endpoints/control frame, and the 3-4-5 triangle also look consistent.

The following existing output deserves attention; these goldens record current
behavior rather than approving it as ideal:

- `inscribed_angle`: the highlighted clockwise arc is the major arc through P,
  rather than the opposite minor arc subtended by the inscribed angle. The
  angular equality assertion passes but does not test the selected arc.
- `euler_nine_point`: the opaque highlighted nine-point disk obscures much of the
  triangle and the points it illustrates. This is a style/visibility concern;
  its incidence assertions pass.
- `aec3_filter_alignment`: the Filter 2 range caption overlaps the coefficient
  stems. The layout reports collisions for r1, r2, r3, r4, lk, llg, and lg1/eq.

Additional layout collision warnings are emitted for `angle_bisector_ratio`
(lD, eq), `bspline_controls` (lM), `ellipse_foci` (lO), `euler_nine_point` (lN), `inscribed_angle`
(li, lc), and `pentagon_golden` (lP). Collision results use real ReX measurements.
Render exposes the warnings on stderr or in its JSON report; they do not prevent
writing an artifact. The describe/inspect goldens retain those commands' existing
semantics and do not initiate layout.
