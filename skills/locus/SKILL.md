---
name: locus
description: "Create, inspect, verify, revise, and render geometry figures and technical diagrams using Locus 0 beta 6 source and the Locus CLI. Use for .locus authoring, relational layout, geometric constructions, assertions, routing, and SVG, TikZ, or PNG export."
---

# Locus

Locus records geometric and layout intent in `.locus` source. The engine owns
placement, measured text, automatic dimensions, and routing. This skill targets
**`locus 0 beta 6`**; older headers and authoring syntax are incompatible.

Confirm the available executable with `locus --version` and `locus --help`.
When working in the Locus repository, an existing `target/release/locus` or
`target/debug/locus` can be used; otherwise build/run `cargo run -p locus-cli --`.
Read [language.md](references/language.md) before writing or repairing source.
It is a standalone authoring reference, including beta 5 migration pitfalls.

## Author

- Start with `locus 0 beta 6`, use one simple statement per line, define
  dependencies before use, and end with a newline. `style textbook` is optional.
- Use `kind id(args)` for that kind's constructor: `point A()`, `rect box()`,
  `label caption("Input")`. Use `kind id = rhs` for an alias, computation, or
  differently named constructor: `point M = midpoint(A, B)`,
  `circle inc = incircle(A, B, C)`. A segment is `segment edge(A, B)`.
- Construct when earlier geometry determines the result: anchors, `midpoint`,
  `foot`, `lerp`, and explicit-center shapes introduce no unknowns of their own.
  Use placed points and shapes when independent relationships must be solved.
  Author no absolute point coordinates, pins, center seeds, or `offset()` calls.
- For diagrams, prefer automatic boxes with measured standalone labels,
  `center(caption, box)`, and `contain(box, caption)`. Use `row`, `column`,
  ordered gaps, alignment, and enclosure to state layout. Fixed dimensions
  stay fixed; containment cannot enlarge them. Use numeric distances for
  requested dimensions or data rather than manually calculating diagram positions.
- Reuse local `fn` blocks for repeated components. A named instance exposes
  qualified members; `group name(members...)` collects existing items and
  does not draw, place, or duplicate them.
- Connect shape anchors or points on guides. Use path-attached labels and
  `route: curved` / `route: orthogonal` for automatic routes; add explicit
  `avoid` requirements where clearance matters. Keep route controls internal.
- `rel` and declaration-local `where` define facts. `assert` checks a chosen
  solution and never moves it. Add meaningful independent assertions for
  requested mathematical verification or a reusable contract; do not duplicate
  every construction. Preserve relevant assertions when revising a figure.
- Encode ordering and construction branches in definitions. An unsigned
  distance does not select left/right; `tangent` is external tangency and
  `internally_tangent(outer, inner)` is explicitly internal.

For a complete reusable diagram with measured nodes, groups, path labels,
and an automatic return route, adapt [relational-routing.locus](references/relational-routing.locus).

## Check and repair

Start with focused structured feedback:

```sh
locus check figure.locus --json --compact
```

Checking measures declared text and validates required layout, routes, clipping,
and existing assertions. Add `--layout` for ordinary annotation collision and
margin diagnostics; add `--strict` when a warning-free result is required.
Feasible but incomplete preference settling remains a warning, so a normal
check can pass while a strict check fails. Report this limitation accurately.

Use `describe --json` for ownership, dimensions, collections, invocation chains,
remaining freedoms, branch choices, and accepted routes. `inspect` defaults to
diagnostics; request only the measurements relevant to the issue:

```sh
locus inspect figure.locus --distance A B --json --compact
locus inspect figure.locus --angle A B C --ratio A M B --json --compact
```

Repeat tuple flags as needed. Angles are radians with the middle point as vertex;
ratios are `AM/MB` and require M strictly between A and B. `--measurements` and
`--incidences` request exhaustive output and can be large.

| Exit | Meaning | Next action |
| --- | --- | --- |
| 0 | Check passed | Review material warnings, then render |
| 1 | Assertion failure or strict warnings | Repair definitions/layout using reported actual values |
| 2 | Solve, construction, dependency, layout, routing, or clipping failure | Repair conflicting requirements or invalid geometry |
| 3 | Usage or syntax failure | Correct the command/source at its reported location |

Add missing relations for unresolved relative geometry; common translation
needs no absolute pin. Use a supported construction branch for an unwanted
configuration. Do not delete assertions or weaken tolerances just to pass.
E413 identifies failed layout/routing requirements; reduce conflicting facts or
fix the requested clearance rather than inventing routing parameters. Retain
and report a persistent failing source instead of claiming verification.

Direct source edits are supported. Safe CLI edits validate the whole candidate
before writing: `add` appends stdin, `update --id X --def '(args)'` replaces
constructor arguments, `--set 'dir=NE'` changes a property, and `remove --id X`
checks dependencies. Instance and group updates are supported; edit function
bodies directly when changing a qualified member. `fmt` writes formatted source
to stdout without modifying the file; `fmt --json` emits an AST, not source.

## Render and deliver

Render after `check` exits 0. Rendering solves independently and does not prove
assertions passed. SVG is the default; extensions do not choose a backend:

```sh
locus render figure.locus -o figure.svg
locus render figure.locus --tikz -o figure.tex
locus render figure.locus --png --background white --dpi 300 -o figure.png
```

PNG needs a build with PNG support. Default backgrounds are transparent;
`--dpi` applies only to PNG. Supply `-o`, `--stdout`, or both; render `--json`
requires file output and conflicts with `--stdout`.

Review the rendered preview for requested geometry, visible text, completeness,
flow, and readable spacing. A passing check alone does not establish visual
quality. Plain/CJK text uses bundled fonts; TeX uses the bundled ReX engine and
outlines. For E414, simplify unsupported TeX to supported notation or plain text.

Deliver the `.locus` source and requested exports with the check result and any
remaining material diagnostics. Honor source-only requests; if the CLI cannot
run, clearly mark the source as unverified.
