# Locus 0 beta 5 authoring reference

Use this for common authoring tasks. For the complete evolving contract in a
Locus checkout, read `spec/specification.md`; demonstration files live under
`spec/test/demonstrations/`. Historical plans and old examples may differ from
the current draft.

## Syntax and placement

```locus
locus 0 beta 5
style textbook

point A = point()
point B = point() where { horizontal(A, B), hdist(A, B) == 4 }
point C = point() where { vertical(A, C), vdist(A, C) == 3 }
polygon tri = polygon(A, B, C)
circle circum = circumcircle(A, B, C) [construction]
right_angle corner = right_angle(A, B, C)
label lA = label(A, "A") { dir: SW }
label lB = label(B, "B") { dir: SE }
label lC = label(C, "C") { dir: NW }

assert dist(B, C) == 5
assert area(tri) == 6
assert on(A, circum) and on(B, circum) and on(C, circum)
```

Metadata uses one `meta key = "value"` statement per line.

`kind id = rhs` must declare the RHS's type. Optional suffix order is
`where { relations }`, `[style_class]`, then `{ properties }`.
Comments start with `//` outside quoted strings and TeX spans. Identifiers
match `[A-Za-z][A-Za-z0-9_']*`; references must already be defined, except that
`where` can reference its own declaration.

`point()` takes no arguments. Placed `circle(r)`, `ellipse(a, b [, theta])`,
`rect(w, h)`, and `diamond(w, h)` own their centers. These constructors must
be the complete RHS of a matching declaration; do not nest them.
Explicit-center overloads are derived: `circle(O, r)`,
`ellipse(O, a, b [, theta])`, `rect(O, w, h)`, `diamond(O, w, h)`.
Aliases such as `point O = c.center` introduce no additional placement.

`num r = free 1.5` introduces one numeric unknown with a seed, not a fixed
radius or a point coordinate. `num r = 1.5` is a constant.
Scene space is y-up; `hdist(P, Q)` and `vdist(P, Q)` are signed offsets from
P to Q. Angle literals require `deg` or `rad`, e.g. `60deg`.

## Derived geometry

| Result kind | Constructors and expressions |
| --- | --- |
| `point` | `midpoint(A, B)`, `lerp(A, B, t)`, `offset(P, dx, dy)` |
| `point` | `intersect(o1, o2 [, k])`, `foot(P, linear)`, `project(P, circle)` |
| `point` | `rotate(P, O, theta)`, `reflect(P, axis_or_point)`, `translate(P, from, to)` |
| `point` | `on_circle(c, theta)`, `extend(A, B, d)`, `centroid(A, B, C, ...)` |
| `point` | `circumcenter(A, B, C)`, `incenter(A, B, C)`, `orthocenter(A, B, C)` |
| `line`, `ray`, `segment` | `line(A, B)`, `ray(A, B)`, `seg(A, B)` respectively |
| `line` | `perpendicular(P, linear)`, `parallel(P, linear)`, `perp_bisector(A, B)` |
| `line` | `angle_bisector(A, B, C)` (vertex B), `tangent_line(P, c, k)` |
| `circle` | `circle_through(O, A)`, `circle_diameter(A, B)`, `circumcircle(A, B, C)`, `incircle(A, B, C)` |
| `arc` | `arc(c, P, Q, ccw)` with on-circle endpoints and boolean direction |
| `angle_arc` | `angle_arc(A, B, C, r)` (vertex B) |
| `polygon` | `polygon(A, B, C, ...)` with ordered vertices |
| `bspline` | `bspline(P0, P1, P2, P3, ...)` with at least four controls |

`intersect` requires `k` when there are multiple intersections. Branches are
ordered along the first object's direction for a linear object, or by CCW
angle from +x for a circle. `tangent_line` always requires `k` (0 or 1) and
a point strictly outside the circle; branches are ordered by contact angle.

Radii and box dimensions must be positive. Lines need distinct points;
triangle-center constructions require a nondegenerate triangle. Polygons must
be simple and have nonzero area. `foot` on a ray or segment obeys that object's
closed domain; use its supporting line when that is the intended construction.

Ellipses use semiaxes `a >= b > 0`, not full width and height. Their `.at(phi)`
uses an ellipse angle parameter, not the polar angle. General ellipse
intersection, projection, perimeter, arcs, and tangency are unsupported.
B-splines are open, clamped, uniform cubics; interior controls are generally
not interpolated. `.at(t)` takes a scalar in `[0, 1]`. General spline incidence,
intersection, projection, tangency, length, and area are unsupported.

## Relations, assertions, and measurements

Use in both `rel` and `assert`:

- Numeric equality: `dist(A, B) == 4` or `angle(A, B, C) == 90deg`.
- Incidence and alignment: `on(P, object)`, `horizontal(P, Q)`,
  `vertical(P, Q)`, `parallel(l1, l2)`, `perpendicular(l1, l2)`.
- Point structure: `coincident(P, Q)`, `midpoint(M, A, B)`,
  `collinear(A, B, C, ...)`, `concyclic(A, B, C, D, ...)`.
- Length and tangency: `congruent(s1, s2)`, `tangent(c, linear)`,
  `tangent(c1, c2)` (external), `internally_tangent(outer, inner)` (ordered).
- Conjoin solvable facts with `and`.

Only assertions accept `<`, `>`, `<=`, `>=`, `or`, `not`, `between(A, B, C)`,
and `inside(P, polygon_or_circle_or_ellipse)`. `between` means B is strictly
inside segment AC. `inside` excludes the boundary. Neither steers the solver.
Compare points with `coincident`, not `P == Q`.
For a selected spline parameter, use `coincident(P, curve.at(t))` instead of
`on(P, curve)`.

Measurements include `dist(P, Q)` (point-to-point), `hdist`, `vdist`,
`angle(A, B, C)` (vertex B), `dir_angle(A, B)`, `radius(c)`, `length(s)`,
`area(poly_or_ellipse)`, and `perimeter(poly)`. Numeric expressions support
`+ - * /`, `sqrt`, `abs`, `min`, `max`, and `ratio(a, b)`.
Lengths, areas, scalars, and angles are distinct dimensions. Keep equalities
and arithmetic dimensionally consistent; bare numbers never infer an angle.

## Anchors and annotations

| Object | Useful anchors |
| --- | --- |
| Segment | `.start`, `.end`, `.midpoint` |
| Circle | `.center`, `.at(30deg)` |
| Rectangle or diamond | `.center`, `.n`, `.s`, `.e`, `.w`, `.ne`, `.nw`, `.se`, `.sw`; numeric `.width`, `.height` |
| Polygon | `.centroid` (declare explicit segments for edges) |
| Ellipse | `.center`, `.at(30deg)`, `.focus1`, `.focus2`, `.major_start`, `.major_end`, `.minor_start`, `.minor_end` |
| B-spline | `.start`, `.end`, `.at(0.5)` |

```locus
rect input = rect(3, 1)
rect output = rect(3, 1) where { horizontal(input.center, output.center), hdist(input.center, output.center) == 5 }
label title_in = label(input.center, "Input") { dir: center }
label title_out = label(output.center, "Output") { dir: center }
arrow link = arrow(input.e, output.w) { head: -> }
```

This fragment needs a version header when used as a complete file.
`label(anchor, text)` accepts directions `auto`, `center`, or the eight compass
directions and a nonnegative `gap`. Use `dir: center` for node text.
`arrow(from, to)` accepts `head: -> | <-> | -` and an angle-valued `bend`.
Other annotations are `tick(segment, n)` (n from 1 to 4),
`right_angle(V, P, Q)` (vertex V), and `text_block(text)` with
`{ place: below | above | left | right }` or explicit `anchor` and `dir`.
The angle bisector and angle arc use vertex B; the right-angle marker uses
its first argument. Verify the rays before adding angle marks.

Built-in style classes are `main`, `auxiliary`, `highlight`, and `construction`.
Custom classes must be defined before use; consult specification §8.3 for
supported properties instead of borrowing CSS names.

For technical diagrams, define reusable classes before the geometry:

```locus
class life { stroke: dashed, weight: 0.75, accent: gray }
class response { stroke: dash(6, 3), weight: 1.25, accent: teal }
class frame { fill: none, weight: 1, accent: gray }
class active { fill: #DBEAFE, weight: 0.75, accent: blue }
```

Class properties accept `stroke: solid | dashed | dotted | dash(n, n, ...)`,
`weight: thin | normal | bold | positive_number`, `accent: named_color | HEX`,
`fill: none | tint | named_color | HEX`, `opacity: number` in `[0, 1]`,
`cap: butt | round | square`, and `join: miter | round | bevel`.
Named colors are `ink`, `gray`, `blue`, `orange`, `teal`, `green`, `red`, `purple`.
HEX colors are unquoted `#RRGGBB` or `#RRGGBBAA`; dash lengths and numeric weights
are CSS pixels. Other properties on geometry include `{ mark: none }` for
points used only as anchors.

Sequence diagrams can use participant rectangles with centered labels, downward
lifeline segments, rows derived by negative vertical `offset`, and endpoints
projected onto lifelines with `foot(row, life)`. Use arrows for messages and a
dashed class for returns. A self-call can use two connected segments and a final
arrow returning to the same participant on a lower row. Activation bars and an
`alt` frame use rectangles. A frame with `fill: none` preserves underlying
lifelines; size the frame and row spacing to include both branches and labels.

Text can be plain (`"A"`), all-TeX (`$O_1$`), or mixed
(`"angle $\alpha$"`). Quoted plain text supports `\"`, `\\`, `\$`, and `\n`.
TeX spans preserve backslashes; do not double them as plain-string escapes.
Unsupported TeX reports E414, and the ReX subset is not full LaTeX.
