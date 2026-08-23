//! Public completion tables shared by the parser, CLI, and future language server.

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
