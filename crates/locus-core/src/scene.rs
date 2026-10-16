//! Parsed and resolved scene representations.

use indexmap::IndexMap;
use locus_syntax::{File, HeadKind, Ident, Span, TextValue};

use crate::eval::EvalError;
use crate::geom::{Arc, BBox, Bspline, Circle, Ellipse, Line, Point, Polygon, Ray, Segment};
use crate::solve::SolveSummary;

/// A numeric semantic dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dimension {
    /// Unitless count, ratio, or coefficient.
    Scalar,
    /// Scene-space distance.
    Length,
    /// Squared scene-space distance.
    Area,
}

/// A finite numeric value and its inferred dimension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericValue {
    /// Numeric magnitude.
    pub value: f64,
    /// Semantic dimension.
    pub dimension: Dimension,
}

/// Visible point marker selected by the `mark` property.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointMark {
    /// Filled point marker.
    #[default]
    Dot,
    /// Cross-shaped point marker.
    Cross,
    /// Invisible geometry point.
    None,
}

/// Resolved point geometry together with its rendering-semantic properties.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedPoint {
    /// Point geometry used by constructions and measurements.
    pub geometry: Point,
    /// Marker retained for the rendering tier.
    pub mark: PointMark,
}

impl ResolvedPoint {
    /// Creates a point with the specification's default dot marker.
    #[must_use]
    pub const fn new(geometry: Point) -> Self {
        Self {
            geometry,
            mark: PointMark::Dot,
        }
    }
}

/// Resolved arc geometry and optional equal-angle multiplicity marks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedArc {
    /// Directed arc geometry.
    pub geometry: Arc,
    /// `Some(1..=4)` for `angle_arc`; `None` for an ordinary arc.
    pub angle_mark_count: Option<u8>,
}

/// A geometry anchor or an annotation-layer anchor deferred to layout.
#[derive(Clone, Debug, PartialEq)]
pub enum AnchorSpec {
    /// A point already resolved by the geometry tier.
    Point(Point),
    /// The eventual center of an annotation's laid-out box.
    AnnotationCenter {
        /// Referenced annotation id.
        id: String,
        /// Source location of the reference.
        span: Span,
    },
}

/// Label placement direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LabelDirection {
    /// Layout chooses a candidate.
    #[default]
    Auto,
    /// Center text on its anchor.
    Center,
    /// Cardinal or diagonal direction token.
    Compass(CompassDirection),
}

/// Cardinal and diagonal placement directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompassDirection {
    /// North.
    N,
    /// Northeast.
    Ne,
    /// East.
    E,
    /// Southeast.
    Se,
    /// South.
    S,
    /// Southwest.
    Sw,
    /// West.
    W,
    /// Northwest.
    Nw,
}

/// Text-block placement before layout.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum TextBlockPlacement {
    /// Below the figure bounding box.
    #[default]
    Below,
    /// Above the figure bounding box.
    Above,
    /// Left of the figure bounding box.
    Left,
    /// Right of the figure bounding box.
    Right,
    /// Relative to an explicit anchor.
    Anchor {
        /// Anchor target.
        anchor: AnchorSpec,
        /// Requested direction.
        direction: LabelDirection,
    },
}

/// Accepted continuous placement of text on a logical or retained route.
#[derive(Clone, Debug, PartialEq)]
pub struct PathLabel {
    /// Expanded path declaration id.
    pub path: String,
    /// Solved route parameter within the preferred region.
    pub parameter: f64,
    /// Solved clearance from the route to the text box.
    pub offset: f64,
    /// Frozen page side.
    pub side: crate::geom::route::RouteSide,
    /// Allowed source, middle, or target parameter interval.
    pub region: (f64, f64),
    /// Preferred candidate: midpoint, lower endpoint, or upper endpoint.
    pub candidate: usize,
}

/// A resolved annotation whose final position remains layout's responsibility.
#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationSpec {
    /// Text attached to an anchor.
    Label {
        /// Accepted route attachment, when this text follows a path.
        path: Option<PathLabel>,
        /// Solve-time measured bounds for independently placed content.
        bounds: Option<BBox>,
        /// Geometry or deferred layout anchor.
        anchor: AnchorSpec,
        /// Segmented source text.
        text: TextValue,
        /// Requested placement direction.
        direction: LabelDirection,
        /// Optional nonnegative gap in scene units.
        gap: Option<f64>,
    },
    /// Equal-length tick marks on a segment.
    Tick {
        /// Marked segment.
        segment: Segment,
        /// Number of marks, in `1..=4`.
        count: u8,
    },
    /// Right-angle square between two rays.
    RightAngle {
        /// Shared ray vertex.
        vertex: Point,
        /// Point on the first ray.
        first: Point,
        /// Point on the second ray.
        second: Point,
        /// Optional positive explicit size.
        size: Option<f64>,
    },
    /// Figure-relative or explicitly anchored block text.
    TextBlock {
        /// Segmented source text.
        text: TextValue,
        /// Placement request.
        placement: TextBlockPlacement,
    },
}

/// Finite path geometry, preserving the straight-segment contract.
#[derive(Clone, Debug, PartialEq)]
pub enum PathGeometry {
    /// An exact straight segment, including a possible zero-length segment.
    Segment(Segment),
    /// A connector with existing endpoint identities and an optional accepted route.
    Routed {
        /// Accepted logical route, including internal continuous parameters.
        route: Option<crate::geom::route::Route>,
        /// Logical source anchor, before visible endpoint clipping.
        from: AnchorSpec,
        /// Logical target anchor, before visible endpoint clipping.
        to: AnchorSpec,
        /// Direct-route bend angle in radians.
        bend: f64,
    },
}

/// Shared finite path with arrowheads independent of its centerline geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPath {
    /// Straight or routed centerline definition.
    pub geometry: PathGeometry,
    /// Endpoint decoration; does not change mathematical geometry or measurements.
    pub head: HeadKind,
    /// Attached-caption bounds, included only in layout bounds.
    pub label_bounds: Option<BBox>,
}

impl ResolvedPath {
    /// Constructs an undecorated straight path.
    #[must_use]
    pub const fn segment(segment: Segment) -> Self {
        Self {
            geometry: PathGeometry::Segment(segment),
            head: HeadKind::None,
            label_bounds: None,
        }
    }

    /// Resolves the logical centerline when its anchors are geometric points.
    pub fn route(&self) -> Result<Option<crate::geom::route::Route>, crate::ConstructError> {
        use crate::geom::route::Route;
        match &self.geometry {
            PathGeometry::Segment(segment) => {
                Route::direct(segment.start, segment.end, 0.0).map(Some)
            }
            PathGeometry::Routed {
                route: Some(route), ..
            } => Ok(Some(route.clone())),
            PathGeometry::Routed {
                from: AnchorSpec::Point(from),
                to: AnchorSpec::Point(to),
                bend,
                ..
            } => Route::direct(*from, *to, *bend).map(Some),
            PathGeometry::Routed { .. } => Ok(None),
        }
    }

    /// Logical centerline bounds without arrowheads or attached captions.
    #[must_use]
    pub fn geometry_bbox(&self) -> Option<BBox> {
        self.route().ok().flatten().map(|route| route.bbox())
    }

    /// Layout bounds including attached captions.
    #[must_use]
    pub fn layout_bbox(&self) -> Option<BBox> {
        let bounds = self.geometry_bbox()?;
        Some(
            self.label_bounds
                .map_or(bounds, |labels| bounds.union(labels)),
        )
    }
}

/// A fully evaluated declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedObject {
    /// Named numeric value.
    Num(NumericValue),
    /// Named angle value in radians.
    Angle(f64),
    /// Point geometry.
    Point(ResolvedPoint),
    /// Infinite line geometry.
    Line(Line),
    /// Half-line geometry.
    Ray(Ray),
    /// Finite straight or routed path with independent endpoint decoration.
    Path(ResolvedPath),
    /// Circle geometry.
    Circle(Circle),
    /// Arc geometry.
    Arc(ResolvedArc),
    /// Ellipse geometry.
    Ellipse(Ellipse),
    /// Open cubic B-spline geometry.
    Bspline(Bspline),
    /// General polygon geometry.
    Polygon(Polygon),
    /// Axis-aligned rectangle geometry.
    Rect(Polygon),
    /// Axis-aligned rectangle with circular corners.
    RoundedRect(crate::geom::RoundedRect),
    /// Axis-aligned diamond geometry.
    Diamond(Polygon),
    /// Annotation input for the layout tier.
    Annotation(AnnotationSpec),
}

impl ResolvedObject {
    /// Constructs a straight finite path without arrowheads.
    #[must_use]
    pub const fn segment(segment: Segment) -> Self {
        Self::Path(ResolvedPath::segment(segment))
    }

    /// Borrows a finite path independently of its constructor spelling.
    #[must_use]
    pub const fn as_path(&self) -> Option<&ResolvedPath> {
        if let Self::Path(path) = self {
            Some(path)
        } else {
            None
        }
    }

    /// Returns the geometry-only bounds, if this object contributes geometry.
    #[must_use]
    pub fn bbox(&self) -> Option<BBox> {
        match self {
            Self::Point(point) => BBox::from_points([point.geometry]).ok(),
            Self::Line(line) => Some(BBox::for_line(*line)),
            Self::Ray(ray) => Some(BBox::for_ray(*ray)),
            Self::Path(path) => path.layout_bbox(),
            Self::Annotation(annotation) => annotation_bbox(annotation),
            Self::Num(_) | Self::Angle(_) => None,
            _ => self.curved_bbox(),
        }
    }

    fn curved_bbox(&self) -> Option<BBox> {
        match self {
            Self::RoundedRect(rect) => Some(rect.bbox()),
            Self::Circle(circle) => Some(BBox::for_circle(*circle)),
            Self::Arc(arc) => Some(BBox::for_arc(arc.geometry)),
            Self::Ellipse(ellipse) => Some(BBox::for_ellipse(*ellipse)),
            Self::Bspline(spline) => Some(BBox::for_bspline(spline)),
            Self::Polygon(polygon) | Self::Rect(polygon) | Self::Diamond(polygon) => {
                Some(BBox::for_polygon(polygon))
            }
            _ => None,
        }
    }
}

fn annotation_bbox(annotation: &AnnotationSpec) -> Option<BBox> {
    match annotation {
        AnnotationSpec::Label { bounds, .. } => *bounds,
        _ => None,
    }
}

/// Complete derived-tier output in file declaration order.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedScene {
    /// Objects keyed by their source identifiers in insertion order.
    pub objects: IndexMap<Ident, ResolvedObject>,
    /// Geometry-only bounding box.
    pub bbox: BBox,
}

/// Result of direct evaluation or relational solving.
#[derive(Clone, Debug, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "retain the public evaluation variants without allocating solver metadata for every scene"
)]
pub enum Evaluation {
    /// All declarations were derived and resolved.
    Resolved(ResolvedScene),
    /// Free declarations were solved and carry a reusable report.
    Solved {
        /// Complete geometry and annotations after final evaluation.
        scene: ResolvedScene,
        /// Normative solver statistics and diagnostics.
        summary: SolveSummary,
    },
}

/// Parsed source together with its latest evaluation state.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// Renderer measurements and pinned scene-space layout metrics.
    pub inputs: crate::inputs::EvaluationInputs,
    /// Editable parsed program, including function definitions and named calls.
    pub source_file: File,
    /// Expanded relational program in declaration order.
    pub file: File,
    /// Normalized groups and owned instance members retained by expansion.
    pub collections: std::collections::BTreeMap<String, locus_syntax::Collection>,
    /// Body spans and invocation chains, aligned with the expanded statements.
    pub origins: Vec<locus_syntax::ExpansionOrigin>,
    /// Latest direct-evaluation result.
    pub evaluation: Evaluation,
}

impl Scene {
    /// Evaluates a parsed file and creates a scene.
    pub fn new(file: File) -> Result<Self, EvalError> {
        Self::with_inputs(file, crate::inputs::EvaluationInputs::default())
    }

    /// Evaluates a source program using renderer-supplied measurements.
    pub fn with_inputs(
        file: File,
        inputs: crate::inputs::EvaluationInputs,
    ) -> Result<Self, EvalError> {
        let _scene = crate::metrics::span("evaluate_scene");
        let expanded = crate::program::expand_program(&file)?;
        let evaluation =
            crate::eval::evaluate_flat_with_inputs(&expanded.file, &inputs, &expanded.collections)
                .map_err(|error| crate::program::annotate_error(error, &expanded))?;
        Ok(Self {
            inputs,
            source_file: file,
            file: expanded.file,
            collections: expanded.collections,
            origins: expanded.origins,
            evaluation,
        })
    }

    /// Builds relation-aware initialization geometry for discrete layout recovery.
    ///
    /// This scene is explicitly unsolved: it carries no solve summary, and its
    /// geometry must never be accepted as satisfying authored relations. Consumers
    /// may derive clipping/routing choices and then run the ordinary solver.
    ///
    /// # Errors
    /// Returns source/measurement or initialization failures, without solving.
    pub fn layout_seed_with_inputs(
        file: File,
        inputs: crate::inputs::EvaluationInputs,
    ) -> Result<Self, EvalError> {
        let expanded = crate::program::expand_program(&file)?;
        let prepared = crate::eval::PreparedEvaluation::with_inputs(
            &expanded.file,
            &inputs,
            &expanded.collections,
        )?;
        let resolved = crate::solve::layout_seed(&prepared)
            .map_err(|error| crate::program::annotate_error(error, &expanded))?;
        Ok(Self {
            inputs,
            source_file: file,
            file: expanded.file,
            collections: expanded.collections,
            origins: expanded.origins,
            evaluation: Evaluation::Resolved(resolved),
        })
    }

    /// Recomputes the scene from the retained parsed file.
    pub fn recompute(&mut self) -> Result<&Evaluation, EvalError> {
        let updated = Self::with_inputs(self.source_file.clone(), self.inputs.clone())?;
        *self = updated;
        Ok(&self.evaluation)
    }

    /// Returns the complete resolved scene when no solve is required.
    #[must_use]
    pub const fn resolved(&self) -> Option<&ResolvedScene> {
        match &self.evaluation {
            Evaluation::Resolved(scene) | Evaluation::Solved { scene, .. } => Some(scene),
        }
    }

    /// Returns the solve summary for a relational scene.
    #[must_use]
    pub const fn solve_summary(&self) -> Option<&SolveSummary> {
        match &self.evaluation {
            Evaluation::Solved { summary, .. } => Some(summary),
            Evaluation::Resolved(_) => None,
        }
    }
}
