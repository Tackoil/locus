# Locus 0 beta 6 authoring reference

## Syntax and geometry

```locus
locus 0 beta 6
style textbook

point A()
point B() where { align_center_y(A, B), hdist(A, B) == 4 }
point C() where { align_center_x(A, C), vdist(A, C) == 3 }
polygon tri(A, B, C)
point M = midpoint(B, C)
circle circum(M, dist(M, A)) [construction]
right_angle corner(A, B, C)
label lA(A, "A") { dir: SW }
label lB(B, "B") { dir: SE }
label lC(C, "C") { dir: NW }

assert dist(B, C) == 5
assert area(tri) == 6
assert on(A, circum) and on(B, circum) and on(C, circum)
```

Use one statement per line and a final newline. Comments use `//` outside text;
metadata uses one `meta title = "Title"` statement per line. Identifiers match
`[A-Za-z][A-Za-z0-9_']*` and are unique within their scope. References must be
defined earlier; a declaration's `where` can also reference that declaration.
Suffix order is `where { facts }`, `[class]`, then `{ properties }`.

`kind id(args)` calls that kind's constructor; `segment` calls `seg`.
`kind id = rhs` requires a matching result type and is for an alias,
expression, or differently named constructor. For example, use
`circle c(2)` and `circle inc = incircle(A, B, C)`.
Placement-owning constructors must be declared and named, never nested:
`point P()` is valid, `line(point(), point())` is invalid.

| Declaration | Placement/dimensions |
| --- | --- |
| `point P()` | Independent point; zero arguments |
| `circle c(2)` | Independent center; fixed positive radius |
| `ellipse e(3, 2, 30deg)` | Independent center; fixed semiaxes/rotation |
| `rect box(4, 2)` / `diamond box(4, 2)` | Independent center; fixed dimensions |
| `rounded_rect box(4, 2, 0.3)` | Independent center; fixed dimensions/radius |
| `rect box()` / `diamond box()` / `rounded_rect box()` | Automatic positive dimensions and center |
| `circle c(O, 2)` / `rect box(O, 4, 2)` | Derived center from existing point O |
| `point O = c.center` | Alias of a derived anchor; no extra unknowns |
| `num r = free 1.5` | One numeric unknown with scalar seed |
| `num r = 1.5` | Fixed numeric value |

Explicit-center ellipse, diamond, and rounded-rectangle overloads also exist.
Automatic boxes fit measured content only when required by `contain`/`cover`;
centering alone does not make a caption fit. Fixed dimensions are never resized.
Scene space is y-up. `hdist(P, Q)` is signed x displacement P→Q; `vdist(P, Q)`
is signed y displacement P→Q. Angles require `deg` or `rad`.

## Derived constructions

Prefer a construction over a new point constrained to equal that construction.
For example, `point Q = box.s` and `point F = foot(P, guide)` add no unknowns.
Independent points remain appropriate for points that must slide on guides.

| Result | Expressions |
| --- | --- |
| point | `midpoint(A, B)`, `lerp(A, B, t)`, `centroid(A, B, C, ...)` |
| point | `intersect(o1, o2 [, k])`, `foot(P, linear)`, `project(P, circle)` |
| point | `rotate(P, O, theta)`, `reflect(P, axis_or_point)`, `translate(P, from, to)` |
| point | `on_circle(c, theta)`, `extend(A, B, d)` |
| point | `circumcenter(A, B, C)`, `incenter(A, B, C)`, `orthocenter(A, B, C)` |
| line/ray/segment | `line(A, B)`, `ray(A, B)`, `seg(A, B)` |
| line | `perpendicular(P, linear)`, `parallel(P, linear)`, `perp_bisector(A, B)` |
| line | `angle_bisector(A, B, C)`, `tangent_line(P, c, k)` |
| circle | `circle_through(O, A)`, `circle_diameter(A, B)`, `circumcircle(A, B, C)`, `incircle(A, B, C)` |
| arc | `arc(c, P, Q, ccw)`, `angle_arc(A, B, C, r)` |
| polygon/bspline | `polygon(A, B, C, ...)`, `bspline(P0, P1, P2, P3, ...)` |

`intersect` requires k when multiple branches exist; a unique intersection may
omit it. Branches follow the first object's direction for linear geometry or
its CCW angle from +x for circles. `tangent_line` always requires k=0 or 1 and
a point strictly outside the circle; branches follow contact angle.
`angle_bisector` and `angle_arc` use vertex B. `right_angle(V, P, Q)` uses V.

Lines and rays need distinct points. Triangle centers need a nondegenerate
triangle. Polygons must be simple, with nonzero area. `foot` on a finite segment
or ray obeys its closed domain; use the supporting line when that is intended.
Arc endpoints must be distinct and on the circle; `ccw` is a Boolean.

Ellipse dimensions are semiaxes `a >= b > 0`, not full dimensions; `.at(phi)`
is an ellipse parameter angle, not a polar angle. General ellipse intersections,
projection, arcs, tangency, and perimeter are unsupported. B-splines are open
clamped uniform cubics with at least four controls and distinct endpoints;
interior controls need not lie on the curve. `.at(t)` uses a scalar in `[0,1]`.
General spline incidence, intersection, projection, tangency, length, and area
are unsupported; use `center(P, curve.at(t))` for a selected parameter.

## Relations and measured layout

Use equality-based facts in `rel`, `where`, and `assert`:

- `dist(A, B) == 4`, `angle(A, B, C) == 90deg`, `length(s) == length(t)`.
- `center(P, Q)` for coincidence; `align_center_y(P, Q)` for horizontal points;
  `align_center_x(P, Q)` for vertical points.
- `on(P, object)`, `parallel(l1, l2)`, `perpendicular(l1, l2)`.
- `midpoint(M, A, B)`, `collinear(A, B, C, ...)`, `concyclic(A, B, C, D, ...)`.
- `tangent(c, linear)`, `tangent(c1, c2)` (external),
  `internally_tangent(outer, inner)` (ordered, unequal radii).

Conjoin solvable facts with `and`. Numeric `< > <= >=`, `or`, `not`,
`between(A, B, C)`, and `inside(P, closed_shape)` are assert-only.
`between` requires B strictly inside segment AC; `inside` excludes the boundary.
Compare points with `center`, not `P == Q`. Assertions never select a branch.

| Layout relation | Meaning |
| --- | --- |
| `center(content, shape)` | Share measured center; no sizing or containment |
| `contain(shape, content...)` | Fit actual interior with style padding; no centering |
| `align_center_x/y(items...)` | Align the corresponding measured center component |
| `align_left/right/top/bottom(items...)` | Align named bounds; overlap remains possible |
| `gap_x(items...)` / `gap_y(items...)` | Adjacent full bounds ordered left→right / top→bottom |
| `row(items...)` / `column(items...)` | Ordered gaps plus shared vertical/horizontal center |
| `inset_left/right/top/bottom(content, container)` | Named content bound at inner container margin; other axis free |
| `cover_x/y(container, items...)` | Cover combined extents plus padding on one axis only |
| `avoid(subject, obstacles...)` | Required clearance of paths, labels, shapes, or collections |
| `trim_endpoints(paths, boundary)` | Clip visible interior path ends to boundary; `rel` only |

Gaps are automatic, not numeric arguments; use explicit measurement equalities
when the user requires an exact dimension. For equal boxes, write ordinary
`.width` and `.height` equalities. Insets alone do not ensure containment;
standalone `rel inset_left(tab, frame) { padding: none }` shares that border.
`edge(rectangle, N|E|S|W)` derives a finite boundary segment for points constrained
with `on`; rounded rectangles and diamonds do not support `edge`.

Measurements include `dist`, `hdist`, `vdist`, `angle(A,B,C)` (vertex B),
`dir_angle`, `radius`, `length`, `area`, `perimeter`, and arrow `.length`.
`sqrt`, `abs`, `min`, `max`, `ratio`, and arithmetic work on matching dimensions.
Lengths, areas, angles, and scalars are distinct; a bare number cannot be an angle.
`style_metric("short_length")` and `style_metric("narrow_width")` are positive
preset lengths for short paths and narrow overlays, without semantic ownership.

## Components and collections

The [runnable example](relational-routing.locus) defines a reusable node:

```locus
fn node(text) {
    rounded_rect shape()
    label caption(text)
    rel center(caption, shape)
    rel contain(shape, caption)
}

node input("Input")
node output("Output") [highlight]
group boxes(input.shape, output.shape)
rel row(boxes)
arrow transfer(input.shape.e, output.shape.w)
```

This fragment needs a header. Define functions before calls. Calls are named
statements with positional arguments and no return value; qualified members
such as `input.shape` expose the expanded objects. Bodies may contain earlier
function calls, groups, relations, assertions, and declarations. There are no
nested definitions, recursion, control flow, defaults, or variadic parameters.
Text, geometry, and class arguments are substituted structurally. An invocation
class is a fallback; an explicit member class takes precedence.

Instances are namespaces and collections, with combined bounds of their owned
visible members; external arguments do not become owned. A group aliases
existing objects, groups, or instances, retaining first occurrence order with
duplicates removed. Groups and instances have no geometric anchors of their
own: use `input.shape.center`, not `input.center`.
`row`/`column` expand groups into members but treat an instance as one item;
other alignment, gap, cover, and containment relations use combined bounds.
Avoidance and clipping visit applicable members. Use a group of node shapes
when captions/connectors should not affect row/column bounds.

## Anchors, annotations, and routes

Geometric anchors include segment `.start/.end/.midpoint`, circle `.center`
and `.at(30deg)`, box `.center/.n/.s/.e/.w/.ne/.nw/.se/.sw`, polygon `.centroid`,
and spline `.start/.end/.at(0.5)`. Ellipses additionally expose `.focus1/.focus2`,
`.major_start/.major_end/.minor_start/.minor_end` and numeric semiaxes/rotation.
Rounded-box diagonal anchors are outer bounding-box corners, outside rounded
arcs; `.corner_radius` is a length. Radii satisfy `0 <= r <= min(w,h)/2`.
Label-box anchors such as `.center` belong to annotation layout and cannot feed
geometric constructors/predicates. Measured label objects can feed layout directly.

| Annotation | Properties |
| --- | --- |
| `label caption("Text")` | Standalone measured text; `role: normal | title`; place with relations |
| `label caption(P, "Text")` | `dir: auto | center | N | NE | E | SE | S | SW | W | NW`, nonnegative `gap`, `role` |
| `label caption(path, "Text")` | `near: auto | start | middle | end`, `side: auto | left | right | above | below`, `role` |
| `arrow path(P, Q)` | `head: -> | <-> | -`, `route: direct | curved | orthogonal` |
| `tick marks(s, n)` | n=1..4; needs nonzero segment |
| `right_angle corner(V, P, Q)` | Positive optional `size`; vertex V |
| `text_block notes("Text")` | `place: below | above | left | right`, or `anchor` and `dir` |
| `point helper()` | `mark: dot | cross | none` |

`dir: center` centers a point-attached label; standalone labels instead use
`rel center(caption, shape)`. Properties cannot cross attachment modes.
`role: title` changes measured typography, not placement.
Path-label and automatic-route `side` is page-relative, independent of traversal.
`near` is a preferred route region, not an authored fraction.

Direct arrows may have an angle-valued `bend` (e.g. `45deg`); singular bends
near odd multiples of 90deg fail E210. Automatic routes accept `side` and
choose controls internally; never combine their `route` with `bend`.
Direct routes cannot specify arrow `side`. A loop connects distinct points on
the same guide with `route: orthogonal`; coincident endpoints are invalid.
Path labels follow the accepted visible route; `.length` remains the logical
centerline length before endpoint clipping. `trim_endpoints` preserves logical
endpoints and middle crossings. `avoid` requires clearance, so do not include
an intended touching endpoint's shape as an obstacle without accounting for it.

## Styling and text

Built-in classes: `main`, `auxiliary`, `highlight`, `construction`.
Declare custom classes before use, for example:

```locus
class guide { accent: #64748B, stroke: dashed, weight: 0.75 }
class response { accent: blue, stroke: dash(6, 3), cap: round }
class frame { accent: gray, fill: none }
```

Classes support `stroke: solid | dashed | dotted | dash(n,n,...)`,
`weight: thin | normal | bold | positive_number`, `accent: color`,
`fill: none | tint | color`, `opacity: 0..1`, `cap: butt | round | square`,
and `join: miter | round | bevel`. Numeric weights/dashes are CSS pixels;
dash lists need at least two positive lengths. Colors are `ink`, `gray`,
`blue`, `orange`, `teal`, `green`, `red`, `purple`, or unquoted `#RRGGBB` /
`#RRGGBBAA`; short hex and quoted hex are invalid.

Text can be plain (`"A"`), bare TeX (`$O_1$`), or mixed (`"angle $\alpha$"`).
Plain quoted text supports `\"`, `\\`, `\$`, and `\n` (centered multiline stack).
Inside TeX spans backslashes are preserved; do not double them as plain-string
escapes. Bare multiline text and unmatched dollar signs are invalid. The ReX
subset is not full LaTeX; E414 identifies unsupported syntax.

## Migrating older source

Use the beta 6 header and declarations above. Older `point A = point()` and
same-name constructor RHS declarations are rejected; aliases and differently
named derived constructors retain `=`. Replace old `coincident`, `horizontal`,
`vertical`, and `congruent` relations with `center`, `align_center_y`,
`align_center_x`, and numeric length equality. `offset()` is removed; diagram
layout uses measured relations, and requested axis distances use `hdist`/`vdist`
equalities. Do not revive absolute coordinates, pins, or manual route controls.
