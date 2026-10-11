//! Public completion tables shared by the parser, CLI, and future language server.

use crate::ast::Kind;

/// Non-object top-level statement keywords.
pub const STATEMENT_NAMES: &[&str] = &["locus", "meta", "style", "class", "rel", "assert"];

/// Built-in rendering presets accepted by a `style` statement.
pub const STYLE_PRESET_NAMES: &[&str] = &["textbook"];

/// The special constructor used by free point and numeric declarations.
pub const FREE_CONSTRUCTOR_NAME: &str = "free";

/// Object kind keywords.
pub const KIND_NAMES: &[&str] = &[
    "point",
    "num",
    "line",
    "ray",
    "segment",
    "circle",
    "arc",
    "ellipse",
    "bspline",
    "polygon",
    "rect",
    "rounded_rect",
    "diamond",
    "label",
    "arrow",
    "tick",
    "right_angle",
    "text_block",
];

/// All derived constructor names from specification Part 5.
pub const CONSTRUCTOR_NAMES: &[&str] = &[
    "point",
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
    "ellipse",
    "bspline",
    "polygon",
    "rect",
    "rounded_rect",
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
    ("point", "point() -> point"),
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
    ("circle", "circle(radius) | circle(O, radius) -> circle"),
    ("circle_through", "circle_through(O, A) -> circle"),
    ("circle_diameter", "circle_diameter(A, B) -> circle"),
    ("circumcircle", "circumcircle(A, B, C) -> circle"),
    ("incircle", "incircle(A, B, C) -> circle"),
    ("arc", "arc(circle, P, Q, ccw) -> arc"),
    ("angle_arc", "angle_arc(A, B, C, radius) -> arc"),
    (
        "ellipse",
        "ellipse(a, b [, angle]) | ellipse(O, a, b [, angle]) -> ellipse",
    ),
    ("bspline", "bspline(P0, P1, P2, P3, ...) -> bspline"),
    ("polygon", "polygon(A, B, C, ...) -> polygon"),
    ("rect", "rect(center, width, height) -> rect"),
    (
        "rounded_rect",
        "rounded_rect(width, height, radius) | rounded_rect(center, width, height, radius) -> rounded_rect",
    ),
    ("diamond", "diamond(center, width, height) -> diamond"),
    ("label", "label(anchor, text) -> label"),
    ("arrow", "arrow(from, to) -> arrow"),
    ("tick", "tick(segment, count) -> tick"),
    ("right_angle", "right_angle(V, P, Q) -> right_angle"),
    ("text_block", "text_block(text) -> text_block"),
];

/// Returns the result kind of a derived constructor.
#[must_use]
pub fn constructor_kind(name: &str) -> Option<Kind> {
    CONSTRUCTOR_KINDS
        .iter()
        .find_map(|(candidate, kind)| (*candidate == name).then_some(*kind))
}

const CONSTRUCTOR_KINDS: &[(&str, Kind)] = &[
    ("point", Kind::Point),
    ("midpoint", Kind::Point),
    ("lerp", Kind::Point),
    ("offset", Kind::Point),
    ("intersect", Kind::Point),
    ("foot", Kind::Point),
    ("project", Kind::Point),
    ("rotate", Kind::Point),
    ("reflect", Kind::Point),
    ("translate", Kind::Point),
    ("on_circle", Kind::Point),
    ("extend", Kind::Point),
    ("centroid", Kind::Point),
    ("circumcenter", Kind::Point),
    ("incenter", Kind::Point),
    ("orthocenter", Kind::Point),
    ("center", Kind::Point),
    ("line", Kind::Line),
    ("ray", Kind::Ray),
    ("seg", Kind::Segment),
    ("perpendicular", Kind::Line),
    ("parallel", Kind::Line),
    ("perp_bisector", Kind::Line),
    ("angle_bisector", Kind::Line),
    ("tangent_line", Kind::Line),
    ("circle", Kind::Circle),
    ("circle_through", Kind::Circle),
    ("circle_diameter", Kind::Circle),
    ("circumcircle", Kind::Circle),
    ("incircle", Kind::Circle),
    ("arc", Kind::Arc),
    ("angle_arc", Kind::Arc),
    ("ellipse", Kind::Ellipse),
    ("bspline", Kind::Bspline),
    ("polygon", Kind::Polygon),
    ("rect", Kind::Rect),
    ("rounded_rect", Kind::RoundedRect),
    ("diamond", Kind::Diamond),
    ("label", Kind::Label),
    ("arrow", Kind::Arrow),
    ("tick", Kind::Tick),
    ("right_angle", Kind::RightAngle),
    ("text_block", Kind::TextBlock),
];

/// Relation and assert predicate names from specification Part 3.
pub const PREDICATE_NAMES: &[&str] = &[
    "on",
    "tangent",
    "internally_tangent",
    "parallel",
    "perpendicular",
    "collinear",
    "concyclic",
    "congruent",
    "coincident",
    "midpoint",
    "horizontal",
    "vertical",
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

const PROP_NAMES_BY_KIND: [&[&str]; 18] = [
    &["mark"],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &[],
    &["dir", "gap"],
    &["head", "bend"],
    &[],
    &["size"],
    &["place", "anchor", "dir"],
];

/// Returns the operation properties accepted by an object kind and constructor.
#[must_use]
pub fn prop_names_for_kind(kind: Kind, constructor: Option<&str>) -> &'static [&'static str] {
    if kind == Kind::Arc && constructor == Some("angle_arc") {
        &["n"]
    } else {
        PROP_NAMES_BY_KIND[kind as usize]
    }
}

/// Properties accepted by style class declarations.
pub const STYLE_PROP_NAMES: &[&str] = &[
    "stroke", "weight", "accent", "fill", "opacity", "cap", "join",
];

/// Geometry and layout property/anchor names.
pub const PROPERTY_NAMES: &[&str] = &[
    "midpoint",
    "start",
    "end",
    "center",
    "at",
    "centroid",
    "n",
    "s",
    "e",
    "w",
    "ne",
    "nw",
    "se",
    "sw",
    "corner_radius",
    "width",
    "height",
    "focus1",
    "focus2",
    "major_start",
    "major_end",
    "minor_start",
    "minor_end",
    "semimajor",
    "semiminor",
    "rotation",
];

/// Accepted angle-unit suffixes.
pub const UNIT_NAMES: &[&str] = &["deg", "rad"];

/// Boolean literals accepted in expressions.
pub const BOOLEAN_LITERAL_NAMES: &[&str] = &["true", "false"];

/// Boolean operators accepted in relation and assertion expressions.
pub const BOOLEAN_OPERATOR_NAMES: &[&str] = &["not", "and", "or"];

/// Built-in style attributes.
pub const BUILTIN_ATTR_NAMES: &[&str] = &["main", "auxiliary", "highlight", "construction"];

/// Closed-set literal values accepted by properties and expressions.
pub const ENUM_VALUE_NAMES: &[&str] = &[
    "true", "false", "auto", "center", "N", "NE", "E", "SE", "S", "SW", "W", "NW", "dot", "cross",
    "none", "below", "above", "left", "right", "solid", "dashed", "dotted", "thin", "normal",
    "bold", "ink", "gray", "blue", "orange", "teal", "green", "red", "purple", "tint", "butt",
    "round", "square", "miter", "bevel",
];

const DIRECTION_NAMES: &[&str] = &["auto", "center", "N", "NE", "E", "SE", "S", "SW", "W", "NW"];
const PLACEMENT_NAMES: &[&str] = &["below", "above", "left", "right"];
const MARK_NAMES: &[&str] = &["dot", "cross", "none"];
const HEAD_NAMES: &[&str] = &["->", "<->", "-"];
const STROKE_NAMES: &[&str] = &["solid", "dashed", "dotted"];
const WEIGHT_NAMES: &[&str] = &["thin", "normal", "bold"];
const COLOR_NAMES: &[&str] = &[
    "ink", "gray", "blue", "orange", "teal", "green", "red", "purple",
];
const FILL_NAMES: &[&str] = &[
    "none", "tint", "ink", "gray", "blue", "orange", "teal", "green", "red", "purple",
];
const CAP_NAMES: &[&str] = &["butt", "round", "square"];
const JOIN_NAMES: &[&str] = &["miter", "round", "bevel"];
const DASH_CALL_NAMES: &[&str] = &["dash"];
const PROPERTY_ENUM_VALUES: &[(&str, &[&str])] = &[
    ("dir", DIRECTION_NAMES),
    ("place", PLACEMENT_NAMES),
    ("mark", MARK_NAMES),
    ("head", HEAD_NAMES),
    ("stroke", STROKE_NAMES),
    ("weight", WEIGHT_NAMES),
    ("accent", COLOR_NAMES),
    ("fill", FILL_NAMES),
    ("cap", CAP_NAMES),
    ("join", JOIN_NAMES),
];

/// Returns the closed-set values accepted by a named property.
#[must_use]
pub fn enum_values_for_property(name: &str) -> &'static [&'static str] {
    PROPERTY_ENUM_VALUES
        .iter()
        .find_map(|(candidate, values)| (*candidate == name).then_some(*values))
        .unwrap_or_default()
}

/// Returns call names accepted as values by a named property.
#[must_use]
pub fn call_names_for_property(name: &str) -> &'static [&'static str] {
    match name {
        "stroke" => DASH_CALL_NAMES,
        _ => &[],
    }
}

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
