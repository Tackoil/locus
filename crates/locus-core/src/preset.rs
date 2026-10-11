//! Pinned textbook geometry, typography, routing, and settling constants.

/// Scene-space metrics shared by measurement, solving, and rendering layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutMetrics {
    /// Minimum seed width for an automatic box.
    pub default_box_width: f64,
    /// Minimum seed height for an automatic box.
    pub default_box_height: f64,
    /// Inner content margin.
    pub padding: f64,
    /// Preferred separation between full bounds.
    pub gap: f64,
    /// Required clearance between avoided objects.
    pub clearance: f64,
    /// Scene units per measured em.
    pub em_size: f64,
    /// General short-path length.
    pub short_length: f64,
    /// General narrow-shape width.
    pub narrow_width: f64,
    /// Normal typography size in ems.
    pub normal_font_size: f64,
    /// Title typography size in ems.
    pub title_font_size: f64,
    /// Outline width used for collision geometry.
    pub stroke_width: f64,
    /// Allowed annotation extension beyond geometry.
    pub margin: f64,
    /// Default tick length and right-angle side.
    pub mark_size: f64,
    /// Corner radius as a fraction of the shorter automatic-box dimension.
    pub rounded_radius_fraction: f64,
}

impl Default for LayoutMetrics {
    fn default() -> Self {
        Self {
            default_box_width: 2.0,
            default_box_height: 1.0,
            padding: 0.25,
            gap: 0.25,
            clearance: 0.15,
            em_size: 0.28,
            short_length: 1.0,
            narrow_width: 0.3,
            normal_font_size: 1.0,
            title_font_size: 1.4,
            stroke_width: 0.04,
            margin: 2.0,
            mark_size: 0.2,
            rounded_radius_fraction: 0.15,
        }
    }
}

impl LayoutMetrics {
    /// Looks up the supported authored preset length metrics.
    #[must_use]
    pub fn metric(self, name: &str) -> Option<f64> {
        match name {
            "short_length" => Some(self.short_length),
            "narrow_width" => Some(self.narrow_width),
            _ => None,
        }
    }

    /// Derives a valid automatic rounded-box radius from its dimensions.
    #[must_use]
    pub fn automatic_radius(self, width: f64, height: f64) -> f64 {
        width.min(height) * self.rounded_radius_fraction.clamp(0.0, 0.5)
    }
}

/// Initial orthogonal-route detour in scene units.
pub const ROUTE_DETOUR: f64 = 0.5;
/// Positive lower bound on route detours and curved offsets.
pub const ROUTE_MIN_OFFSET: f64 = 1.0e-6;
/// Initial curved-route control offset in scene units.
pub const ROUTE_CONTROL_OFFSET: f64 = 0.5;
/// Default attached-label route parameter.
pub const LABEL_PARAMETER: f64 = 0.5;
/// Preferred source, middle, and target route-parameter regions.
pub const LABEL_REGIONS: [(f64, f64); 3] = [(0.1, 0.3), (0.4, 0.6), (0.7, 0.9)];
/// Deterministic route detour-side order.
pub const ROUTE_SIDE_ORDER: [f64; 2] = [1.0, -1.0];
/// Deterministic page-side order for automatic routes.
pub const ROUTE_PAGE_SIDE_ORDER: [crate::geom::route::RouteSide; 4] = [
    crate::geom::route::RouteSide::Right,
    crate::geom::route::RouteSide::Left,
    crate::geom::route::RouteSide::Above,
    crate::geom::route::RouteSide::Below,
];
/// Working-set activation threshold in normalized coordinates.
pub const ACTIVE_TOLERANCE: f64 = 1.0e-7;
/// Normalized preference-step stopping threshold.
pub const SETTLING_TOLERANCE: f64 = 1.0e-9;
/// Euclidean trust radius for normalized layout preference steps.
pub const SETTLING_TRUST_RADIUS: f64 = 8.0;
/// First regularization relative to the largest squared preference singular value.
pub const SETTLING_TRUST_DAMPING: f64 = 1.0e-6;
/// Maximum regularization trials for a bounded layout preference step.
pub const SETTLING_TRUST_TRIALS: usize = 64;
/// Relative squared-norm accuracy for the preference trust boundary.
pub const SETTLING_TRUST_TOLERANCE: f64 = 1.0e-6;
/// Smallest normalized preference trust radius before reporting a line-search failure.
pub const SETTLING_TRUST_MIN_RADIUS: f64 = 1.0e-6;
/// Radius contraction when restoration or sufficient decrease fails.
pub const SETTLING_TRUST_SHRINK: f64 = 0.25;
/// Minimum meaningful descent derivative per unit direction length.
pub const DIRECTION_TOLERANCE: f64 = 1.0e-14;
/// Smallest backtracking step.
pub const LINE_SEARCH_MIN_ALPHA: f64 = 1.0 / 1024.0;
/// Armijo sufficient-decrease coefficient.
pub const LINE_SEARCH_ARMIJO: f64 = 1.0e-4;
/// Maximum accepted preference moves per continuous solve.
pub const SETTLING_STEP_CAP: usize = 256;
/// Maximum consecutive working-set pivots at unchanged geometry.
pub const SETTLING_PIVOT_CAP: usize = 64;
/// Maximum total preference iterations, including working-set pivots.
pub const SETTLING_WORK_CAP: usize = 1024;
/// Relative gradient at which one local curvature rebuild may be attempted.
pub const SETTLING_RECOVERY_GRADIENT: f64 = 1.0e-5;
/// Central finite-difference step in normalized tangent coordinates.
pub const SETTLING_RECOVERY_DIFFERENCE: f64 = 1.0e-4;
/// Maximum tangent dimension for bounded curvature recovery.
pub const SETTLING_RECOVERY_DIMENSIONS: usize = 32;
/// Maximum feasibility-restoration attempts per settling step.
pub const RESTORATION_CAP: usize = 8;
/// Relative threshold used only to prioritize materially failing discrete rows.
pub const SEARCH_ROW_PRIORITY_FACTOR: f64 = 1.0e-8;
/// Maximum forced escape-direction choices for each avoidance pair.
pub const AVOIDANCE_DIRECTION_CAP: usize = 16;
/// Maximum outer discrete-layout candidates.
pub const SEARCH_CAP: usize = 32;
/// Maximum solve/layout measurement-feedback passes.
pub const MEASUREMENT_PASS_CAP: usize = 3;
/// Maximum adaptive interval-search depth for render-only boundary clipping.
pub const CLIPPING_SEARCH_DEPTH: usize = 48;
/// Fixed boundary intersection refinement count after bracketing.
pub const CLIPPING_BISECTIONS: usize = 64;

/// Fixed bisection count for exact ellipse boundary-distance nearest points.
pub const ELLIPSE_DISTANCE_ITERATIONS: usize = 96;

/// Bounded geometric detour enlargement factors, in deterministic order.
pub const ROUTE_OFFSET_FACTORS: [f64; 8] = [1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0];
/// Preferred page sides for unpinned attached labels.
pub const LABEL_PAGE_SIDE_ORDER: [crate::geom::route::RouteSide; 4] = [
    crate::geom::route::RouteSide::Above,
    crate::geom::route::RouteSide::Below,
    crate::geom::route::RouteSide::Right,
    crate::geom::route::RouteSide::Left,
];
/// Initial outward contact-clearance bracket subdivision count.
pub const ATTACHMENT_SEARCH_STEPS: u32 = 256;
/// Maximum adaptive intervals used to prove a curved contact prefix stays outside.
pub const ATTACHMENT_PREFIX_CAP: usize = 2048;
/// Shared conservative quadratic/cubic collision subdivision count.
pub const COLLISION_SUBDIVISIONS: u32 = 64;

/// Maximum adaptive intervals used to locate a visible endpoint boundary crossing.
pub const CLIPPING_INTERVAL_CAP: usize = 16_384;
