# Locus Specification v0.2 (Draft)

*A constructive language and CLI toolchain for plane-geometry figures with precise alignment and ratios.*

---

## 0. Design principles (informative)

1. **The agent declares relationships; Locus computes coordinates.** No raw coordinates exist outside `free(...)`.
2. **The `.locus` file is the figure's source code** — ordered, diffable, hand-editable, self-testing (asserts live in the file).
3. **Text-based verification is the primary feedback channel.** `describe` / `inspect` / `check` must be complete enough that a vision-less agent can catch every mistake.
4. **Determinism is normative.** Identical file → identical resolved geometry → byte-identical SVG.
5. **Statement-per-line syntax with compiler-grade diagnostics.** Local errors stay local; error messages name source lines and suggest fixes.

---

## Part 1 — The Locus language

### 1.1 File identity

- Extension: `*.locus`; encoding UTF-8; newline-terminated statements.
- Comments: `//` to end of line.
- The file stores the **construction program**, never resolved coordinates. Resolved geometry is always derived by evaluation.
- Machine interchange: `locus fmt --json` emits the parsed AST as JSON (§7.4). The DSL is the authoring format; JSON is derived, never authored.

### 1.2 File shape

```locus
locus 1                      // format version header, mandatory, first statement

meta {
  title "Incircle with radius mark"
  note  "optional free text"
}

style textbook               // optional; defaults to textbook

// ---- construction ----
point A = free(0, 0)
circle inc = incircle(A, B, C)   [highlight]

// ---- annotation ----
label lr = label(r.midpoint, $r$)  { dir: NE }

// ---- intent ----
assert tangent(inc, seg(B, C))
```

### 1.3 Grammar (EBNF)

```
file        := header statement*
header      := "locus" integer NEWLINE
statement   := op | assert | meta | style | COMMENT | NEWLINE

op          := kind ident "=" constructor [attr] [props] NEWLINE
kind        := "point" | "line" | "ray" | "segment" | "circle" | "arc"
             | "polygon" | "label" | "arrow" | "tick" | "right_angle"
             | "text_block"
constructor := ident "(" [args] ")"
attr        := "[" ident "]"                       -- semantic style
props       := "{" [prop ("," prop)*] "}"
prop        := ident ":" propvalue
propvalue   := expr | string | ident               -- ident for enums: NE, auto, ->

assert      := "assert" boolexpr ["tol" number] NEWLINE
meta        := "meta" "{" (ident string)* "}"
style       := "style" ident NEWLINE

args        := expr ("," expr)*
expr        := term (("+" | "-") term)*
term        := unary (("*" | "/") unary)*
unary       := ["-"] atom
atom        := number [unit] | boolean | string | tex_literal
             | funcall | property | ref | "(" expr ")"
funcall     := ident "(" [args] ")"
property    := ref "." ident ["(" args ")"]
ref         := ident
unit        := "deg" | "rad"
boolean     := "true" | "false"

boolexpr    := cmp | predicate
             | boolexpr ("and" | "or") boolexpr | "not" boolexpr
             | "(" boolexpr ")"
cmp         := expr ("==" | "<" | ">" | "<=" | ">=") expr
predicate   := ident "(" args ")"

ident       := [A-Za-z][A-Za-z0-9_']*
string      := '"' … '"'                           -- standard escapes; TeX as-is
tex_literal := "$" … "$"                         -- TeX backslashes preserved
COMMENT     := "//" …end of line
```

### 1.4 Statement rules

- **One statement per line.** No semicolons, no line continuations in v0.2. Parse errors are reported and recovered per line, so one bad statement never invalidates the file's remaining diagnostics.
- **`kind id = constructor`** is deliberately redundant: the declared kind must match the constructor's result type. This cross-check catches agent mistakes early (`point M = seg(A,B)` → immediate type error) and makes files skimmable.
- **Identifiers**: unique per file, `[A-Za-z][A-Za-z0-9_']*`. Convention (not enforced): single/primed letters for points (`A`, `B'`, `M_1`), words for derived objects (`tri`, `inc`).
- **Definition order**: a constructor may only reference ids defined on earlier lines. `locus fmt` verifies dependency order and reports cycles/forward references.
- **`[attr]`** — semantic style, closed enum: `main` (default) | `auxiliary` | `highlight` | `construction`. No raw colors, widths, or dash arrays in v0.2.
- **`{props}`** — op-specific fields (closed set per kind, §3.6). Unknown props are errors.
- **Strings**: double quotes with escapes `\"`, `\\`, and `\n`; strings always represent plain text.
- **TeX literals**: bare `$...$` values are distinct from strings and preserve backslashes verbatim: `$\alpha$`, `$\triangle ABC$`. Raw newlines and nested dollar delimiters are invalid.

### 1.5 Asserts

```locus
assert tangent(inc, seg(B, C))
assert ratio(dist(B, D), dist(D, C)) == dist(A, B) / dist(A, C)   tol 1e-6
```

- Asserts are persistent — they travel with the file and are re-verified by `locus check` (the figure carries its own tests).
- `tol` is optional; default tolerance is **relative**: `1e-9 ×` figure diameter (§5.2).
- On failure, the report includes the actual value of **every measurable subterm**, e.g. both distances and the resulting ratio, so the agent can localize which upstream definition is wrong.

---

## Part 2 — Coordinate and angle conventions

- Cartesian scene space, **y-up**. The renderer flips to screen space; scene coordinates never denote pixels. Viewport is auto-fitted (§6.3).
- Angles are CCW-positive. The unit suffix is **mandatory** on angle literals: `60deg`, `1.2rad`. A bare number in an angle position is a parse error (this kills the degrees/radians hallucination class).
- Multi-valued constructions (intersections, tangents) require an explicit **branch index** `k ∈ {0, 1}` with a deterministic ordering rule per constructor (§3.1). Omitting `k` when two branches exist is an error, not a default.

---

## Part 3 — Object kinds and constructors

### 3.1 `point`

```
free(x, y)                      -- x, y: expressions; the only raw-coordinate entry point
midpoint(A, B)
lerp(A, B, t)                   -- t=0 → A, t=1 → B; t may lie outside [0,1]
intersect(o1, o2, k)            -- k: branch index; error if intersection is empty
foot(P, l)                      -- perpendicular foot of P on line/ray/segment l
project(P, c)                   -- nearest point on circle c to P
rotate(P, O, theta)
reflect(P, axis)                -- axis: line;  reflect(P, O): point reflection
translate(P, from, to)          -- P + (to − from)
on_circle(c, theta)             -- point on c at CCW angle theta from +x
extend(A, B, d)                 -- on ray A→B, at distance dist(A,B) + d from A
centroid(A, B, C, ...)
circumcenter(A, B, C)
incenter(A, B, C)
orthocenter(A, B, C)
center(c)                       -- center of circle/arc
```

**Branch ordering rule for `intersect(o1, o2, k)`:** branches are sorted by the parameter of `o1` — along its direction for a line/ray/segment, by CCW angle from +x for a circle. The rule is deterministic and documented per type pair. `locus describe` always prints which branch resolved to which coordinates, so an agent that picked the wrong side flips `k` and re-checks.

### 3.2 `line`, `ray`, `segment`

```
line(A, B)          ray(A, B)          seg(A, B)
perpendicular(P, l)                 -- line through P ⊥ l
parallel(P, l)                      -- line through P ∥ l
perp_bisector(A, B)
angle_bisector(A, B, C)             -- interior bisector at vertex B
tangent_line(P, c, k)               -- P strictly outside c; k = 0 | 1
```

### 3.3 `circle`, `arc`

```
circle(O, r)                        -- r: expression, e.g. dist(A, B) / 2
circle_through(O, A)
circle_diameter(A, B)
circumcircle(A, B, C)
incircle(A, B, C)
arc(c, P, Q, ccw)                   -- ccw: true|false; P, Q must lie on c (checked)
angle_arc(A, B, C, r)               -- angle-mark arc at vertex B, radius r
```

### 3.4 `polygon`

```
polygon(A, B, C, ...)               -- ≥ 3 vertices, listed in order
regular(O, n, A)                    -- regular n-gon, center O, one vertex at A
```

A polygon implies its edges for incidence detection, label collision, and rendering, but does **not** auto-create edge ids. An agent needing a specific edge writes `seg(A, B)` explicitly (index-based edge access is deliberately absent — agents are unreliable with indices).

### 3.5 Annotations

```
label(anchor, text)
arrow(from_anchor, to_anchor)
tick(s, n)                          -- n equal-length tick marks on segment s, n ∈ 1..4
right_angle(V, P, Q)                -- square mark at V between rays V→P and V→Q
text_block(text)
```

The `text` argument accepts a quoted plain-text string or a bare `$...$` TeX literal.

### 3.6 Props per kind

| Kind | Props | Default |
|---|---|---|
| `label` | `dir: auto \| N \| NE \| E \| SE \| S \| SW \| W \| NW`; `gap: expr` | `auto`; style-derived gap |
| `arrow` | `head: -> \| <-> \| -`; `bend: angle` | `->`; `0deg` |
| `right_angle` | `size: expr` | style-derived |
| `text_block` | `place: below \| above \| left \| right` (relative to figure bbox) or `anchor: <anchor>, dir: <dir>` | `below` |
| `point` | `mark: dot \| cross \| none` | `dot` |

### 3.7 Anchors

Anywhere an annotation accepts a point, an **anchor expression** is accepted:

```
A                       -- any point id
s.midpoint  s.start  s.end
c.center    c.at(30deg)
poly.centroid
lbl.center              -- resolved label box center (annotation layer only)
```

Anchors are read-only derived points. They may **not** appear in geometric constructors (`intersect`, `foot`, …) — construction must use real point ops. Rationale: annotation anchors resolve *after* label layout (§6), so annotations may reference label boxes, but geometry must never depend on layout.

---

## Part 4 — Expression language

### 4.1 Numeric functions

| Function | Result |
|---|---|
| `dist(P, Q)` | Euclidean distance |
| `angle(A, B, C)` | unsigned interior angle at `B`, in `(0deg, 180deg)` |
| `dir_angle(A, B)` | CCW angle of vector A→B from +x, in `[0deg, 360deg)` |
| `ratio(a, b)` | `a / b` (sugar — agents state ratios constantly) |
| `radius(c)` `length(s)` `area(poly)` `perimeter(poly)` | as named |
| `sqrt(x)` `abs(x)` `min(a, b)` `max(a, b)` | as named |

### 4.2 Predicates (boolean contexts / asserts only)

| Predicate | Semantics (within tolerance) |
|---|---|
| `on(P, obj)` | P lies on line/ray/segment/circle/arc |
| `collinear(A, B, C, ...)` | all points on one line |
| `concyclic(A, B, C, D, ...)` | all points on one circle |
| `parallel(l1, l2)` / `perpendicular(l1, l2)` | as named |
| `tangent(c, l)` / `tangent(c1, c2)` | tangency |
| `between(A, B, C)` | B strictly between A and C on their common line |
| `inside(P, poly)` / `inside(P, c)` | strict containment |
| `congruent(s1, s2)` | equal lengths |

Angle/length equality uses `==` on numeric functions rather than dedicated predicates: `angle(A, B, C) == 90deg`.

---

## Part 5 — Evaluation semantics

### 5.1 Evaluation model

- Ops evaluate strictly top-to-bottom; each op resolves to concrete geometry using previously resolved objects. Arithmetic is IEEE double.
- Editing any op invalidates and recomputes all dependents automatically. Removing an op that others depend on is an error naming every orphaned dependent.

### 5.2 Tolerance

- Default tolerance for all predicates and `==` comparisons: **relative**, `1e-9 ×` the figure's bounding-box diameter. Per-assert override via `tol`.
- Rationale: agents pick arbitrary scene units; an absolute tolerance would silently change meaning with scale.

### 5.3 Errors

Construction failures — empty `intersect`, `circumcircle` of collinear points, `tangent_line` from an interior point, division by zero — abort evaluation with **exit code 2** and a source-anchored diagnostic:

```
figure.locus:9:11  error[E201]: intersect(c1, c2, 0): circles do not intersect
                   c1 radius 1.20, c2 radius 0.80, center distance 3.41 > 2.00
   9 | point P = intersect(c1, c2, 0)
```

Diagnostics must name the op id, the failing precondition, and the offending measured values. No NaN or infinity ever leaks into a resolved scene.

### 5.4 Determinism (normative)

Identical file bytes → identical resolved coordinates → byte-identical SVG output. All tie-breaks (branch ordering, label candidate selection, formatting) are defined without randomness. This enables golden-file snapshot testing in CI.

---

## Part 6 — Annotation layout and rendering

### 6.1 Label placement

For each label with `dir: auto`:

1. **Candidates**: 8 compass directions × 2 gap radii around the anchor, plus domain-informed preferences — vertex labels prefer the external angle bisector direction (away from polygon interior); segment labels prefer the two perpendicular offsets; circle labels prefer outward along the radius.
2. **Scoring**: penalties for overlapping strokes, other label boxes, or exceeding figure margin.
3. **Assignment**: greedy, most-constrained-first, deterministic tie-break (candidate list order). Any label left with a positive collision score is reported as a diagnostic — never silently accepted.

Pinned `dir` bypasses the heuristic entirely.

### 6.2 TeX

Bare `$...$` TeX literals render server-side (KaTeX subset in v0.2) to SVG; the **measured** bounding box feeds label layout. Extents are never estimated.

### 6.3 Viewport & style

- Auto-fit: bounding box of all geometry + resolved label boxes, plus margin; y-flip to screen space. No pixel-space controls exist in the language.
- One built-in preset (`textbook`): thin black strokes, small filled point disks, dashed rendering for `[auxiliary]`/`[construction]`, accent color for `[highlight]`, TeX sized relative to figure scale.

---

## Part 7 — CLI

### 7.1 Commands

| Command | Purpose |
|---|---|
| `locus init <file> [--style textbook]` | create a new scene file |
| `locus add <file>` | append statements from stdin; whole file re-validated |
| `locus update <file> --id X --def "<constructor>"` / `--set 'dir=NE'` | surgical edit; dependents recompute |
| `locus remove <file> --id X` | remove; orphaned dependents → exit 2 |
| `locus describe <file>` | structural dump: objects, defs, resolved positions, dependency order, branch resolutions |
| `locus inspect <file>` | measurements, detected incidences, diagnostics |
| `locus check <file>...` | diagnostics + all in-file asserts; exit 0 iff clean — the CI command |
| `locus fmt <file> [--json]` | canonical formatting / AST-as-JSON export |
| `locus render <file> -o out.svg [--png] [--tikz] [--stdout]` | render (intended for humans, not agent feedback) |

Since the format is line-oriented, agents may equally edit the file directly; `check` validates everything regardless of how edits happened. CLI ops are the "safe API," direct editing the "expert mode."

### 7.2 Exit codes

| Code | Meaning |
|---|---|
| `0` | success / all asserts pass |
| `1` | assert failed (actuals in the JSON report) |
| `2` | construction or dependency error |
| `3` | usage / parse error |

### 7.3 Output conventions

- All commands support `--json` for machine-readable output (agents); default output is human-readable with source-line anchors. Structured errors go to stdout under `--json`; stderr mirrors them for humans.
- `inspect --json` reports: measurement tables (pairwise distances among named points, triangle angles, ratios along shared lines), detected incidences ("`inc` tangent to `seg(A,B)`", "`M` on `AB` with `AM:MB = 1:1`"), and diagnostics (`degenerate`, `label_collision`, `out_of_view`, `crowding`, `unreferenced`).

### 7.4 JSON interchange

`locus fmt --json` emits the AST: version, meta, ordered ops (`{id, kind, def, attr, props}` with `def` as both source string and parsed tree), and asserts. This is the integration surface for future MCP shims, editors, or watch-mode preview servers — all of which wrap the CLI, never reimplement it.

### 7.5 Intended agent loop

```bash
locus init incircle.locus
locus add incircle.locus < construction.locus
locus inspect incircle.locus --json     # → diagnostics: label_collision on lr
locus update incircle.locus --id lr --set 'dir=NE'
locus check incircle.locus              # → exit 0
locus render incircle.locus -o incircle.svg
```

Recommended agent policy: **never `render` until `check` exits 0.**

---

## Part 8 — Out of scope for v0.2 (reserved)

- Raw coordinates outside `free(...)` — every non-free position must be constructed. This is the coordinate-hallucination firewall; do not weaken it.
- Numeric constraint solving (`solve` blocks) — possible v2 direction.
- Custom styles beyond the four semantic classes; whole-figure transforms; layers; animation; 3D.
- Multi-line statements, includes/imports, user-defined functions.

The `locus <n>` header exists so these can arrive without breaking v1 files.

---

## Part 9 — Complete example

```locus
locus 1

meta {
  title "Incircle with radius mark"
}

style textbook

// ---- construction ----------------------------------------
point A = free(0, 0)
point B = free(6, 0)
point C = free(1.8, 4.2)

polygon tri = polygon(A, B, C)
circle  inc = incircle(A, B, C)   [highlight]
point   I   = center(inc)
point   T   = foot(I, seg(B, C))
segment r   = seg(I, T)           [auxiliary]

right_angle ra = right_angle(T, I, B)

// ---- annotation ------------------------------------------
label lA = label(A, $A$)
label lB = label(B, $B$)
label lC = label(C, $C$)
label lr = label(r.midpoint, $r$)  { dir: NE }

// ---- intent ----------------------------------------------
assert tangent(inc, seg(B, C))
assert perpendicular(seg(I, T), seg(B, C))
assert dist(I, T) == radius(inc)
```

The three asserts pass **by construction** — the agent could not have gotten them wrong — which is the design thesis in one file.

---

## Part 10 — Open questions before freezing

1. **`update --def` vs. pure file editing** — with a line-oriented format, `update`/`remove` may be redundant; keeping them adds safety but widens the CLI. Lean: keep, mark as convenience.
2. **Polygon edge anchors** (`tri.edge(0)`) — still excluded due to index unreliability; revisit if real figures make explicit `seg(A,B)` too noisy.
3. **Relative default tolerance** — chosen over absolute; confirm once real figures with extreme scales are tested.
4. **KaTeX subset boundary** — which TeX commands are guaranteed? Needs an explicit whitelist appendix before v1.
