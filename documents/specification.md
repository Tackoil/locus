# Locus Specification v0.4

*A relational language and CLI toolchain for plane-geometry figures and technical diagrams with precise alignment and ratios.*

This document is self-contained and normative. It supersedes v0.3 (the relational, coordinate-hinted format-2 model); the version changes are summarized in §0.1.

This revision adopts object-owned placement and replaces hinted free-point declarations with parameterless `point()` declarations. It is a breaking specification change; existing implementations and fixtures using point hints require migration. Format version 3 remains unfrozen pending the initialization rules in Part 12.

---

## 0. Design principles (informative)

1. **The agent declares geometry and relationships; Locus owns placement.** Objects carry degrees of freedom; relations consume them; a deterministic solver finds the coordinates internally. Authors and agents never supply absolute positions, coordinate hints, or pins. Points may be declared with `point()` or derived from anchors and geometric constructions; there is no separate location value.
2. **The `.locus` file is the figure's source code** — ordered, diffable, hand-editable, self-testing (asserts live in the file).
3. **Text-based verification is the primary feedback channel.** `describe` / `inspect` / `check` must be complete enough that a vision-less agent can catch every mistake, including wrong solution branches and leftover degrees of freedom.
4. **Determinism is normative.** Identical file → identical solve → identical resolved geometry → byte-identical SVG.
5. **Statement-per-line syntax with compiler-grade diagnostics.** Local errors stay local; error messages name source lines and suggest fixes.

### 0.1 Version history and the v0.4 model (informative)

v0.2 was purely **constructive**. v0.3 introduced **relations** and a solver, with coordinate hints on free points. v0.4 keeps the relational model, moves placement initialization into Locus, introduces parameterless `point()` and dimension-only shape declarations, makes circle tangency choices explicit, and adds ellipses and B-splines:

1. Geometry is declared by its intrinsic properties (`point()`, `circle(2)`, `rect(4, 2)`); its placement belongs to Locus. Objects are related through points and anchors, never through authored coordinates.
2. Relations between objects (`tangent`, `on`, `horizontal`, `dist(A,B) == 6`, …) are first-class statements that *define* geometry, not just check it.

This is the geometric-constraint paradigm proven by CAD sketchers (SolveSpace, FreeCAD/planegcs). The constructive constructors survive as the **derived tier** — direct evaluation, zero unknowns — under the doctrine: **construct when you can, relate when you must.**

---

## Part 1 — The Locus language

### 1.1 File identity

- Extension: `*.locus`; encoding UTF-8; newline-terminated statements.
- Comments: `//` to end of line. Outside strings and TeX spans, a comment may occupy a whole line or follow any statement.
- Header: `locus 3` (the v0.4 language is format version 3).
- **Compatibility:** `locus 2` denotes the v0.3 language. Its coordinate-hinted declarations and seed-selected tangency must not be silently interpreted using format-3 semantics. Migration requires rewriting the intended relationships and changing the header together. This spec version does not imply a CLI/package release.
- The file stores the **relational program**, never resolved coordinates. Resolved geometry is always derived by solving + evaluation.
- Machine interchange: `locus fmt --json` emits the parsed AST as JSON (§9.4). The DSL is the authoring format; JSON is derived, never authored.

### 1.2 File shape

```locus
locus 3                      // format version header, mandatory, first statement

meta {
  title "Externally tangent circles"
}

style textbook               // optional; defaults to textbook
class faint { stroke: dash(5, 3), weight: 0.75, accent: #6B7280 }   // optional style classes (§8.3)

// ---- objects ----
circle c1 = circle(2)
circle c2 = circle(3) where { horizontal(c1.center, c2.center) }   [highlight]
point O_1 = c1.center
point O_2 = c2.center
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

op            := free_num_op | object_op
free_num_op   := "num" ident "=" "free" scalar_hint [where] [attr] [props] eol
object_op     := kind ident "=" rhs [where] [attr] [props] eol
rhs           := expr                              -- result type must match kind
kind        := "point" | "num" | "line" | "ray" | "segment" | "circle" | "arc" | "ellipse" | "bspline"
             | "polygon" | "rect" | "diamond"
             | "label" | "arrow" | "tick" | "right_angle" | "text_block"
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
- **`free` is reserved for numeric declarations.** `num` uses a scalar hint; a missing hint or a coordinate-pair hint is a local parse error. `point P = free ...` is invalid. There is no location type, absolute point constructor, coordinate-hint syntax, or `pin` relation.
- **Points and object references are values.** `point O = c.center` names a derived anchor and introduces no unknowns. A reference such as `circle other = c` aliases the same geometry; it does not allocate another placement.
- **`point()` takes no arguments.** `point P = point()` declares an independently placed point; `point(1)`, `point(1, 2)`, and `point(A)` are local arity errors. Use `point Q = P` to name the same point, or a derived constructor to construct another. Neither declaration syntax nor point props accept coordinates, seeds, or a location field.
- **Placement-owning constructors occur only as the complete right-hand side of a matching declaration.** `circle c = circle(2)` is valid; `label(circle(2).center, "O")` is invalid. Declare the object first. This gives each placement a stable source id and prevents hidden unknowns in nested expressions (§2.2).
- **Identifiers**: unique per file, `[A-Za-z][A-Za-z0-9_']*`. Convention (not enforced): single/primed letters for points (`A`, `B'`, `M_1`), words for derived objects (`tri`, `inc`).
- **Reference order**: an op's `rhs` may only reference ids defined on earlier lines; its `where` clause may also reference the id being declared. `rel` and `assert` statements may reference any id defined above them; `rel` equations constrain *earlier* objects (the solve is simultaneous, §7).
- **`[attr]`** — a style class: one of the built-ins `main` (default) | `auxiliary` | `highlight` | `construction`, or a `class` declared on an earlier line (§8.3). A class may use preset tokens or exact color, width, dash, opacity, cap, and join values.
- **`{props}`** — op-specific fields (closed set per kind, §5.7). Unknown props are errors.
- **Text strings may mix plain text and TeX.** Outside a TeX span, quoted strings support `\"`, `\\`, `\$`, and `\n`; `\n` produces a centered multi-line text stack. An unescaped `$` opens or closes an inline TeX span, so `"filter offset $O_2$"` is stored as three ordered segments: plain text, TeX, plain text. Use `\$` for a literal dollar sign in the plain-text portion. An unmatched delimiter is a local syntax error.
- **TeX spans preserve backslashes.** Inside either an inline span or a bare all-TeX literal, TeX source is not processed as string escapes: `"angle $\alpha$"` and `$\triangle ABC$` both deliver the backslash to the TeX engine. Raw newlines and nested unescaped dollar delimiters are invalid. `\$` inside TeX remains TeX source for a rendered dollar and does not close the span.
- **Text is represented as segments.** The AST stores every text value as an ordered list of `{kind: "plain" | "tex", source: string}` segments. A bare `$...$` value is therefore one TeX segment, a string without inline spans is one plain segment, and a mixed string has multiple segments. Formatting preserves segment boundaries and escapes canonically.

---

## Part 2 — The object model: placed and derived

Geometry is either **placed** (owns an unknown placement) or **derived** (computed from other objects). Numeric parameters may also be unknown. A parameterless `point()` owns its placement just as `circle(r)` owns its center. A point obtained from an anchor or construction is derived. Placement remains internal in both cases.

### 2.1 Derived tier (direct, zero unknowns)

A derived object is a deterministic `f64` function of previously defined objects (`midpoint(A, B)`, `incircle(A, B, C)`, `foot(P, l)`, …). It introduces no unknowns and has no iterative convergence of its own. Derived definitions are evaluated at every solver trial that needs them and once more for the final scene; a violated construction precondition is a construction error (§7.5). “Direct” means evaluation without an additional solve, not symbolic or infinite-precision arithmetic. **Construct when you can, relate when you must.**

### 2.2 Placed geometry and unknown parameters

The following declarations introduce unknowns:

```locus
point P = point()               // 2 internal placement DOFs; no constructor arguments
circle c = circle(2)            // 2 internal placement DOFs; radius is exactly 2
ellipse e = ellipse(3, 2)        // 2 internal placement DOFs; semiaxes 3 and 2
rect box = rect(4, 2)           // 2 internal placement DOFs; width 4, height 2
diamond decision = diamond(3, 2) // 2 internal placement DOFs; width 3, height 2
num r = free ~1.5               // 1 DOF: a numeric parameter, with a scalar seed
```

`circle(r)`, `ellipse(a, b [, theta])`, `rect(w, h)`, and `diamond(w, h)` each own a center that Locus solves internally. Rectangles and diamonds are axis-aligned; a circle has no orientation parameter. An ellipse's rotation is an angle expression, defaulting to `0deg`, and introduces no implicit orientation unknown. Dimensions are expressions of earlier values, not implicitly free parameters. `circle(r)` with a free numeric `r` therefore depends on three unknown components in total: its two placement components and the separately declared radius parameter.

Anchors such as `c.center`, `box.n`, and `decision.e` expose geometric relationships without exposing coordinates. Naming an anchor does not allocate unknowns, and constraining it moves its owning geometry (or the upstream objects and parameters on which it depends). `point()` introduces two internal unknowns owned by its declaration; relations constrain them directly. The old `point P = free ...` syntax remains invalid, and there is no independently placed line constructor. Lines, rays, segments, polygons, arcs, B-splines, and annotations are derived from existing geometry. A B-spline's controls may be declared points, anchors, or constructed points; the spline allocates no hidden control points or placement (§5.11).

Explicit-center forms such as `circle(O, r)`, `ellipse(O, a, b [, theta])`, and `rect(C, w, h)` remain derived constructors: their center argument must already be a point value, whether declared with `point()` or derived. They allocate no placement of their own. This permits concentric shapes and constructions at geometric centers without introducing authored coordinates.

The unknown vector concatenates the two internal components of each declared `point()`, the two center components of each placed shape, and the single value of each free `num`, in declaration order. Derived objects and aliases contribute no components. Two `point()` declarations own distinct placements even if relations later make them coincident; `point Q = P` shares P's placement and adds no unknowns.

`num` also has a derived form — a named constant or computed scalar:

```
num fw  = 4
num phi = (1 + sqrt(5)) / 2
```

### 2.3 Placement and solution choices

- **Locus initializes placement.** No source syntax or public CLI field accepts a point or center seed. The engine supplies a deterministic initial placement and settles unconstrained placement internally (§7). A standalone `point P = point()` or `circle c = circle(2)` is a complete valid figure after the required header.
- **Relations express intent.** Use anchors, alignment, distances, and tangency to relate shapes. Relative measurements such as `hdist(a.center, b.center) == 5` remain part of the language; they express separation between objects, not absolute locations.
- **Discrete choices are geometric.** `tangent(c1, c2)` means external tangency; `internally_tangent(outer, inner)` explicitly requests internal tangency. Multi-valued derived constructions use the documented branch selector (§5). Source choices must never be inferred from hidden center seeds.
- **Numeric hints remain numeric.** `num r = free ~1.5` supplies a solver seed for a dimension, angle, ratio, or other numeric parameter. It is never a fixed value or an absolute coordinate. Relations determine the solved value; unresolved numeric freedom settles toward this seed. To fix a radius, write `circle(2)` or pass a constant `num`.
- **Remaining freedoms are reported in terms of objects.** The engine distinguishes harmless rigid movement of a whole figure from unresolved relationships or dimensions (§7.2). Feedback names the affected objects and suggests geometric relations; it never asks an author to move a coordinate hint.

Assertions verify the chosen solution; they do not steer placement or select another solution. If a particular ordering or configuration matters, encode it with defining relations or an explicit construction rather than relying on engine placement.

### 2.4 `where` sugar

Relations may be attached at the declaration site; this desugars to standalone `rel` statements immediately following the op:

```locus
circle c2 = circle(3) where { horizontal(c1.center, c2.center), tangent(c1, c2) }
// ≡
circle c2 = circle(3)
rel horizontal(c1.center, c2.center)
rel tangent(c1, c2)
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
| `on(P, e)` | ellipse: `b × (sqrt((u/a)² + (v/b)²) − 1)`, with local components `u, v` from §5.10; a length-valued implicit residual, not Euclidean distance | 1 |
| `tangent(c, l)` | circle tangent to a line/ray/segment; closest distance from center to the closed linear object minus radius | 1 |
| `tangent(c1, c2)` | external circle tangency; center distance minus `r1 + r2` | 1 |
| `internally_tangent(outer, inner)` | internal circle tangency; center distance minus `r_outer − r_inner`, requiring `r_outer > r_inner` | 1 |
| `parallel(l1, l2)` | parallel linear objects; normalized 2D cross product of their direction vectors | 1 |
| `perpendicular(l1, l2)` | perpendicular linear objects; normalized dot product of their direction vectors | 1 |
| `collinear(A, B, C, ...)` | points after the first two on their line; one signed perpendicular distance for each remaining point | n−2 |
| `concyclic(A, B, C, D, ...)` | points after the first three on their circumcircle; one radial difference for each remaining point | n−3 |
| `congruent(s1, s2)` | equal segment lengths; `length(s1) − length(s2)` | 1 |
| `coincident(P, Q)` | same point; `x(P) − x(Q)` and `y(P) − y(Q)` | 2 |
| `midpoint(M, A, B)` | relational form of the constructor; component differences from `(A+B)/2` | 2 |
| `horizontal(P, Q)` / `vertical(P, Q)` | `y(Q) − y(P)` / `x(Q) − x(P)` | 1 |

Here a **linear object** is a line, ray, or segment. Its direction is the ordered vector from its defining first point to its second point; a zero direction is degenerate. `parallel` and `perpendicular` accept any pair of linear objects. A ray, segment, and arc are closed sets for `on`, so their endpoints count as on the object. Closest-point distance is the ordinary Euclidean distance to that closed set. For `tangent(c, ray|segment)`, endpoint-only contact counts as tangency of the two closed rendered objects; callers that require tangency to the supporting line can additionally assert perpendicularity to the radius.

Circle-circle tangency is selected by the predicate, never by initialization. `tangent(c1, c2)` is symmetric and always external. `internally_tangent(outer, inner)` is ordered: its first circle contains the second, and equal or reversed radii are invalid. The same meaning and domain apply in `rel` and `assert`.

All raw residuals are normalized by §7.4. `collinear` requires its first two points to be distinct. `concyclic` requires its first three points to be distinct and non-collinear. Direction normalization in `parallel`/`perpendicular` divides by both direction lengths. Violating one of these preconditions produces a source-anchored degenerate-geometry error instead of an arbitrary truth value.

Signed axis offsets are expressed through the numeric functions `hdist`/`vdist` (§6.2) under `==`: `rel vdist(b.n, a.s) == 0.8`.

There are **no vocabulary aliases**: exactly one name per concept. (A `touch` alias for `tangent` was considered and rejected — synonyms multiply the surface an agent must recall and the surface `describe` must print.) Naming an existing geometric value (§1.4) does not add a synonym to the language vocabulary.

### 3.3 Assert-only predicates

Inherently inequality-shaped conditions cannot be solved for in v0.4 and are **assert-only**: `between(A, B, C)`, `inside(P, poly)`, `inside(P, c)`, and the comparisons `<  >  <=  >=`. Boolean `or` and `not` are also assert-only because they do not define a single smooth equation set. A `rel` or `where` clause may contain a solvable atomic equality/predicate or an `and` conjunction of them; using an assert-only form there is a semantic error. Ordering, containment, and other discrete choices are established by geometric relations or explicit constructions, then *verified* by asserts.

`between(A, B, C)` is strict: it requires `B` to be collinear with distinct `A` and `C`, and the projection parameter of `B` on directed segment `A→C` to lie inside the tolerance-shrunk interval `(ε, 1−ε)`. Endpoints are not between. `inside(P, c)` is also strict and requires `(radius(c) − dist(center(c), P)) / D > ε`; a boundary point is not inside. `inside(P, poly)` requires a valid simple polygon, first rejects points within tolerance of any closed edge, then uses the nonzero winding rule; boundary points are not inside. These definitions use the assert's `ε` from §7.4.

`inside(P, e)` for an ellipse requires the negative of its `on` residual divided by `D` to exceed `ε`; its boundary is excluded. B-splines are open curves and do not accept `inside`. General `on(P, spline)` is not supported in this version: to constrain a point to a selected curve parameter, use `coincident(P, spline.at(t))`. A free numeric `t` may be solved within the domain `[0, 1]`; it does not create a free geometric point.

---

## Part 4 — Direction, measurement, and internal coordinates

- Internal Cartesian scene space is **y-up**. The renderer flips to screen space; scene coordinates never denote pixels. The origin and absolute locations are engine details, absent from the authoring language and public `describe` / `inspect` output. Viewport is auto-fitted (§8.4).
- Horizontal, vertical, compass directions, and angles remain available to express relative geometry and annotation intent. Removing authored locations does not remove relative placement.
- `hdist(P, Q) = x(Q) − x(P)`; `vdist(P, Q) = y(Q) − y(P)` (signed).
- Angles are CCW-positive. The unit suffix is **mandatory** on angle literals: `60deg`, `1.2rad`. A bare number in an angle position is a parse error (this kills the degrees/radians hallucination class).
- Scene units are arbitrary; figures state their own scale conventions in `meta` if useful (e.g. "1 unit = 32 ms").

---

## Part 5 — Object kinds and constructors

### 5.1 `point`

```locus
point P = point()
point Q = point() where { horizontal(P, Q), dist(P, Q) == 4 }
point M = midpoint(P, Q)
```

`point()` has exactly zero arguments and introduces two internal placement DOFs. Each
declaration creates a distinct geometric point, not a point fixed at the origin. Its
position is determined by relations and Locus's deterministic defaults. It supports the
usual point marks and styles (§5.7), and may be used anywhere a point value is accepted,
including as a shape's center or a B-spline control. A `where` clause adds relationships
without adding constructor parameters. In the example, `horizontal` does not specify
which point is to the right; use `hdist(P, Q) == 4` when that ordering matters.

Like other placement-owning constructors, `point()` is declaration-only: nested calls
such as `line(point(), point())` are invalid. Declare and name the points first. The
following constructors and anchor expressions remain derived and introduce no unknowns:

```
midpoint(A, B)
lerp(A, B, t)                   -- t=0 → A, t=1 → B; t may lie outside [0,1]
offset(P, dx, dy)               -- derived displacement relative to P; introduces no unknowns
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
center(c)                       -- center of circle/arc/ellipse
```

An independently declared point may instead be constrained by relations, for example `point T = point() where { on(T, c1), on(T, c2) }`. Such relations do not distinguish multiple intersections; use an explicit construction branch when that distinction matters. `intersect` omits `k` only when the intersection is unique; when two branches exist and `k` is omitted, that is an error (never a default). Branch ordering for explicit `k`: sorted by the parameter of `o1` — along its direction for a line/ray/segment, by CCW angle from +x for a circle. `locus describe` reports the selected index, defining objects, and distinguishing relative measurements. A non-integer or out-of-range branch is a type error.

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

The contact point is derived as `foot(c.center, tangent_line(P, c, k))`; no independently placed point is needed.

### 5.4 `circle`, `arc`

```
circle(r)                           -- placed shape: radius r, center owned by Locus
circle(O, r)                        -- derived circle at an existing point or anchor
circle_through(O, A)
circle_diameter(A, B)
circumcircle(A, B, C)
incircle(A, B, C)
arc(c, P, Q, ccw)                   -- ccw: true|false; P, Q must lie on c (checked)
angle_arc(A, B, C, r)               -- angle-mark arc at vertex B, radius r
```

In either `circle` form, `r` is a length expression, for example `2`, `dist(A, B) / 2`, or a free `num`. `circle(r)` is declaration-only (§1.4); `circle(O, r)` composes as a derived value. Every circle exposes `.center`, including circles whose center was never named by the author.

### 5.5 `polygon`, `rect`, `diamond`

```
polygon(A, B, C, ...)               -- ≥ 3 vertices, listed in order
rect(w, h)                         -- placed axis-aligned rectangle; width w, height h
diamond(w, h)                      -- placed axis-aligned rhombus; width w, height h
rect(C, w, h)                      -- derived rectangle at existing anchor C
diamond(C, w, h)                   -- derived rhombus at existing anchor C
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
e.center    e.at(30deg)   e.focus1   e.focus2              -- ellipse (§5.10)
e.major_start  e.major_end  e.minor_start  e.minor_end
e.semimajor  e.semiminor  e.rotation                       -- length, length, angle
curve.start  curve.end  curve.at(0.5)                     -- B-spline; scalar parameter (§5.11)
poly.centroid
r.center  r.n  r.s  r.e  r.w  r.ne  r.nw  r.se  r.sw     -- rect / diamond
r.width   r.height                                        -- numeric anchors
lbl.center              -- resolved label box center (annotation layer ONLY)
```

Geometry-tier anchors (everything above except `lbl.center`) are ordinary derived points: legal in constructors, `rel`, and `assert`. Label-box anchors resolve *after* layout (§8) and are legal only in annotations — geometry must never depend on layout.

### 5.9 Domains and degeneracy

Every parsed number, evaluated expression, hint, solved value, and resolved coordinate must be finite. Overflow, NaN, or infinity is an error at the source expression that produced it.

- Circle radii, rectangle/diamond widths and heights, and `angle_arc` radii must be strictly positive. Label gaps are nonnegative. A `right_angle` size, whether explicit or style-derived, is strictly positive.
- Ellipse semiaxes satisfy `a >= b > 0`; both are lengths. Rotation is a finite angle. Equal semiaxes are valid, with coincident foci, but the value retains its ellipse type and parameterization.
- A B-spline requires at least four finite point-valued controls, whether independently placed or derived. Repeated controls are allowed; the first and last must be distinct. Parameters are finite scalars in `[0, 1]`, without wrapping, extrapolation, or clamping of out-of-domain input. Self-intersection and vanishing derivatives are allowed for evaluation and rendering; no arbitrary tangent is supplied at a stationary point.
- A free `num` has no universal sign restriction; its uses impose constructor-specific domains. A trial vector that violates a domain is rejected as described in §7.1, while a final violation is a construction error.
- A line or ray requires distinct defining points. A zero-length segment may be stored and rendered as a point, but any operation requiring its direction—including `parallel`, `perpendicular`, `foot`, and line-like incidence—reports degeneracy.
- A polygon requires at least three distinct vertices, nonzero signed area, no crossing non-adjacent edges, and no adjacent collinear overlap; it may be clockwise or counterclockwise. This is the “simple polygon” required by `inside`.
- `arc(c, P, Q, ccw)` requires distinct endpoints on `c`. Endpoint incidence is checked with normalized residual `< 1e-12`; `ccw` selects the directed sweep from `P` to `Q`. A full circle must be represented as `circle`, not as an arc with coincident endpoints.
- `angle_arc(A, B, C, r)` and `right_angle(V, P, Q)` require two nonzero rays at their vertex. `tick(s, n)` requires a nonzero segment and an integer `n` in `1..4`.
- Division requires a nonzero denominator under the normalized `1e-12` construction threshold. `sqrt` requires a nonnegative operand; an operand within normalized `1e-12` below zero is clamped to zero, while a more negative operand is an error.

Degenerate inputs never acquire arbitrary fallback directions, centers, branches, or truth values. They produce the source-anchored diagnostics required by §7.5.

### 5.10 `ellipse`

```
ellipse(a, b)                       -- placed; semimajor a, semiminor b, rotation 0deg
ellipse(a, b, theta)                -- placed; major axis at CCW angle theta from +x
ellipse(O, a, b)                    -- derived at existing anchor O, rotation 0deg
ellipse(O, a, b, theta)             -- derived at existing anchor O, rotated
```

The two numeric dimensions are **semiaxes**, not full width and height. Overloads are
resolved by the first argument's type: numeric for a placed ellipse, point for a derived
ellipse. Placed forms are declaration-only (§1.4). A numeric parameter may control a
semiaxis or rotation; its unknown is owned by its `num` declaration.

For internal evaluation, let `U = (cos(theta), sin(theta))` and
`V = (−sin(theta), cos(theta))`. Then `e.at(phi)` is
`O + a cos(phi) U + b sin(phi) V`. Here `phi` is an angle parameter around the ellipse,
**not the polar angle of the ray from its center**, except when `a = b`.
For incidence, `u = dot(P−O, U)` and `v = dot(P−O, V)` (§3.2).

The `.major_start` / `.major_end` anchors are `O − aU` / `O + aU`;
`.minor_start` / `.minor_end` are `O − bV` / `O + bV`. With
`f = sqrt((a−b)(a+b))`, `.focus1` / `.focus2` are `O − fU` / `O + fU`.
Numeric anchors `.semimajor`, `.semiminor`, and `.rotation` return the constructor values.
`area(e)` is `πab`, with `π` the binary64 constant with bits `0x400921FB54442D18`. An ellipse has no
`.radius`, compass-box anchors, or implicit conversion to a circle.

Ellipse anchors, `on`, `inside`, and area measurements are supported. General ellipse
intersection, nearest-point projection, perimeter, ellipse arcs, and tangency overloads
are outside this version; unsupported combinations are source-anchored type errors.
In particular, existing circle-only `arc`, `project`, `tangent`, and `tangent_line`
constructors/predicates do not silently accept an ellipse.

### 5.11 `bspline`

```
bspline(P0, P1, P2, P3, ...)        -- derived open, clamped, uniform cubic B-spline
```

Control points are ordered point expressions, with at least four arguments. Named
`point()` declarations are valid controls, as are anchors and derived constructions. Their order
defines the traversal from `.start` (first control) to `.end` (last control). Interior
controls guide the curve and are **not generally interpolated**. There is no control-point
coordinate list, free-control syntax, or automatically created control polygon. Declare
segments between controls when a construction polygon should be shown.

The degree is fixed at `p = 3`; all weights are one. For `m` controls `P0 … P(m−1)`, the
knot vector has `m+4` entries: four zeros, the simple interior knots
`j/(m−3)` for `j = 1 … m−4`, then four ones. Thus four controls produce one cubic Bézier
segment; five or more produce multiple polynomial spans. Interior simple knots give
`C²` continuity. These are standard clamped B-spline conventions
([Open CASCADE reference](https://dev.opencascade.org/doc/occt-7.7.0/refman/html/class_geom2d___b_spline_curve.html)); the fixed degree and automatic knots are Locus's initial authoring contract.

`.at(t)` takes a **dimensionless scalar** in `[0, 1]`, not an angle or an arc-length
fraction. `.start` is exactly `.at(0)` and `.end` is exactly `.at(1)`. The spline has no
center, radius, area, or length measurement in this version. Numeric comparisons of
distances between sampled points remain available.

Evaluation uses de Boor's recurrence. Return the first or last control exactly at `t = 0`
or `1`. Otherwise choose the unique span `k` with `u[k] <= t < u[k+1]`, initialize
`d[j] = P[k−3+j]` for `j = 0 … 3`, and for `r = 1 … 3`, visit `j = 3 … r` in descending
order. Set `i = k−3+j`, `alpha = (t−u[i]) / (u[i+4−r]−u[i])`, and replace
`d[j]` with `(1−alpha)d[j−1] + alpha d[j]`. Return `d[3]`. Every denominator in these
active spans is positive under the specified knot vector. An exact interior knot uses
the span on its right; automatic differentiation follows the same evaluation. Although
endpoint values are returned exactly, their derivatives with respect to `t` are the
one-sided curve derivatives, not zero: `C'(0) = 3(m−3)(P1−P0)` and
`C'(1) = 3(m−3)(P(m−1)−P(m−2))`. Derivatives with respect to control points still follow
the endpoint identity. Implementations must preserve both contributions when `t` and
controls depend on solver unknowns.

Point samples and endpoint relations compose with the existing language. General
curve incidence, intersections, projection, tangency, custom degree or knots, weights
(NURBS), periodic/closed splines, fitting, and interpolation-through-points constructors
are outside this version. Unsupported operations produce type errors rather than
silently sampling the curve or treating its control polygon as the curve.

---

## Part 6 — Expression language

### 6.1 Numeric dimensions and inference

Every numeric expression has one of four dimensions: **scalar**, **length**, **area**, or **angle**. Length is measured in the file's arbitrary scene unit; area is its square. Angles are stored internally in radians, with `deg` and `rad` accepted at the source boundary. Dimensions are semantic types, not user-selectable unit systems.

- Geometry determines dimensions: `dist`, `hdist`, `vdist`, `radius`, `length`, rectangle dimensions, ellipse semiaxes, and relative point offsets are lengths; `area` is area; `angle`, `dir_angle`, ellipse rotation and ellipse `.at` arguments, and arrow `bend` are angles; B-spline `.at` arguments, counts, and ratios are scalars.
- A numeric literal without `deg` or `rad` begins as a dimension variable and is unified from context. Thus the `6` in `dist(A, B) == 6` becomes a length, while the `2` in `2 * angle(A, B, C)` is scalar. An otherwise unconstrained literal defaults to scalar.
- Each named `num`, including a free `num`, has one dimension inferred from all of its uses in the file. Conflicting uses are a type error. A free numeric hint has the inferred dimension of its declaration.
- Addition, subtraction, comparison, `min`, and `max` require matching dimensions. Multiplication or division by a scalar preserves the other operand's dimension. `length * length` produces area; `area / length` produces length; equal-dimension division produces scalar. Other dimension products are errors in format version 3.
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
| `area(e)` | ellipse area `π × e.semimajor × e.semiminor` |
| `sqrt(x)` `abs(x)` `min(a, b)` `max(a, b)` | as named |

All numeric functions are differentiable almost everywhere and legal inside `rel` equations; the solver composes through them (and through derived-object definitions).

### 6.3 Predicate summary

Relations (§3.2) work in both `rel` and `assert`. Assert-only: `between`, `inside`, `< > <= >=` (§3.3). Angle/length equality uses `==` on numeric functions rather than dedicated predicates: `angle(A, B, C) == 90deg`.

---

## Part 7 — Solve and evaluation semantics (normative)

### 7.1 The solve

The format-version-3 solve contract is defined below. The numerical iteration is specified, but the deterministic placement initializer and retry schedule must be finalized before this draft is frozen (Part 12). Implementations may optimize the frozen solver only if they produce the same resolved `f64` values and reports for every valid input; changing any frozen constant or acceptance rule requires a new format version.

1. **Unknown vector and initialization.** Concatenate each `point()` declaration's internal `(x, y)`, each placed shape's internal center `(x, y)`, and each free numeric value in declaration order (§2.2). Build the initial vector `h` from engine-generated point and center placements and authored numeric hints. Initialization must account for shape dimensions and relation dependencies, avoid coincident-point or coincident-center degeneracy where a direction is needed, and use a bounded deterministic retry schedule when an initial arrangement is invalid. It must accept a standalone `point()` or `circle(2)` declaration without any additional input. The initializer may not read asserts to choose a solution or change explicit branch semantics. Normalize unknown coordinates before linear algebra: point/center coordinates and length-valued nums by `D`, area-valued nums by `D²`, angle-valued nums by `1 rad`, and scalar nums by `max(1, abs(hᵢ))`, where `D` is the solve-time characteristic length from §7.4. The normalized vector is `z`; its normalized initial vector is `zₕ`. All point/center components and placement seeds remain internal.
2. **Residual vector.** Desugar `where` clauses in source order, flatten allowed `and` conjunctions left-to-right, and expand each relation into the component residuals specified in §3.2. Evaluate them in that order and normalize each by §7.4. Tangency type and explicit constructor branch indices come from the source and remain unchanged throughout the solve.
3. **Derived evaluation and differentiation.** For the initial vector and every trial vector, evaluate the transitive dependencies of relations and placed-shape dimensions top-to-bottom as pure functions of the current unknown values before evaluating residuals. Derived objects used only by annotations or asserts are deferred until final evaluation; for example, the intersection of two circles being solved into tangency need not exist at initialization. Compute the Jacobian `J = ∂r/∂z` with forward-mode automatic differentiation through those definitions; finite differences are not conforming. At a piecewise definition, differentiate the active piece of the current evaluation. A construction precondition failure at initialization requests the next bounded initialization attempt; a failure at a later trial gives that trial infinite objective without leaking NaN or infinity into output. Explicit branches retain their documented geometric ordering at each evaluation.
4. **Rank and null space.** Compute an SVD of `J` with the deterministic one-sided Jacobi procedure below. A singular value has rank when `σᵢ > 1e-10 × max(σ₀, 1)`. The Moore–Penrose pseudoinverse `J⁺` uses exactly those retained singular values, and the null-space projector is `N = I − J⁺J`. Sign choices for singular vectors do not affect `J⁺` or `N`.
5. **LM trial step.** Start with `λ = 1e-3`. At each attempt, compute the primary step from `(JᵀJ + λI) δₚ = −Jᵀr` using Cholesky factorization in ascending row/column order. Compute the settling component `δₕ = N(zₕ − (z + δₚ))`, then use `δ = δₚ + δₕ / (1 + λ)`. All dot products and sums execute sequentially in index order; contraction into fused multiply-add is forbidden.
6. **Acceptance schedule.** Let `F(z) = 0.5 × Σ rᵢ²`, summed in residual order. Accept a finite trial if its maximum absolute residual is below `1e-12`, or otherwise only if its `F` is strictly smaller. After acceptance set `λ = max(λ / 10, 1e-15)` and recompute values, `J`, rank, and `N`. After rejection keep `z` unchanged and set `λ = min(λ × 10, 1e15)`. An attempt, accepted or rejected, counts as one iteration. Stop successfully when `max(abs(rᵢ)) < 1e-12`; a file with no relations requires zero numerical iterations after valid initialization. End this initialization attempt after 200 numerical attempts or after a rejection at `λ = 1e15`; report solve failure only after the bounded initialization schedule is exhausted.
7. **Lexicographic settling semantics.** Relation satisfaction is the primary objective. Null-space motion toward `zₕ` is secondary and may never justify accepting a worse primary objective. Remaining freedoms settle deterministically toward engine placement defaults and numeric hints; this is not a claim of a global closest solution across disconnected branches. Engine placement does not become an authored constraint.
8. **Final evaluation and asserts.** Evaluate every derived object top-to-bottom at the accepted unknown vector, build the final scene, and then check asserts using §7.4. This final pass uses the same functions as residual evaluation and introduces no new geometry semantics. A failed assert is reported as a failed test, never used to trigger a search for a different arrangement.

All arithmetic is IEEE-754 binary64, round-to-nearest ties-to-even, with subnormals preserved. Transcendental functions come from the format-version-pinned `libm` implementation. No randomness, parallel reductions, locale-sensitive parsing/formatting, reassociation, or platform BLAS/LAPACK calls are permitted in the normative path.

For the one-sided Jacobi SVD, start with `A = J` and `V = I`. A sweep visits every column pair `(p, q)` in lexicographic order. Let `α = dot(Ap, Ap)`, `β = dot(Aq, Aq)`, and `γ = dot(Ap, Aq)`, with dots summed by ascending row. Skip the pair if either norm is zero or `abs(γ) <= 2^-48 × sqrt(αβ)`. Otherwise set `ζ = (β−α)/(2γ)`, `t = 1` when `ζ = 0`, and otherwise `t = sign(ζ)/(abs(ζ)+sqrt(1+ζ²))`; then `c = 1/sqrt(1+t²)` and `s = c×t`. Replace the columns with `(c×Ap−s×Aq, s×Ap+c×Aq)` and apply the same rotation to `V`. Stop after the first sweep that performs no rotations, or fail with numerical non-convergence after 64 sweeps. Singular values are the final column norms; sort them descending with the original column index as the stable tie-break, and derive `U` by normalizing nonzero columns. This procedure, including its skip threshold and ordering, is part of format version 3.

### 7.2 DOF accounting

`check` and `describe` always report unknown count, equation count, rank, global remaining DOFs (`unknowns − rank`), initialization attempts, numerical attempts, accepted steps, maximum normalized residual, and final damping. Counts may expose how much freedom remains, but never the internal coordinate vector.

For each independently placed point, placed shape, or free numeric parameter, take the rows of the final null-space projector `N` (§7.1) belonging to that object's components. The rank of that row block, using the same singular-value threshold as §7.1, is its **local remaining DOF count**. Zero means fully constrained; a positive value means that many independent global null directions can move the object or change its parameter. Local counts intentionally need not sum to the global remaining count because one global freedom, such as translation, can move several objects. A derived anchor contributes no independent unknowns and is attributed to its upstream owners.

Common translation of the whole geometric scene is always harmless: no language construct refers to an absolute origin. Its two independent directions, when placed points or shapes exist, are included in raw DOF counts but excluded from actionable under-constraint diagnostics and `--strict` failures. A standalone point or fixed-radius circle therefore has two raw placement DOFs, zero unresolved relative DOFs, and passes `check --strict`. Relative movement between disconnected objects is still unresolved; it is not exempted as common translation. Additional rigid symmetries, such as a common rotation in a figure without directional intent, need the classification rules identified in Part 12 before receiving the same exemption.

To compute unresolved relative DOFs, remove the span of the two common-translation vectors from the final null space: each vector moves the matching component of every placed point and every placed shape's center equally and leaves numeric parameters unchanged. Apply the same rank threshold as above. Per-object actionable counts use row blocks of this reduced projector. Reports distinguish these counts from raw local counts; they must not claim that an anchor is independently free.

For a free `num`, report its authored hint and solved value. Emit `hint_far` only when its absolute normalized change exceeds `0.25`, including the threshold and actual change. Never report point or center seed displacement or emit placement-based `hint_far`; internal initialization is not user intent.

### 7.3 Failure taxonomy

| Condition | Detection | Exit | Report |
|---|---|---|---|
| Under-constrained | unresolved relative DOFs remain after removing common translation (§7.2) | 0 + info; `--strict`: 1 + warning | affected points, shapes, or numeric parameters, with suggested geometric relations |
| Over-constrained, consistent | redundant rows, residuals converge | 0 + warning; `--strict`: 1 | the redundant relation(s) by source line |
| Over-constrained, conflicting | inconsistent defining facts established by analysis; otherwise report non-convergence | 2 | best-fit residual per relation, worst first; minimal infeasible subset when isolable |
| Non-convergence / initialization failure | bounded initialization attempts exhausted without a valid converged solve | 2 | affected object ids, construction preconditions or final relation residuals; suggestions involving relations, dimensions, or explicit branches |

Generic-position figures may intentionally leave relative freedoms, hence "info" rather than "warning" by default. Harmless common translation is solver bookkeeping, not a request for additional author input. A failed bounded solve does not by itself prove that the relations are inconsistent.

`--strict` is a `check` policy, not a different solver: it promotes under-constraint information to warnings and returns exit 1 if any warning remains, including a consistent redundant relation. Without `--strict`, warnings are reported but do not change a successful exit code.

```
figure.locus  solve: 4 unknowns, 1 equation, rank 1
  placement: 2 common-translation DOFs handled by Locus
  info[I310]: c1 and c2 have 1 unresolved relative DOF
     12 | circle c2 = circle(3)
        = add horizontal(c1.center, c2.center) if alignment is intended
  solved: max scaled residual 3.1e-13 in 6 numerical iterations
```

### 7.4 Tolerance

All tolerances operate on a **normalized, dimensionless residual**. The default assert/incidence threshold is `ε = 1e-9`. `tol t` replaces `ε` for every atomic comparison or predicate inside that assert; `t` must be a finite positive scalar literal. It is a normalized factor, never an absolute scene-unit distance.

The characteristic length `D` is computed deterministically. During each initialization attempt it is the bounding-box diameter of all placed points and shapes and the geometry in the dependency closure needed for relations and shape dimensions, evaluated at that attempt's initial vector; it stays fixed for that numerical solve. Deferred constructions (§7.1) do not participate in this initial bound. During assertion and incidence checking it is the geometry-only solved bounding-box diameter of the complete scene. Circle/arc extents and polygon/rect/diamond vertices participate whether or not their marks are visible. Ellipses use their exact rotated bounds; B-splines use the conservative bounding box of their control points, independent of render tessellation (§8.5). An unbounded line or ray contributes only its finite defining points, not an infinite carrier; annotations do not participate. If that diameter is zero, use the largest finite positive length-valued numeric hint, literal, or constructor argument in declaration order; if none exists, use `D = 1`.

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

Identical file bytes → identical solve → identical internal coordinates → byte-identical SVG. All tie-breaks (initialization and retry schedule, LM schedule, branch ordering, label candidate selection, float formatting) must be frozen without randomness. The outstanding initialization rules in Part 12 are therefore a prerequisite for format-version-3 conformance. `locus-core` must use a pinned math library (`libm` crate, not system libm) so this holds across platforms. This enables golden-file snapshot testing in CI, including solve-heavy fixtures on every platform in the matrix. Rendered formats necessarily contain coordinates; those are generated output, not an authoring or agent-feedback interface.

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

### 8.5 Ellipse and B-spline rendering

An ellipse is closed stroked geometry and accepts fill. Its rotated bounding-box half
extents are `sqrt((a cos(theta))² + (b sin(theta))²)` horizontally and
`sqrt((a sin(theta))² + (b cos(theta))²)` vertically. SVG uses a native ellipse with its
rotation transform; PNG follows SVG, and TikZ uses the equivalent rotated ellipse.

A B-spline is open stroked geometry; `fill` has no effect. Convert each nonzero knot span
`[u, v]` to an exact cubic Bézier representation with controls `C(u)`,
`C(u) + (v−u)C'(u)/3`, `C(v) − (v−u)C'(v)/3`, and `C(v)`, in increasing span order.
Here “exact” means the same polynomial curve, evaluated with the pinned `f64` arithmetic,
not an interpolating approximation. SVG and TikZ emit these cubic segments; PNG follows
SVG. The control-point bounding box is a conservative viewport bound, not extra visible
geometry. Control points are only marked when separately declared with a visible mark.

Incidence and measurements always use the analytic definitions, not rendered polylines.
The fixed derivative evaluation, conversion operation order, and curve-aware label
collision procedure must be frozen with the curve implementation (Part 12).

---

## Part 9 — CLI

### 9.1 Commands

| Command | Purpose |
|---|---|
| `locus init <file> [--style textbook]` | create a new scene file |
| `locus add <file>` | append statements from stdin; whole file re-validated |
| `locus update <file> --id X --def "<rhs>"` / `--set 'dir=NE'` | surgical edit; re-solve |
| `locus remove <file> --id X` | remove; orphaned dependents → exit 2 |
| `locus describe <file>` | structural dump: objects, defs, anchor ownership, relative geometry, solve summary, DOF report, numeric-hint changes, branch resolutions |
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
- `describe` and `inspect`, including their JSON forms, expose object relationships, dimensions, anchor identities, relative measurements, and branch choices. They omit absolute positions, center seeds, coordinate arrays, and any `location` field. Rendering may use internal coordinates (§7.6); agents do not need them to author or repair a figure.
- `inspect --json` reports: measurement tables (pairwise distances among named points, triangle angles, ratios along shared lines), detected incidences ("`inc` tangent to `seg(A,B)`", "`M` on `AB` with `AM:MB = 1:1`"), and diagnostics (`degenerate`, `hint_far`, `label_collision`, `out_of_view`, `crowding`, `unreferenced`).
- On assert failure, the report includes the actual value of **every measurable subterm** so the agent can localize which upstream definition is wrong.
- Ellipse reports include semiaxes, rotation, anchor identities, and area. B-spline reports include the ordered control expressions, degree `3`, derived knot vector, span count, and endpoint identities. Neither report exposes internal control coordinates. Detected incidence for ellipses uses §3.2; B-spline incidence is not inferred from samples or its control polygon.

### 9.4 JSON interchange

`locus fmt --json` emits the AST: version, segmented meta text, style preset, declared classes, ordered ops (`{id, kind, def, where, attr, props}` with `def` as both source string and parsed tree), rels, and asserts. A free numeric op additionally carries its scalar `hint`; geometry ops never carry a hint or location field. Placement ownership follows from the constructor overload; implicit centers are not synthesized as free-point ops. Numeric expression nodes include their inferred dimension (§6.1); text nodes contain the ordered plain/TeX segments from §1.4. This is the integration surface for future MCP shims, editors, or watch-mode preview servers — all of which wrap the CLI, never reimplement it.

### 9.5 Intended agent loop

```bash
locus init fig.locus
locus add fig.locus < ops.locus
locus check fig.locus --json    # → solve summary + DOF report + assert results
# wrong branch? → change the explicit construction or tangency predicate.
# unresolved relative DOF? → add a relation. conflict? → revise the defining facts.
locus render fig.locus -o fig.svg
```

Recommended agent policy: **never `render` until `check` exits 0.**

---

## Part 10 — Out of scope for v0.4 (reserved)

- Inequality relations in `rel` (only asserts may compare with `< > <= >=`).
- Content-driven box sizing (rect width from measured label extents) and arrow routing (elbows, box clipping) — the rest of "diagram mode."
- Location values, the old hinted free-point syntax, absolute coordinate hints, and `pin` are excluded from the authoring model, not reserved for later addition. Independent placements beyond points, circles, ellipses, rectangles, and diamonds are out of scope for this version (§2.2). B-splines derive all controls from existing geometry.
- General ellipse/B-spline intersections, curve tangency and projection; ellipse arcs and perimeter; variable-degree, custom-knot, rational, periodic, closed, or fitted B-splines (§5.10–§5.11).
- User-defined presets or named palettes; per-op inline style declarations; whole-figure transforms; layers; animation; 3D. Exact values remain reusable through classes (§8.3).
- Statement continuations or other multi-line statements beyond the structured `meta` block; includes/imports; user-defined functions.

The `locus <n>` header exists so these can arrive without breaking earlier files.

---

## Part 11 — Complete example

```locus
locus 3

meta {
  title "Externally tangent circles with a common external tangent"
}

style textbook

// ---- objects ----------------------------------------------
num r_1 = 2
num r_2 = 3

circle c1 = circle(r_1)
circle c2 = circle(r_2) where { horizontal(c1.center, c2.center) }
point O_1 = c1.center
point O_2 = c2.center

rel tangent(c1, c2)                     // explicitly external tangency

// contact point divides the center segment in the ratio of the radii
point T = lerp(O_1, O_2, r_1 / (r_1 + r_2))

// external homothety center — direct, so constructed
point E = lerp(O_1, O_2, r_1 / (r_1 - r_2))

// common external tangent: through E, with an explicit construction branch
line t_ext = tangent_line(E, c1, 0)   [highlight]
point P_1 = foot(O_1, t_ext)
point P_2 = foot(O_2, t_ext)

// ---- annotation -------------------------------------------
label l1 = label(O_1, $O_1$)  { dir: S }
label l2 = label(O_2, $O_2$)  { dir: S }
label lT = label(T, $T$)      { dir: SE }

// ---- intent -----------------------------------------------
assert dist(O_1, O_2) == r_1 + r_2
assert on(T, c1)
assert on(T, c2)
assert between(O_1, T, O_2)
assert tangent(c2, t_ext)               // touches BOTH circles — the theorem
assert collinear(E, P_1, P_2)
```

The agent states radii and a tangency relation, then derives anchors, the contact point, and a common tangent. The assertions verify their consequences. No standalone location, free point, or coordinate seed appears in the file.

---

## Part 12 — Open questions before freezing

1. **Deterministic placement initialization** — specify and validate the exact dimension-aware initial arrangements, construction-domain handling, bounded retry schedule, and tie-breaks between otherwise valid arrangements. Cover standalone points and shapes, points related into nondegenerate triangles and spline controls, tangent and concentric circles, relation-dependent intersections, and disconnected diagrams. Authors must never repair initialization by supplying coordinates. This algorithm is required before freezing format version 3; the iteration rules alone no longer define a complete reference solver.
2. **Rigid symmetry classification** — common translation is handled by §7.2. Define when common rotation is also presentation-only, accounting for horizontal/vertical relations, compass anchors, relative offsets, and angle-based constructions, before exempting it from strict checks.
3. **Solver constants** — empirically validate the specified normalization, rank threshold, damping schedule, and `1e-12`/`1e-9` gap on extreme-scale figures before freezing format version 3; any change after freezing requires a new format version.
4. **Implementation migration** — update parser/AST, constructor typing, unknown ownership, solver initialization, CLI/JSON reports, and fixtures together. Existing coordinate hints are not silently discarded: translate their intended relative geometry into relations and explicit construction branches.
5. **KaTeX subset boundary** — which TeX commands are guaranteed? Needs an explicit whitelist appendix before v1.
6. **Curve implementation** — add ellipse/B-spline syntax and typed overloads, parameter domains, analytic evaluation and differentiation (including B-spline endpoint derivatives), rotated/conservative bounds, exact cubic span conversion, and curve-aware layout. Pin conversion operation order and collision tolerances; verify endpoint, interior-knot, repeated-control, and rotated-ellipse cases across renderers. The curve demonstrations are draft acceptance cases, not claims of existing runtime support.

### Decided (previously open)

- **Object-owned placement** — `point()` declarations, circles, ellipses, rectangles, and diamonds introduce internal placement; anchors and constructive points remain derived (§2.2).
- **Ellipses** — positive ordered semiaxes, optional explicit rotation, derived anchors, incidence, containment, and area (§5.10).
- **B-splines** — derived open clamped uniform cubic curves, with ordered existing control points and scalar parameter sampling (§5.11).
- **No authored locations** — hinted free-point declarations, coordinate hints, and `pin` are removed. The new `point()` constructor takes no arguments. Public authoring and feedback expose geometry and relationships, not absolute coordinates (§1.4, §9.3).
- **Explicit tangency semantics** — `tangent(c1, c2)` is external; `internally_tangent(outer, inner)` is internal, independent of initialization (§3.2).
- Unresolved relative DOFs report as **info** by default; `--strict` promotes. Common translation alone never fails strict checking (§7.2–§7.3).
- **No vocabulary aliases** — `touch` removed; exactly one name per concept (§3.2).
- **Numeric `free` only** — scalar hints remain for unknown numeric parameters; points and lines cannot use `free` (§2.2–§2.3).
- **Style classes introduced** — reusable classes with a closed property set (§8.3); resolves F13.
- **Exact class styles supported** — HEX colors, numeric line weights, custom dash arrays, opacity, caps, and joins coexist with preset tokens (§8.3).
- **Mixed text and TeX supported** — quoted strings may contain measured inline `$...$` segments while bare `$...$` remains the all-TeX shorthand (§1.4, §8.2).
