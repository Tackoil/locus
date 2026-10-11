---
name: locus
description: "Create, inspect, verify, revise, and render Locus geometry figures and technical diagrams using .locus source files and the Locus CLI. Use for Locus authoring and diagram troubleshooting, including relational placement, geometric constructions, assertions, and SVG, TikZ, or PNG export."
---

# Locus

Locus is a deterministic relational language for plane geometry and technical
diagrams. The `.locus` file records geometric intent; Locus owns placement.
Construct the figure, check syntax and construction, then render and review
the result for correctness, completeness, and readable layout.

## Find the tool and language contract

Use `locus --help` to confirm an installed CLI and available export formats.

This skill targets `locus 0 beta 5`.

Read [the language reference](references/language.md) when writing or repairing
source. It covers common constructors, anchors, annotations, and pitfalls.
Use CLI diagnostics to verify syntax rather than inventing APIs.

## Author the figure

- Begin with `locus 0 beta 5`. Use one statement per line and a final newline.
  `style textbook` is optional. Define dependencies before use and give every
  declaration a unique identifier.
- Construct when you can, relate when you must. Use anchors, `offset`, `foot`,
  `midpoint`, and other derived constructions for geometry determined by earlier
  objects. Each `point()` adds two solver unknowns; wrapping a known construction
  in `point() where { coincident(...) }` adds unnecessary solve work. Use
  parameterless points and dimension-only shapes for independent placement that
  must be solved from relationships. For determined centers, use explicit-center
  shapes such as `rect(offset(input.center, 5, 0), 3, 1)`.
- Express placement with geometric anchors, signed relative offsets, distances,
  alignment, and tangency. Never supply absolute point coordinates, center
  seeds, pins, or a `location` field.
- Use `rel` or declaration-local `where` for defining equations. Assertions are
  optional for drawing tasks. Add `assert` when the user requests numerical
  verification or a reusable geometry contract, or for an important independent
  invariant that construction alone does not guarantee. Avoid repeating every
  derived offset or construction as an assertion. Assertions verify the chosen
  solution; they do not move geometry or choose another branch.
- Select ambiguous constructions explicitly. `tangent(c1, c2)` means external
  tangency; internal tangency is `internally_tangent(outer, inner)`. Use signed
  offsets when left/right or above/below order matters.
- Keep geometry independent of annotation layout. Use shape anchors to route
  connectors; label-box anchors are annotation-only.

For diagram rows, lifelines, and routing points, read
[derived placement](references/language.md#prefer-derived-placement) and adapt
the [runnable routing example](references/derived-routing.locus). It keeps one
placement owner and derives the rest, without absolute coordinates or pins.

For an existing figure, preserve its identifiers and assertions while changing
the definitions needed for the request. Direct source editing is supported.
The CLI also supports appending statements with `locus add figure.locus`
from stdin, replacing a RHS with `locus update figure.locus --id X --def 'RHS'`,
changing a property with `--set 'dir=NE'`, and removing a declaration with
`locus remove figure.locus --id X` (dependents must be handled first).

## Verify and repair

Start with the saved source, using `--json` for agent feedback:

```sh
locus check figure.locus --json --compact
```

This checks syntax, construction, and any assertions already in the source;
new assertions are not required. When diagnosing a specific issue, use
`locus describe figure.locus --json` for placement ownership, unresolved freedoms,
and branch choices. Compact check feedback counts assertions and retains failed
results with their actual values; omit `--compact` for full solver and assertion
details. Reports describe relationships rather than absolute coordinates.

`inspect` returns diagnostics by default. Request only the measurements needed
for the issue, including hidden helper points:

```sh
locus inspect figure.locus --distance A B --json --compact
locus inspect figure.locus --angle A B C --ratio A M B --json --compact
```

Repeat query flags for additional tuples. Angles return radians with the middle
point as vertex; ratios require the middle point strictly between the endpoints.
Use `--measurements` only when an exhaustive distance/angle/ratio table is needed,
and `--incidences` for all detected facts; both can produce large output.
Invisible `mark: none` helpers do not cause visual crowding/clipping warnings.

| Exit | Meaning | Response |
| --- | --- | --- |
| 0 | Check passed | Review relevant warnings, then render |
| 1 | Assertion failure or strict-check warnings | Use reported actual values to fix definitions or layout |
| 2 | Solve, construction, or dependency failure | Repair conflicting facts, degeneracy, or dependencies |
| 3 | Usage or syntax failure | Correct the command or source at the reported location |

Add missing defining relations for unresolved relative geometry; harmless rigid
movement of the whole figure does not need an absolute pin. Fix an unwanted
branch by changing its explicit construction or defining relations. Do not
delete assertions or weaken tolerances merely to obtain a passing check.
If a solver limitation persists, retain the failing source and report the
diagnostic and remaining unmet requirement instead of claiming verification.

`check --strict` treats inspection warnings as failures; use it when the task
requires a warning-free figure. `locus fmt figure.locus` emits formatted source
without editing the file; capture it separately before replacing the source.
`fmt --json` exports the parsed AST, not an authoring format.

## Render and deliver

Render only after `check` exits 0. Rendering independently parses and solves;
a successful export does not prove that assertions passed.

```sh
locus render figure.locus -o figure.svg
locus render figure.locus --tikz -o figure.tex
locus render figure.locus --png --background white --dpi 300 -o figure.png
```

Review the rendered preview against the requested geometry or diagram flow,
visible text, completeness, and layout. A passing check does not establish that
the image meets the request.

SVG is the default. Extensions do not select a backend: request `--tikz` or
`--png` explicitly. Use `--background white` for opaque output; the default is
transparent. `--dpi` requires PNG. Supply `-o`, `--stdout`, or both;
`--json` requires file output and conflicts with `--stdout`.

Plain text and CJK labels use bundled fonts. TeX fragments use the default ReX
engine and bundled math outlines; no external TeX process is needed. Unsupported
TeX produces E414: simplify to supported notation or plain text.

Deliver the `.locus` source and requested exports, with the check result and any
remaining material diagnostics. If execution is unavailable, state that the
source is unverified. Honor a request for source-only output.
