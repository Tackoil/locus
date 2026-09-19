# Write Locus geometry

Produce a complete Locus format-3 file for the user's diagram. Return source
only: no Markdown fences, commentary, tool calls, or external assets.

## File and declaration rules

- Begin with `locus 3`, then `style textbook`.
- Declare one object per line: `kind id = constructor(args)`.
- Identifiers are unique and case-sensitive. Define every dependency before use.
- Use parameterless `point()` for independent vertices. Describe placement with
  relative relationships; do not use coordinate arguments or absolute pins.
- Optional suffixes are `where { relation, relation }`, then `[style_class]`,
  then `{ property: value }`. Standalone constraints use `rel expression`.
- Use `//` comments, ordinary numbers, and `+ - * /` in numeric expressions.
- Every declaration and assertion must end with a newline, including the final
  statement. End your response with a newline.

Placement example (adapt names and dimensions to the task):

    point P = point()
    point Q = point() where { horizontal(P, Q), hdist(P, Q) == 2 }
    point R = point() where { vertical(P, R), vdist(P, R) == 5 }

`hdist(P, Q)` and `vdist(P, Q)` are signed offsets from P to Q. `dist(P, Q)`
is unsigned Euclidean distance between points, not point-to-line distance.

## Geometry toolbox

Use these constructors with previously declared points or objects:

    polygon name = polygon(P, Q, R)
    segment name = seg(P, Q)
    line name = line(P, Q)
    point name = midpoint(P, Q)
    point name = foot(P, line(Q, R))
    circle name = circle_diameter(P, Q)
    circle name = circumcircle(P, Q, R)
    circle name = incircle(P, Q, R)

These are signatures; choose unique names. Prefer derived constructions over
solving an already constructible object. A right triangle's hypotenuse is a
diameter of its circumcircle. `[construction]`, `[auxiliary]`, and `[highlight]`
select textbook styles for geometry.

## Annotations

    label caption = label(P, "P") { dir: SW }
    right_angle square = right_angle(P, Q, R)
    text_block note = text_block("A plain-text caption") { place: below }

`right_angle(V, P, Q)` marks the vertex V between rays VP and VQ. Choose
perpendicular rays. Label directions are N, NE, E, SE, S, SW, W, NW, or auto.
Use quoted plain text throughout; no TeX or dollar-delimited strings.

## Verify intent

After declarations, add separate `assert expression` lines. Use `==` for
numeric equality and `and` to combine checks. Compare points with
`coincident(P, Q)`, never `P == Q`. Measurements: `dist(P, Q)`,
`length(segment)`, `area(polygon)`, `radius(circle)`, `center(circle)`.
Predicates: `horizontal(P, Q)`, `vertical(P, Q)`, `on(point, object)`,
`between(P, Q, R)` (Q lies on segment PR), `inside(point, polygon)`,
`coincident(P, Q)`, `perpendicular(linear, linear)`,
`tangent(circle, line_or_segment)`.

Check the requested dimensions, incidences, perpendicularity, and circle
relationships. Include all geometry and annotations requested by the user.
Check names, dependency order, constructor types, and assertion truth before
returning the file. Your answer is rendered directly in one attempt.
