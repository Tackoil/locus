//! Public completion tables shared by the parser, CLI, and future language server.

/// Non-object top-level statement keywords.
pub const STATEMENT_NAMES: &[&str] = &["locus", "meta", "style", "class", "rel", "assert"];

/// Object kind keywords.
pub const KIND_NAMES: &[&str] = &[
    "point",
    "num",
    "line",
    "ray",
    "segment",
    "circle",
    "arc",
    "polygon",
    "rect",
    "diamond",
    "label",
    "arrow",
    "tick",
    "right_angle",
    "text_block",
];

/// All derived constructor names from specification Part 5.
pub const CONSTRUCTOR_NAMES: &[&str] = &[
    "midpoint",
    "lerp",
    "offset",
    "intersect",
    "foot",
    "project",
    "rotate",
    "reflect",
    "translate",
    "on_circle",
    "extend",
    "centroid",
    "circumcenter",
    "incenter",
    "orthocenter",
    "center",
    "line",
    "ray",
    "seg",
    "perpendicular",
    "parallel",
    "perp_bisector",
    "angle_bisector",
    "tangent_line",
    "circle",
    "circle_through",
    "circle_diameter",
    "circumcircle",
    "incircle",
    "arc",
    "angle_arc",
    "polygon",
    "rect",
    "diamond",
    "label",
    "arrow",
    "tick",
    "right_angle",
    "text_block",
];

/// Canonical constructor signatures for editor documentation.
///
/// Entries use source-language names and intentionally preserve constructor order.
pub const CONSTRUCTOR_SIGNATURES: &[(&str, &str)] = &[
    ("midpoint", "midpoint(A, B) -> point"),
    ("lerp", "lerp(A, B, t) -> point"),
    ("offset", "offset(P, dx, dy) -> point"),
    ("intersect", "intersect(o1, o2 [, k]) -> point"),
    ("foot", "foot(P, linear) -> point"),
    ("project", "project(P, circle) -> point"),
    ("rotate", "rotate(P, O, angle) -> point"),
    ("reflect", "reflect(P, axis) -> point"),
    ("translate", "translate(P, from, to) -> point"),
    ("on_circle", "on_circle(circle, angle) -> point"),
    ("extend", "extend(A, B, distance) -> point"),
    ("centroid", "centroid(A, B, C, ...) -> point"),
    ("circumcenter", "circumcenter(A, B, C) -> point"),
    ("incenter", "incenter(A, B, C) -> point"),
    ("orthocenter", "orthocenter(A, B, C) -> point"),
    ("center", "center(circle_or_arc) -> point"),
    ("line", "line(A, B) -> line"),
    ("ray", "ray(A, B) -> ray"),
    ("seg", "seg(A, B) -> segment"),
    ("perpendicular", "perpendicular(P, linear) -> line"),
    ("parallel", "parallel(P, linear) -> line"),
    ("perp_bisector", "perp_bisector(A, B) -> line"),
    ("angle_bisector", "angle_bisector(A, B, C) -> line"),
    ("tangent_line", "tangent_line(P, circle, k) -> line"),
    ("circle", "circle(O, radius) -> circle"),
    ("circle_through", "circle_through(O, A) -> circle"),
    ("circle_diameter", "circle_diameter(A, B) -> circle"),
    ("circumcircle", "circumcircle(A, B, C) -> circle"),
    ("incircle", "incircle(A, B, C) -> circle"),
    ("arc", "arc(circle, P, Q, ccw) -> arc"),
    ("angle_arc", "angle_arc(A, B, C, radius) -> arc"),
    ("polygon", "polygon(A, B, C, ...) -> polygon"),
    ("rect", "rect(center, width, height) -> rect"),
    ("diamond", "diamond(center, width, height) -> diamond"),
    ("label", "label(anchor, text) -> label"),
    ("arrow", "arrow(from, to) -> arrow"),
    ("tick", "tick(segment, count) -> tick"),
    ("right_angle", "right_angle(V, P, Q) -> right_angle"),
    ("text_block", "text_block(text) -> text_block"),
];

/// Relation and assert predicate names from specification Part 3.
pub const PREDICATE_NAMES: &[&str] = &[
    "on",
    "tangent",
    "parallel",
    "perpendicular",
    "collinear",
    "concyclic",
    "congruent",
    "coincident",
    "midpoint",
    "horizontal",
    "vertical",
    "pin",
    "between",
    "inside",
];

/// Numeric expression functions from specification Part 6.
pub const NUMERIC_FUNCTION_NAMES: &[&str] = &[
    "dist",
    "hdist",
    "vdist",
    "angle",
    "dir_angle",
    "ratio",
    "radius",
    "length",
    "area",
    "perimeter",
    "sqrt",
    "abs",
    "min",
    "max",
];

/// Closed set of operation properties, including constructor-specific annotation props.
pub const PROP_NAMES: &[&str] = &[
    "dir", "gap", "head", "bend", "n", "size", "place", "anchor", "mark",
];

/// Properties accepted by style class declarations.
pub const STYLE_PROP_NAMES: &[&str] = &[
    "stroke", "weight", "accent", "fill", "opacity", "cap", "join",
];

/// Geometry and layout property/anchor names.
pub const PROPERTY_NAMES: &[&str] = &[
    "midpoint", "start", "end", "center", "at", "centroid", "n", "s", "e", "w", "ne", "nw", "se",
    "sw", "width", "height",
];

/// Accepted angle-unit suffixes.
pub const UNIT_NAMES: &[&str] = &["deg", "rad"];

/// Built-in style attributes.
pub const BUILTIN_ATTR_NAMES: &[&str] = &["main", "auxiliary", "highlight", "construction"];

/// Closed-set literal values accepted by properties and expressions.
pub const ENUM_VALUE_NAMES: &[&str] = &[
    "true", "false", "auto", "center", "N", "NE", "E", "SE", "S", "SW", "W", "NW", "dot", "cross",
    "none", "below", "above", "left", "right", "solid", "dashed", "dotted", "thin", "normal",
    "bold", "ink", "gray", "blue", "orange", "teal", "green", "red", "purple", "tint", "butt",
    "round", "square", "miter", "bevel",
];

/// Returns whether a call name belongs to any expression-level vocabulary.
#[must_use]
pub fn is_known_call(name: &str) -> bool {
    CONSTRUCTOR_NAMES.contains(&name)
        || PREDICATE_NAMES.contains(&name)
        || NUMERIC_FUNCTION_NAMES.contains(&name)
        || name == "dash"
}

/// Returns all call names in stable suggestion order.
#[must_use]
pub fn all_call_names() -> Vec<&'static str> {
    let mut names = Vec::with_capacity(
        CONSTRUCTOR_NAMES.len() + PREDICATE_NAMES.len() + NUMERIC_FUNCTION_NAMES.len() + 1,
    );
    names.extend_from_slice(CONSTRUCTOR_NAMES);
    names.extend_from_slice(PREDICATE_NAMES);
    names.extend_from_slice(NUMERIC_FUNCTION_NAMES);
    names.push("dash");
    names
}
