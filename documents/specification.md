# Locus Specification v0.3

*A relational language and CLI toolchain for plane-geometry figures and technical diagrams with precise alignment and ratios.*

This document is self-contained and normative. It supersedes v0.2 (constructive-only model), which is deleted; the paradigm change is summarized in §0.1.

---

## 0. Design principles (informative)

1. **The agent declares relationships; Locus computes coordinates.** Objects carry degrees of freedom; relations consume them; a deterministic solver finds the coordinates. Raw coordinates exist only as *hints* (solver seeds, never truth) and in the rare `pin` relation.
2. **The `.locus` file is the figure's source code** — ordered, diffable, hand-editable, self-testing (asserts live in the file).
3. **Text-based verification is the primary feedback channel.** `describe` / `inspect` / `check` must be complete enough that a vision-less agent can catch every mistake, including wrong solution branches and leftover degrees of freedom.
4. **Determinism is normative.** Identical file → identical solve → identical resolved geometry → byte-identical SVG.
5. **Statement-per-line syntax with compiler-grade diagnostics.** Local errors stay local; error messages name source lines and suggest fixes.

### 0.1 The v0.3 paradigm shift (informative)

v0.2 was purely **constructive**: every object was a deterministic function of earlier objects, multi-valued constructions needed memorized branch indices, and alignment relations ("horizontally aligned", "0.8 apart") were inexpressible. v0.3 is **relational**:

1. Objects are described *relative to* free points or other objects — never by absolute truth-coordinates.
2. Relations between objects (`tangent`, `on`, `horizontal`, `dist(A,B) == 6`, …) are first-class statements that *define* geometry, not just check it.

This is the geometric-constraint paradigm proven by CAD sketchers (SolveSpace, FreeCAD/planegcs). The constructive constructors survive as the **derived tier** — direct evaluation, zero unknowns — under the doctrine: **construct when you can, relate when you must.**

---

## Part 1 — The Locus language

### 1.1 File identity

- Extension: `*.locus`; encoding UTF-8; newline-terminated statements.
- Comments: `//` to end of line. Outside strings and TeX spans, a comment may occupy a whole line or follow any statement.
- Header: `locus 2` (the v0.3 language is format version 2).
- The file stores the **relational program**, never resolved coordinates. Resolved geometry is always derived by solving + evaluation.
- Machine interchange: `locus fmt --json` emits the parsed AST as JSON (§9.4). The DSL is the authoring format; JSON is derived, never authored.

### 1.2 File shape

```locus
locus 2                      // format version header, mandatory, first statement

meta {
  title "Externally tangent circles"
}

style textbook               // optional; defaults to textbook
class faint { stroke: dash(5, 3), weight: 0.75, accent: #6B7280 }   // optional style classes (§8.3)

// ---- objects ----
point O_1 = free ~(0, 0)
point O_2 = free ~(5, 0) where { horizontal(O_1, O_2) }
circle c1 = circle(O_1, 2)
circle c2 = circle(O_2, 3)   [highlight]
line  ax  = line(O_1, O_2)   [faint]

// ---- relations ----
rel tangent(c1, c2)

// ---- annotation ----
label l1 = label(O_1, $O_1$)  { dir: S }

// ---- intent ----
assert dist(O_1, O_2) == 5
```

### 1.3 Grammar (EBNF)

```
file          := header (statement | eol)*
header        := "locus" integer eol
statement     := op | rel | assert | meta | style | class

op            := free_point_op | free_num_op | derived_op
free_point_op := "point" ident "=" "free" point_hint [where] [attr] [props] eol
free_num_op   := "num" ident "=" "free" scalar_hint [where] [attr] [props] eol
derived_op    := kind ident "=" rhs [where] [attr] [props] eol
rhs           := constructor | expr                -- bare expr is valid only when kind = "num"
kind        := "point" | "num" | "line" | "ray" | "segment" | "circle" | "arc"
             | "polygon" | "rect" | "diamond"
             | "label" | "arrow" | "tick" | "right_angle" | "text_block"
constructor := ident "(" [args] ")"
point_hint  := "~" "(" expr "," expr ")"
scalar_hint := "~" expr
where       := "where" "{" boolexpr ("," boolexpr)* "}"
attr        := "[" ident "]"                       -- built-in or declared style class
props       := "{" [prop ("," prop)*] "}"
prop        := ident ":" propvalue
propvalue   := expr | string | ident | headtok | hexcolor
headtok     := "->" | "<->" | "-"                  -- dedicated tokens (arrow heads)
hexcolor    := "#" hex_digit hex_digit hex_digit hex_digit hex_digit hex_digit

rel         := "rel" boolexpr eol
assert      := "assert" boolexpr ["tol" number] eol
meta        := "meta" "{" eol (ident string eol)* "}" eol
style       := "style" ident eol
class       := "class" ident props eol             -- declared style class (§8.3)

args        := expr ("," expr)*
expr        := term (("+" | "-") term)*
term        := unary (("*" | "/") unary)*
unary       := ["-"] postfix
postfix     := atom ("." ident ["(" args ")"])*    -- property access on any value
atom        := number [unit] | boolean | string | tex_literal
             | funcall | ref | "(" expr ")"
funcall     := ident "(" [args] ")"
ref         := ident
unit        := "deg" | "rad"
boolean     := "true" | "false"

boolexpr    := bool_or
bool_or     := bool_and ("or" bool_and)*
bool_and    := bool_not ("and" bool_not)*
bool_not    := ["not"] bool_primary
bool_primary := cmp | predicate | "(" boolexpr ")"
cmp         := expr ("==" | "<" | ">" | "<=" | ">=") expr
predicate   := ident "(" args ")"

number      := (digits ["." [digits]] | "." digits) [exponent]
integer     := digits
exponent    := ("e" | "E") [("+" | "-")] digits
digits      := digit+
digit       := "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"
hex_digit   := digit | "A" | "B" | "C" | "D" | "E" | "F"
                   | "a" | "b" | "c" | "d" | "e" | "f"
ident       := [A-Za-z][A-Za-z0-9_']*
text_value  := string | tex_literal
string      := '"' (plain_char | escape | inline_tex)* '"'
escape      := '\\"' | '\\\\' | '\\$' | '\\n'
inline_tex  := "$" tex_chars "$"                  -- TeX embedded in surrounding plain text
tex_literal := "$" tex_chars "$"                  -- an all-TeX text value
COMMENT     := "//" …end of line
eol         := [COMMENT] NEWLINE
```

`plain_char` excludes raw newlines, `"`, `\`, and `$`. `tex_chars` excludes raw newlines and an unescaped `$`; backslashes inside a TeX span are preserved verbatim. These lexical exclusions make a quoted string and a bare TeX literal unambiguous.

Note the `postfix` production: property access applies to any expression, so `seg(B, D).midpoint` and `rect(...)` anchors compose without intermediate ids.

### 1.4 Statement rules

- **One logical statement per line.** No semicolons or continuation syntax. The `meta` block is the sole structural exception: its opening delimiter, each `key "value"` entry, and its closing delimiter occupy separate lines. Parse errors are recovered at the next line boundary, so one bad statement or metadata entry never invalidates the file's remaining diagnostics.
- **Comments are trivia.** The lexer removes `//` through the end of its physical line when it occurs outside a quoted string or TeX span; the terminating newline remains for recovery. This makes both comment-only lines and trailing comments valid without adding comment alternatives to every production.
- **`kind id = rhs`** is deliberately redundant: the declared kind must match the result type. This cross-check catches agent mistakes early (`point M = seg(A,B)` → immediate type error) and makes files skimmable.
- **`free` is reserved declaration syntax, not a constructor or identifier.** `point` uses a two-expression point hint and `num` uses a scalar hint. A missing hint, a point-shaped hint on a `num`, or a scalar hint on a `point` is a local parse error.
- **Identifiers**: unique per file, `[A-Za-z][A-Za-z0-9_']*`. Convention (not enforced): single/primed letters for points (`A`, `B'`, `M_1`), words for derived objects (`tri`, `inc`).
- **Reference order**: an op's `rhs` and `where` clause may only reference ids defined on earlier lines. `rel` and `assert` statements may reference any id defined above them; `rel` equations constrain *earlier* objects (the solve is simultaneous, §7).
- **`[attr]`** — a style class: one of the built-ins `main` (default) | `auxiliary` | `highlight` | `construction`, or a `class` declared on an earlier line (§8.3). A class may use preset tokens or exact color, width, dash, opacity, cap, and join values.
- **`{props}`** — op-specific fields (closed set per kind, §5.7). Unknown props are errors.
- **Text strings may mix plain text and TeX.** Outside a TeX span, quoted strings support `\"`, `\\`, `\$`, and `\n`; `\n` produces a centered multi-line text stack. An unescaped `$` opens or closes an inline TeX span, so `"filter offset $O_2$"` is stored as three ordered segments: plain text, TeX, plain text. Use `\$` for a literal dollar sign in the plain-text portion. An unmatched delimiter is a local syntax error.
- **TeX spans preserve backslashes.** Inside either an inline span or a bare all-TeX literal, TeX source is not processed as string escapes: `"angle $\alpha$"` and `$\triangle ABC$` both deliver the backslash to the TeX engine. Raw newlines and nested unescaped dollar delimiters are invalid. `\$` inside TeX remains TeX source for a rendered dollar and does not close the span.
- **Text is represented as segments.** The AST stores every text value as an ordered list of `{kind: "plain" | "tex", source: string}` segments. A bare `$...$` value is therefore one TeX segment, a string without inline spans is one plain segment, and a mixed string has multiple segments. Formatting preserves segment boundaries and escapes canonically.

---

## Part 2 — The object model: free and derived

Every object is either **derived** or **free**.

### 2.1 Derived tier (direct, zero unknowns)

A derived object is a deterministic `f64` function of previously defined objects (`midpoint(A, B)`, `incircle(A, B, C)`, `foot(P, l)`, …). It introduces no unknowns and has no iterative convergence of its own. Derived definitions are evaluated at every solver trial that needs them and once more for the final scene; a violated construction precondition is a construction error (§7.5). “Direct” means evaluation without an additional solve, not symbolic or infinite-precision arithmetic. **Construct when you can, relate when you must.**

### 2.2 Free tier (unknowns)

Only two kinds may be free:

```
point P = free ~(2, 1)          -- 2 DOF (x, y)
num   r = free ~1.5             -- 1 DOF (scalar parameter)
```

Every other "free-ish" object is built from free points and nums: a free circle is `circle(O, r)` with free `O` and free `r`. The unknown vector is therefore trivially well-defined: the concatenation of all `free` declarations in file order.

No other kind may be free — this is normative, not provisional. Free lines were considered and rejected: lines through free points covered every stress-test case, and a second parameterization of unknowns would complicate DOF reporting for no expressive gain.

`num` also has a derived form — a named constant or computed scalar:

```
num fw  = 4
num phi = (1 + sqrt(5)) / 2
```

### 2.3 Hints (`~`) — the branch selector, not the truth

Every `free` declaration **must** carry a hint. Semantics:

- The hint is the **solver seed**. It selects *which* solution you get when relations admit several (inner vs. outer tangency, two intersection points, mirror configurations). Instead of memorizing branch-ordering conventions, the agent sketches roughly where the object is; a wrong branch is fixed by moving the hint.
- The hint is **never the truth**. A converged figure satisfies every relation exactly regardless of how sloppy the hint was. Coordinate hallucination is thereby *contained*, not trusted: a wrong hint can pick the wrong branch (visible in `describe`, flagged as `hint_far` when the solution lands far from the seed), but can never make a stated relation false.
- **Under-constrained DOFs settle toward their hints.** The fixed projected solver step moves current null-space components toward the hint vector without accepting a worse relation objective (§7.1). This deterministically settles local gauge freedoms such as translation and rotation, but does not promise a global closest solution across disconnected branches. Generic-position figures (most textbook geometry) are under-constrained *by design*; this is reported as information, not an error (§7.3).

### 2.4 `where` sugar

Relations may be attached at the declaration site; this desugars to standalone `rel` statements immediately following the op:

```locus
point T = free ~(2, 0) where { on(T, c1), on(T, c2) }
-- ≡
point T = free ~(2, 0)
rel on(T, c1)
rel on(T, c2)
```

A `where` clause may only reference earlier ids (plus the id being declared).

---

## Part 3 — Relations

### 3.1 One relation language, two modes

The boolean expression language is shared between `rel` and `assert`; only the keyword differs:

```
rel    tangent(c1, c2)        -- constrains: the solver must make this true
assert tangent(c1, c2)        -- checks: verified after solving, never solved for
```

**`rel` is an input to the solver; `assert` is an independent test.** A well-written figure uses both: `rel` for the defining facts, `assert` for derived truths that should follow — the figure carries its own tests.

### 3.2 Relation vocabulary (usable in `rel` and `assert`)

| Relation | Meaning and raw residual | Equations |
|---|---|---|
| `expr == expr` | matching numeric values; `lhs − rhs` | 1 |
| `on(P, obj)` | zero signed distance to a line, radial difference to a circle, or zero closest-point distance to a closed ray/segment/arc | 1 |
| `tangent(c, l)` | circle tangent to a line/ray/segment; closest distance from center to the closed linear object minus radius | 1 |
| `tangent(c1, c2)` | seed-selected external or internal circle tangency; center distance minus the selected radius target | 1 |
| `parallel(l1, l2)` | parallel linear objects; normalized 2D cross product of their direction vectors | 1 |
| `perpendicular(l1, l2)` | perpendicular linear objects; normalized dot product of their direction vectors | 1 |
| `collinear(A, B, C, ...)` | points after the first two on their line; one signed perpendicular distance for each remaining point | n−2 |
| `concyclic(A, B, C, D, ...)` | points after the first three on their circumcircle; one radial difference for each remaining point | n−3 |
| `congruent(s1, s2)` | equal segment lengths; `length(s1) − length(s2)` | 1 |
| `coincident(P, Q)` | same point; `x(P) − x(Q)` and `y(P) − y(Q)` | 2 |
| `midpoint(M, A, B)` | relational form of the constructor; component differences from `(A+B)/2` | 2 |
| `horizontal(P, Q)` / `vertical(P, Q)` | `y(Q) − y(P)` / `x(Q) − x(P)` | 1 |
| `pin(P, x, y)` | P exactly at length-valued `(x, y)`; two component differences | 2 |

Here a **linear object** is a line, ray, or segment. Its direction is the ordered vector from its defining first point to its second point; a zero direction is degenerate. `parallel` and `perpendicular` accept any pair of linear objects. A ray, segment, and arc are closed sets for `on`, so their endpoints count as on the object. Closest-point distance is the ordinary Euclidean distance to that closed set. For `tangent(c, ray|segment)`, endpoint-only contact counts as tangency of the two closed rendered objects; callers that require tangency to the supporting line can additionally assert perpendicularity to the radius.

Circle-circle tangency makes its discrete choice exactly once at the seed. Let `d₀` be the seed center distance and let the positive seed radii be `r₁₀`, `r₂₀`. Choose external tangency when `abs(d₀ − (r₁₀+r₂₀)) <= abs(d₀ − abs(r₁₀−r₂₀))`; ties therefore choose external. Its residual target is `r₁+r₂`. Otherwise choose internal tangency, record which seed radius is larger as the containing circle, and use residual target `r_outer−r_inner`. Trials in which that recorded radius ordering reverses are invalid. Equal seed radii cannot select internal tangency.

All raw residuals are normalized by §7.4. `collinear` requires its first two points to be distinct. `concyclic` requires its first three points to be distinct and non-collinear. Direction normalization in `parallel`/`perpendicular` divides by both direction lengths. Violating one of these preconditions produces a source-anchored degenerate-geometry error instead of an arbitrary truth value.

Signed axis offsets are expressed through the numeric functions `hdist`/`vdist` (§6.2) under `==`: `rel vdist(b.n, a.s) == 0.8`.

There are **no aliases**: exactly one name per concept. (A `touch` alias for `tangent` was considered and rejected — synonyms multiply the surface an agent must recall and the surface `describe` must print.)

### 3.3 Assert-only predicates

Inherently inequality-shaped conditions cannot be solved for in v0.3 and are **assert-only**: `between(A, B, C)`, `inside(P, poly)`, `inside(P, c)`, and the comparisons `<  >  <=  >=`. Boolean `or` and `not` are also assert-only because they do not define a single smooth equation set. A `rel` or `where` clause may contain a solvable atomic equality/predicate or an `and` conjunction of them; using an assert-only form there is a semantic error. Ordering, containment, and other discrete choices are selected by hints or constructions, then *verified* by asserts.

`between(A, B, C)` is strict: it requires `B` to be collinear with distinct `A` and `C`, and the projection parameter of `B` on directed segment `A→C` to lie inside the tolerance-shrunk interval `(ε, 1−ε)`. Endpoints are not between. `inside(P, c)` is also strict and requires `(radius(c) − dist(center(c), P)) / D > ε`; a boundary point is not inside. `inside(P, poly)` requires a valid simple polygon, first rejects points within tolerance of any closed edge, then uses the nonzero winding rule; boundary points are not inside. These definitions use the assert's `ε` from §7.4.

---

## Part 4 — Coordinate and angle conventions

- Cartesian scene space, **y-up**. The renderer flips to screen space; scene coordinates never denote pixels. Viewport is auto-fitted (§8.4).
- `hdist(P, Q) = x(Q) − x(P)`; `vdist(P, Q) = y(Q) − y(P)` (signed).
- Angles are CCW-positive. The unit suffix is **mandatory** on angle literals: `60deg`, `1.2rad`. A bare number in an angle position is a parse error (this kills the degrees/radians hallucination class).
- Scene units are arbitrary; figures state their own scale conventions in `meta` if useful (e.g. "1 unit = 32 ms").

---

## Part 5 — Object kinds and constructors

### 5.1 `point` (derived constructors)

```
midpoint(A, B)
lerp(A, B, t)                   -- t=0 → A, t=1 → B; t may lie outside [0,1]
offset(P, dx, dy)               -- P + (dx, dy); sugar for a free point with hdist/vdist rels
intersect(o1, o2 [, k])         -- k: branch index, optional when the intersection is unique
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

The idiomatic v0.3 alternative to `intersect` is a free point with two `on` relations and a hint — no branch conventions to memorize. `intersect` survives for exactness and brevity when the branch is unique; when two branches exist and `k` is omitted, that is an error (never a default). Branch ordering for explicit `k`: sorted by the parameter of `o1` — along its direction for a line/ray/segment, by CCW angle from +x for a circle. `locus describe` always prints which branch resolved where.

### 5.2 `num`

```
num fw = 4                      -- derived: any scalar expression of earlier ids
num r  = free ~1.5              -- free: 1 DOF
```

### 5.3 `line`, `ray`, `segment`

```
line(A, B)          ray(A, B)          seg(A, B)
perpendicular(P, l)                 -- line through P ⊥ l
parallel(P, l)                      -- line through P ∥ l
perp_bisector(A, B)
angle_bisector(A, B, C)             -- interior bisector at vertex B
tangent_line(P, c, k)               -- P strictly outside c; k is 0 or 1
```

`tangent_line` always has two branches under its strict-outside precondition, so `k` is mandatory. Branches are sorted by the CCW angle of their contact point on `c`, in `[0deg, 360deg)`; `k = 0` selects the smaller angle and `k = 1` the larger. A non-integer or out-of-range branch is a type error.

The relational alternative to `tangent_line`: a free point `P_1` with `on(P_1, c)`, the line `line(P, P_1)`, and `rel tangent(c, line(P, P_1))` — the hint on `P_1` picks the side.

### 5.4 `circle`, `arc`

```
circle(O, r)                        -- r: expression, e.g. dist(A, B) / 2, or a free num
circle_through(O, A)
circle_diameter(A, B)
circumcircle(A, B, C)
incircle(A, B, C)
arc(c, P, Q, ccw)                   -- ccw: true|false; P, Q must lie on c (checked)
angle_arc(A, B, C, r)               -- angle-mark arc at vertex B, radius r
```

### 5.5 `polygon`, `rect`, `diamond`

```
polygon(A, B, C, ...)               -- ≥ 3 vertices, listed in order
rect(C, w, h)                       -- axis-aligned rectangle, center C, width w, height h
diamond(C, w, h)                    -- axis-aligned rhombus (flowchart decision), center C
```

A polygon implies its edges for incidence detection, label collision, and rendering, but does **not** auto-create edge ids; an agent needing a specific edge writes `seg(A, B)` explicitly (index-based edge access is deliberately absent — agents are unreliable with indices). `regular(...)` from v0.2 is removed: its vertices were unaddressable; construct vertices by `rotate` instead.

`rect` and `diamond` exist because box-and-arrow diagrams drowned in corner plumbing without them. Their anchors (§5.8) are ordinary derived points, legal in constructors and relations.

### 5.6 Annotations

```
label(anchor, text)
arrow(from_anchor, to_anchor)
tick(s, n)                          -- n equal-length tick marks on segment s, n ∈ 1..4
right_angle(V, P, Q)                -- square mark at V between rays V→P and V→Q
text_block(text)
```

The `text` argument accepts a quoted text string, including any number of inline `$...$` TeX spans, or a bare all-TeX `$...$` literal. See §1.4 and §8.2.

### 5.7 Props per kind

| Kind | Props | Default |
|---|---|---|
| `label` | `dir: auto \| center \| N \| NE \| E \| SE \| S \| SW \| W \| NW`; `gap: expr` | `auto`; style-derived gap |
| `arrow` | `head: -> \| <-> \| -`; `bend: angle` | `->`; `0deg` |
| `angle_arc` | `n: 1..4` (multiplicity marks, for distinguishing equal-angle families) | `1` |
| `right_angle` | `size: expr` | style-derived |
| `text_block` | `place: below \| above \| left \| right` (relative to figure bbox) or `anchor: <anchor>, dir: <dir>` | `below` |
| `point` | `mark: dot \| cross \| none` | `dot` |

`dir: center` places the label centered on its anchor with zero gap — text inside a `rect`/`diamond`/box.

### 5.8 Anchors

Anywhere a point is accepted, an **anchor expression** is accepted:

```
A                       -- any point id
s.midpoint  s.start  s.end
c.center    c.at(30deg)
poly.centroid
r.center  r.n  r.s  r.e  r.w  r.ne  r.nw  r.se  r.sw     -- rect / diamond
r.width   r.height                                        -- numeric anchors
lbl.center              -- resolved label box center (annotation layer ONLY)
```

Geometry-tier anchors (everything above except `lbl.center`) are ordinary derived points: legal in constructors, `rel`, and `assert`. Label-box anchors resolve *after* layout (§8) and are legal only in annotations — geometry must never depend on layout.

### 5.9 Domains and degeneracy

Every parsed number, evaluated expression, hint, solved value, and resolved coordinate must be finite. Overflow, NaN, or infinity is an error at the source expression that produced it.

- Circle radii, rectangle/diamond widths and heights, and `angle_arc` radii must be strictly positive. Label gaps are nonnegative. A `right_angle` size, whether explicit or style-derived, is strictly positive.
- A free `num` has no universal sign restriction; its uses impose constructor-specific domains. A trial vector that violates a domain is rejected as described in §7.1, while a final violation is a construction error.
- A line or ray requires distinct defining points. A zero-length segment may be stored and rendered as a point, but any operation requiring its direction—including `parallel`, `perpendicular`, `foot`, and line-like incidence—reports degeneracy.
- A polygon requires at least three distinct vertices, nonzero signed area, no crossing non-adjacent edges, and no adjacent collinear overlap; it may be clockwise or counterclockwise. This is the “simple polygon” required by `inside`.
- `arc(c, P, Q, ccw)` requires distinct endpoints on `c`. Endpoint incidence is checked with normalized residual `< 1e-12`; `ccw` selects the directed sweep from `P` to `Q`. A full circle must be represented as `circle`, not as an arc with coincident endpoints.
- `angle_arc(A, B, C, r)` and `right_angle(V, P, Q)` require two nonzero rays at their vertex. `tick(s, n)` requires a nonzero segment and an integer `n` in `1..4`.
- Division requires a nonzero denominator under the normalized `1e-12` construction threshold. `sqrt` requires a nonnegative operand; an operand within normalized `1e-12` below zero is clamped to zero, while a more negative operand is an error.

Degenerate inputs never acquire arbitrary fallback directions, centers, branches, or truth values. They produce the source-anchored diagnostics required by §7.5.

---

## Part 6 — Expression language

### 6.1 Numeric dimensions and inference

Every numeric expression has one of four dimensions: **scalar**, **length**, **area**, or **angle**. Length is measured in the file's arbitrary scene unit; area is its square. Angles are stored internally in radians, with `deg` and `rad` accepted at the source boundary. Dimensions are semantic types, not user-selectable unit systems.

- Geometry determines dimensions: `dist`, `hdist`, `vdist`, `radius`, `length`, rectangle dimensions, point-hint components, and coordinate offsets are lengths; `area` is area; `angle`, `dir_angle`, and arrow `bend` are angles; counts and ratios are scalars.
- A numeric literal without `deg` or `rad` begins as a dimension variable and is unified from context. Thus the `6` in `dist(A, B) == 6` becomes a length, while the `2` in `2 * angle(A, B, C)` is scalar. An otherwise unconstrained literal defaults to scalar.
- Each named `num`, including a free `num`, has one dimension inferred from all of its uses in the file. Conflicting uses are a type error. A free numeric hint has the inferred dimension of its declaration.
- Addition, subtraction, comparison, `min`, and `max` require matching dimensions. Multiplication or division by a scalar preserves the other operand's dimension. `length * length` produces area; `area / length` produces length; equal-dimension division produces scalar. Other dimension products are errors in format version 2.
- `abs` preserves dimension. `sqrt` accepts a scalar or an area and returns a scalar or length respectively. `ratio(a, b)` requires equal dimensions and returns scalar.
- Constructor arguments and properties impose their documented dimensions. In particular, circle radii, rectangle/diamond sizes, label gaps, tick/right-angle sizes, and all point offsets are lengths. A bare number in an angle position remains an error; contextual inference never turns it into an angle without `deg` or `rad`.

Dimension inference is whole-file and deterministic even though value references remain definition-before-use. The JSON AST reports the inferred dimension for every numeric declaration and expression so agents can localize type errors.

### 6.2 Numeric functions

| Function | Result |
|---|---|
| `dist(P, Q)` | Euclidean distance |
| `hdist(P, Q)` / `vdist(P, Q)` | signed axis offsets: `x(Q)−x(P)` / `y(Q)−y(P)` |
| `angle(A, B, C)` | unsigned interior angle at `B`, in `(0deg, 180deg)` |
| `dir_angle(A, B)` | CCW angle of vector A→B from +x, in `[0deg, 360deg)` |
| `ratio(a, b)` | `a / b` (sugar — agents state ratios constantly) |
| `radius(c)` `length(s)` `area(poly)` `perimeter(poly)` | as named |
| `sqrt(x)` `abs(x)` `min(a, b)` `max(a, b)` | as named |

All numeric functions are differentiable almost everywhere and legal inside `rel` equations; the solver composes through them (and through derived-object definitions).

### 6.3 Predicate summary

Relations (§3.2) work in both `rel` and `assert`. Assert-only: `between`, `inside`, `< > <= >=` (§3.3). Angle/length equality uses `==` on numeric functions rather than dedicated predicates: `angle(A, B, C) == 90deg`.

---

## Part 7 — Solve and evaluation semantics (normative)

### 7.1 The solve

The format-version-2 reference solver is defined below. Implementations may optimize it only if they produce the same resolved `f64` values and reports for every valid input; changing any constant or acceptance rule requires a new format version.

1. **Unknown vector and seed.** Concatenate point `(x, y)` components and free numeric values in declaration order. The seed `h` is the hint vector exactly as written. Normalize unknown coordinates before linear algebra: point coordinates and length-valued nums by `D`, area-valued nums by `D²`, angle-valued nums by `1 rad`, and scalar nums by `max(1, abs(hᵢ))`, where `D` is the solve-time characteristic length from §7.4. The normalized vector is `z`; its normalized hint is `zₕ`.
2. **Residual vector.** Desugar `where` clauses in source order, flatten allowed `and` conjunctions left-to-right, and expand each relation into the component residuals specified in §3.2. Evaluate them in that order and normalize each by §7.4. Discrete branches are selected once from the seed as specified in §3.2 and remain fixed for the solve.
3. **Derived evaluation and differentiation.** For the seed and every trial vector, evaluate referenced derived definitions top-to-bottom as pure functions of the current free values before evaluating residuals. Compute the Jacobian `J = ∂r/∂z` with forward-mode automatic differentiation through those definitions; finite differences are not conforming. At a piecewise definition, differentiate the seed-selected active piece. A construction precondition failure at the initial seed is a degenerate-seed error; at a later rejected trial it makes that trial have infinite objective without leaking NaN or infinity into output.
4. **Rank and null space.** Compute an SVD of `J` with the deterministic one-sided Jacobi procedure below. A singular value has rank when `σᵢ > 1e-10 × max(σ₀, 1)`. The Moore–Penrose pseudoinverse `J⁺` uses exactly those retained singular values, and the null-space projector is `N = I − J⁺J`. Sign choices for singular vectors do not affect `J⁺` or `N`.
5. **LM trial step.** Start with `λ = 1e-3`. At each attempt, compute the primary step from `(JᵀJ + λI) δₚ = −Jᵀr` using Cholesky factorization in ascending row/column order. Compute the hint-settling component `δₕ = N(zₕ − (z + δₚ))`, then use `δ = δₚ + δₕ / (1 + λ)`. All dot products and sums execute sequentially in index order; contraction into fused multiply-add is forbidden.
6. **Acceptance schedule.** Let `F(z) = 0.5 × Σ rᵢ²`, summed in residual order. Accept a finite trial if its maximum absolute residual is below `1e-12`, or otherwise only if its `F` is strictly smaller. After acceptance set `λ = max(λ / 10, 1e-15)` and recompute values, `J`, rank, and `N`. After rejection keep `z` unchanged and set `λ = min(λ × 10, 1e15)`. An attempt, accepted or rejected, counts as one iteration. Stop successfully when `max(abs(rᵢ)) < 1e-12`; a file with no relations succeeds at its hints in zero iterations. Fail after 200 attempts or after a rejection at `λ = 1e15`.
7. **Lexicographic hint semantics.** Relation satisfaction is the primary objective. Null-space motion toward `zₕ` is secondary and may never justify accepting a worse primary objective. At convergence, the reported hint-settled state is the result of the fixed projected schedule above; the specification does not claim a global closest solution across disconnected branches.
8. **Final evaluation and asserts.** Re-evaluate every derived object top-to-bottom at the accepted free vector, build the final scene, and then check asserts using §7.4. This final pass uses the same functions as residual evaluation and introduces no new geometry semantics.

All arithmetic is IEEE-754 binary64, round-to-nearest ties-to-even, with subnormals preserved. Transcendental functions come from the format-version-pinned `libm` implementation. No randomness, parallel reductions, locale-sensitive parsing/formatting, reassociation, or platform BLAS/LAPACK calls are permitted in the normative path.

For the one-sided Jacobi SVD, start with `A = J` and `V = I`. A sweep visits every column pair `(p, q)` in lexicographic order. Let `α = dot(Ap, Ap)`, `β = dot(Aq, Aq)`, and `γ = dot(Ap, Aq)`, with dots summed by ascending row. Skip the pair if either norm is zero or `abs(γ) <= 2^-48 × sqrt(αβ)`. Otherwise set `ζ = (β−α)/(2γ)`, `t = 1` when `ζ = 0`, and otherwise `t = sign(ζ)/(abs(ζ)+sqrt(1+ζ²))`; then `c = 1/sqrt(1+t²)` and `s = c×t`. Replace the columns with `(c×Ap−s×Aq, s×Ap+c×Aq)` and apply the same rotation to `V`. Stop after the first sweep that performs no rotations, or fail with numerical non-convergence after 64 sweeps. Singular values are the final column norms; sort them descending with the original column index as the stable tie-break, and derive `U` by normalizing nonzero columns. This procedure, including its skip threshold and ordering, is part of format version 2.

### 7.2 DOF accounting

`check` and `describe` always report unknown count, equation count, rank, global remaining DOFs (`unknowns − rank`), attempts, accepted steps, maximum normalized residual, and final damping.

For each free object, take the rows of the final null-space projector `N` (§7.1) belonging to that object's components. The rank of that row block, using the same singular-value threshold as §7.1, is its **local hint-settled DOF count**. Zero means fully constrained; a positive value means that many independent global null directions can move the object. Local counts intentionally need not sum to the global remaining count because one global freedom, such as translation, can move several objects.

Hint displacement is reported both in source dimensions and in normalized unknown coordinates. `hint_far` is emitted when the maximum absolute normalized component displacement for an object exceeds `0.25`. This threshold means one quarter of the characteristic length for coordinates/lengths, one quarter radian for angles, and one quarter of the §7.1 scalar/area normalization. The diagnostic includes both the threshold and actual displacement rather than relying on the flag alone.

### 7.3 Failure taxonomy

| Condition | Detection | Exit | Report |
|---|---|---|---|
| Under-constrained | rank(J) < #unknowns at solution | 0 + info; `--strict`: 1 + warning | per-object hint-settled DOFs, with suggested relations |
| Over-constrained, consistent | redundant rows, residuals converge | 0 + warning; `--strict`: 1 | the redundant relation(s) by source line |
| Over-constrained, conflicting | no convergence to tolerance | 2 | best-fit residual per relation, worst first; minimal infeasible subset when isolable |
| Non-convergence / degenerate seed | iteration cap, damping ceiling, or construction/residual undefined at seed | 2 | failing seed geometry or final residuals; suggestion to spread or correct hints |

Generic-position figures are under-constrained by design, hence "info" not "warning" by default.

`--strict` is a `check` policy, not a different solver: it promotes under-constraint information to warnings and returns exit 1 if any warning remains, including a consistent redundant relation. Without `--strict`, warnings are reported but do not change a successful exit code.

```
figure.locus  solve: 7 unknowns, 6 equations, rank 6
  info[I310]: P is under-constrained (1 DOF settled by hint)
     12 | point P = free ~(2, 1)
        = add a relation if unintended, e.g. on(P, …) or vdist(…) == …
  solved: max scaled residual 3.1e-13 in 6 iterations
```

### 7.4 Tolerance

All tolerances operate on a **normalized, dimensionless residual**. The default assert/incidence threshold is `ε = 1e-9`. `tol t` replaces `ε` for every atomic comparison or predicate inside that assert; `t` must be a finite positive scalar literal. It is a normalized factor, never an absolute scene-unit distance.

The characteristic length `D` is computed deterministically. During solving it is the bounding-box diameter of all geometry evaluated at the hint vector; during assertion and incidence checking it is the geometry-only solved bounding-box diameter. Circle/arc extents and polygon/rect/diamond vertices participate whether or not their marks are visible. An unbounded line or ray contributes only its finite defining points, not an infinite carrier; annotations do not participate. If that diameter is zero, use the largest finite positive length-valued hint, literal, or constructor argument in declaration order; if none exists, use `D = 1`.

For numeric comparisons, both operands have the same inferred dimension (§6.1). Their normalization scale `S` is:

| Dimension | `S` |
|---|---|
| scalar | `max(1, abs(lhs), abs(rhs))` |
| length | `max(D, abs(lhs), abs(rhs))` |
| area | `max(D², abs(lhs), abs(rhs))` |
| angle | `1 rad` |

Equality passes when `abs(lhs − rhs) / S <= ε`. Strict comparison uses a tolerance dead band: `lhs < rhs` requires `(rhs − lhs) / S > ε`, while `lhs <= rhs` requires `(lhs − rhs) / S <= ε`; `>` and `>=` are symmetric. Boolean `and`, `or`, and `not` combine the already evaluated atomic truth values.

Each geometric predicate defines a raw residual with a dimension in §3.2; it is divided by the matching scale above before comparison with `ε`. Multi-equation predicates pass only when every component passes. Assert-only containment predicates use the boundary rules in §3.3. Detected incidences use the same normalized residuals and default `ε`, so `inspect` and `assert` cannot disagree solely because they use different epsilon systems.

The solve criterion is `max(abs(normalized relation residual)) < 1e-12`, strictly tighter than the default assert threshold. A converged solve therefore cannot fail its own `rel` when restated as a default-tolerance `assert` with the same branch and domain semantics.

### 7.5 Construction errors

Derived-tier failures — empty `intersect`, `circumcircle` of collinear points, `tangent_line` from an interior point, division by zero — abort with **exit code 2** and a source-anchored diagnostic:

```
figure.locus:9:11  error[E201]: intersect(c1, c2): circles do not intersect
                   c1 radius 1.20, c2 radius 0.80, center distance 3.41 > 2.00
   9 | point P = intersect(c1, c2)
```

Diagnostics must name the op id, the failing precondition, and the offending measured values. No NaN or infinity ever leaks into a resolved scene.

### 7.6 Determinism (normative)

Identical file bytes → identical solve → identical resolved coordinates → byte-identical SVG. All tie-breaks (LM schedule, branch ordering, label candidate selection, float formatting) are defined without randomness. `locus-core` must use a pinned math library (`libm` crate, not system libm) so this holds across platforms. This enables golden-file snapshot testing in CI, including solve-heavy fixtures on every platform in the matrix.

---

## Part 8 — Annotation layout and rendering

### 8.1 Label placement

For each label with `dir: auto`:

1. **Candidates**: 8 compass directions × 2 gap radii around the anchor, plus domain-informed preferences — vertex labels prefer the external angle bisector direction; segment labels prefer the two perpendicular offsets; circle labels prefer outward along the radius.
2. **Scoring**: penalties for overlapping strokes, other label boxes, or exceeding figure margin.
3. **Assignment**: greedy, most-constrained-first, deterministic tie-break (candidate list order). Any label left with a positive collision score is reported as a diagnostic — never silently accepted.

Pinned `dir` (including `center`) bypasses the heuristic entirely.

### 8.2 Mixed text and TeX

Each text value is laid out as its ordered segment list (§1.4). Plain segments use the preset's deterministic text font and metrics; TeX segments render server-side using the guaranteed TeX subset. Adjacent segments share one baseline and are concatenated without implicit whitespace. A `\n` in a plain segment ends the current line and starts a centered new line; TeX spans cannot contain raw line breaks.

The layout engine measures every segment with its actual renderer, composes the line boxes, and feeds the resulting **measured** bounding box into label placement. Extents are never estimated. Rendering a mixed value such as `"filter offset $O_2$"` therefore produces one label, not independently placeable fragments.

### 8.3 Style system

Styling is a two-layer system. Preset tokens provide concise semantic defaults, while exact values let technical diagrams reproduce an established visual language. Both forms are deterministic.

**Layer 1 — preset.** `style <ident>` selects the rendering preset for the whole figure. One built-in (`textbook`): thin ink strokes, small filled point disks, TeX sized relative to figure scale, and a fixed accent palette.

**Layer 2 — classes.** `class id { props }` declares a reusable style class; `[id]` applies it to any op. A class may mix preset tokens and exact values:

| Prop | Values | Default | Applies to |
|---|---|---|---|
| `stroke` | `solid \| dashed \| dotted \| dash(n, n, ...)` | `solid` | stroked geometry |
| `weight` | `thin \| normal \| bold \| number` | `normal` | stroked geometry |
| `accent` | named color or HEX color | `ink` | stroke, point mark, and text |
| `fill` | `none \| tint` or a named/HEX color | `none` | closed geometry |
| `opacity` | number in `[0, 1]` | `1` | the complete rendered op |
| `cap` | `butt \| round \| square` | `butt` | open stroke ends |
| `join` | `miter \| round \| bevel` | `miter` | stroke corners |

A named color is one of `ink | gray | blue | orange | teal | green | red | purple` and is resolved by the preset. A HEX color is an unquoted six-digit sRGB literal such as `#2563EB`; `locus fmt` canonicalizes its digits to uppercase. `accent` colors strokes, point marks, and label/text glyphs. `fill: tint` remains the preset's light wash of the resolved accent, while an explicit fill color is used unchanged.

A numeric `weight` is a finite value greater than zero, measured in CSS pixels; it bypasses the preset's `thin | normal | bold` mapping. SVG emits that value directly, PNG inherits it from the SVG, and TikZ converts it using `1px = 0.75pt`. `dash(...)` accepts at least two finite positive CSS-pixel lengths. An odd-length sequence is repeated once, matching SVG dash-array semantics. `solid`, `dashed`, and `dotted` remain preset-defined patterns.

Values are validated when the class is declared. Non-finite numbers, out-of-range opacity, non-positive weights or dash lengths, an empty/one-element dash sequence, and malformed HEX colors are source-anchored errors. Props that do not apply to an op kind are ignored; for example, `fill` has no effect on a line and `cap` has no effect on a label. Exact values do not weaken byte determinism: identical literals resolve to identical backend values without palette lookup.

For example:

```locus
class signal { accent: #2563EB, weight: 1.5, stroke: dash(6, 3), cap: round }
class region { accent: #0F766E, fill: #CCFBF1, opacity: 0.85, join: round }
```

The four built-ins are ordinary classes with reserved names:

| Built-in | Equivalent declaration |
|---|---|
| `main` | `{ stroke: solid, weight: normal, accent: ink, fill: none }` |
| `auxiliary` | `{ stroke: dashed, weight: thin, accent: ink }` |
| `construction` | `{ stroke: dotted, weight: thin, accent: gray }` |
| `highlight` | `{ stroke: solid, weight: normal, accent: blue, fill: tint }` |

When a class omits a prop, the built-in `main` value supplies the default; an exact class value takes precedence over its preset counterpart. Class names share the file's identifier namespace (unique, declared before use). Semantic built-ins remain the recommended default for textbook geometry; declared classes exist for diagram vocabulary — dashed lifelines, exact brand colors, color-coded signal families — which previously had to abuse `[auxiliary]` (finding F13, resolved).

### 8.4 Viewport

- Auto-fit: bounding box of all *visible* geometry + resolved label boxes, plus margin; y-flip to screen space. Points with `mark: none` that are referenced only as anchors do **not** extend the viewport.

---

## Part 9 — CLI

### 9.1 Commands

| Command | Purpose |
|---|---|
| `locus init <file> [--style textbook]` | create a new scene file |
| `locus add <file>` | append statements from stdin; whole file re-validated |
| `locus update <file> --id X --def "<rhs>"` / `--set 'dir=NE'` | surgical edit; re-solve |
| `locus remove <file> --id X` | remove; orphaned dependents → exit 2 |
| `locus describe <file>` | structural dump: objects, defs, resolved positions, solve summary, DOF report, hint displacements, branch resolutions |
| `locus inspect <file>` | measurements, detected incidences, diagnostics |
| `locus check <file>... [--strict]` | solve + diagnostics + all in-file asserts; warnings are non-fatal unless strict — the CI command |
| `locus fmt <file> [--json]` | canonical formatting / AST-as-JSON export |
| `locus render <file> -o out.svg [--png] [--tikz] [--stdout]` | render (intended for humans, not agent feedback) |

Since the format is line-oriented, agents may equally edit the file directly; `check` validates everything regardless of how edits happened. CLI ops are the "safe API," direct editing the "expert mode."

### 9.2 Exit codes

| Code | Meaning |
|---|---|
| `0` | success / all asserts pass |
| `1` | one or more asserts failed, or `check --strict` found warnings |
| `2` | solve failure (conflicting relations, non-convergence) or construction/dependency error |
| `3` | usage / parse error |

### 9.3 Output conventions

- All commands support `--json` for machine-readable output (agents); default output is human-readable with source-line anchors.
- `inspect --json` reports: measurement tables (pairwise distances among named points, triangle angles, ratios along shared lines), detected incidences ("`inc` tangent to `seg(A,B)`", "`M` on `AB` with `AM:MB = 1:1`"), and diagnostics (`degenerate`, `hint_far`, `label_collision`, `out_of_view`, `crowding`, `unreferenced`).
- On assert failure, the report includes the actual value of **every measurable subterm** so the agent can localize which upstream definition is wrong.

### 9.4 JSON interchange

`locus fmt --json` emits the AST: version, segmented meta text, style preset, declared classes, ordered ops (`{id, kind, def, hint, where, attr, props}` with `def` as both source string and parsed tree), rels, and asserts. Numeric expression nodes include their inferred dimension (§6.1); text nodes contain the ordered plain/TeX segments from §1.4. This is the integration surface for future MCP shims, editors, or watch-mode preview servers — all of which wrap the CLI, never reimplement it.

### 9.5 Intended agent loop

```bash
locus init fig.locus
locus add fig.locus < ops.locus
locus check fig.locus --json    # → solve summary + DOF report + assert results
# wrong branch? → move a hint. leftover DOF? → add a relation. conflict? → drop one.
locus render fig.locus -o fig.svg
```

Recommended agent policy: **never `render` until `check` exits 0.**

---

## Part 10 — Out of scope for v0.3 (reserved)

- Inequality relations in `rel` (only asserts may compare with `< > <= >=`).
- Content-driven box sizing (rect width from measured label extents) and arrow routing (elbows, box clipping) — the rest of "diagram mode."
- Free kinds beyond `point` and `num` (free lines considered and rejected, §2.2).
- User-defined presets or named palettes; per-op inline style declarations; whole-figure transforms; layers; animation; 3D. Exact values remain reusable through classes (§8.3).
- Statement continuations or other multi-line statements beyond the structured `meta` block; includes/imports; user-defined functions.

The `locus <n>` header exists so these can arrive without breaking earlier files.

---

## Part 11 — Complete example

```locus
locus 2

meta {
  title "Externally tangent circles with a common external tangent"
}

style textbook

// ---- objects ----------------------------------------------
point O_1 = free ~(0, 0)
point O_2 = free ~(5, 0) where { horizontal(O_1, O_2) }

num r_1 = 2
num r_2 = 3

circle c1 = circle(O_1, r_1)
circle c2 = circle(O_2, r_2)

rel tangent(c1, c2)                     // external tangency — picked by the hints

// tangency point: a free point pinned to both circles
point T = free ~(2, 0) where { on(T, c1), on(T, c2) }

// external homothety center — direct, so constructed
point E = lerp(O_1, O_2, r_1 / (r_1 - r_2))

// common external tangent: through E, touching c1 (upper side by hint)
point P_1 = free ~(-0.4, 2) where { on(P_1, c1) }
line t_ext = line(E, P_1)   [highlight]
rel tangent(c1, t_ext)

point P_2 = foot(O_2, t_ext)

// ---- annotation -------------------------------------------
label l1 = label(O_1, $O_1$)  { dir: S }
label l2 = label(O_2, $O_2$)  { dir: S }
label lT = label(T, $T$)      { dir: SE }

// ---- intent -----------------------------------------------
assert dist(O_1, O_2) == r_1 + r_2
assert between(O_1, T, O_2)
assert tangent(c2, t_ext)               // touches BOTH circles — the theorem
assert collinear(E, P_1, P_2)
```

The asserts pass **by relation** — the agent stated tangency once and got the tangent point, the center distance, and the common tangent as consequences. That is the design thesis in one file.

---

## Part 12 — Open questions before freezing

1. **Solver constants** — empirically validate the specified normalization, rank threshold, damping schedule, and `1e-12`/`1e-9` gap on extreme-scale figures before freezing format version 2; any change after freezing requires a new format version.
2. **KaTeX subset boundary** — which TeX commands are guaranteed? Needs an explicit whitelist appendix before v1.

### Decided (previously open)

- Hint-settled DOFs report as **info** by default; `--strict` promotes (§7.3).
- **No aliases** — `touch` removed; exactly one name per concept (§3.2).
- **No free lines** — `free` is `point`/`num` only, final (§2.2).
- **Style classes introduced** — reusable classes with a closed property set (§8.3); resolves F13.
- **Exact class styles supported** — HEX colors, numeric line weights, custom dash arrays, opacity, caps, and joins coexist with preset tokens (§8.3).
- **Mixed text and TeX supported** — quoted strings may contain measured inline `$...$` segments while bare `$...$` remains the all-TeX shorthand (§1.4, §8.2).
