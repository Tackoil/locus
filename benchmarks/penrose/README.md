# Penrose feature-coverage benchmark

This benchmark asks whether **Locus 0 beta 5 can express the defining feature of every
figure in Penrose's public examples gallery**. It is a capability benchmark, not a speed
test or a pixel-similarity contest.

The corpus is the 53 entries marked `"gallery": true` in Penrose's official
[`registry.json`](https://github.com/penrose/penrose/blob/main/packages/examples/src/registry.json),
checked at commit [`164ad557`](https://github.com/penrose/penrose/commit/164ad55777e9975baa0e0860a5d18051f66beafa)
on 2026-09-20. [`gallery.tsv`](gallery.tsv) records every entry, its result, a local Locus
capability witness where one exists, and the first material gap.

## Result

| Outcome | Figures | Meaning |
| --- | ---: | --- |
| `native` | 8 | Locus has the geometry, annotations, and relations needed for the defining static figure. |
| `approximation` | 28 | A recognizable static version is possible, but Penrose's generator, reusable domain rules, layout, computation, styling, or interaction is missing. |
| `blocked` | 17 | A central visual or semantic capability cannot be represented faithfully by the current object model. |

Therefore, **the current implementation cannot draw all of Penrose's main gallery figures at
feature parity**. It is strongest on Euclidean constructions, 2D labelled geometry, and
explicit box-and-arrow diagrams. The largest gaps are:

- 3D projection, surfaces, lighting, shadows, gradients, blur, and imported SVG/image shapes;
- arbitrary or closed paths, Catmull-Rom/fitted curves, and shape-specific gate glyphs;
- collections, loops, user functions, reusable domain/style rules, and automatic graph layout;
- data transforms and generators such as regression, random walks, fractals, envelopes, and
  generic ray/closest-point queries;
- animation and interaction.

The status is deliberately stricter than "a patient author could hand-place enough segments to
look similar." Manual sampling or enumeration earns `approximation`, not `native`, when the
Penrose example's defining feature is the missing generator or computation. Purely decorative
differences do not block a native result.

## Verification

Run the executable contract with:

```sh
cargo test -p locus-render --test penrose_features
```

The test verifies the 53-row inventory and outcome totals, rejects duplicate Penrose IDs,
parses every cited Locus witness, and runs each `native` witness through solve, assertions,
text measurement, layout, and SVG rendering. Witnesses demonstrate the required Locus feature;
they are not claimed to be pixel-identical copies of Penrose's artwork.

When Penrose changes its gallery, refresh the `gallery: true` inventory from the upstream
registry, reassess each changed program's Domain/Substance/Style sources, update the pinned
commit above, and adjust the expected totals in `penrose_features.rs`.
