//! Parsed and resolved scene representations.

use indexmap::IndexMap;
use locus_syntax::{File, HeadKind, Ident, Span, TextValue};

use crate::eval::{EvalError, evaluate};
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

/// A resolved annotation whose final position remains layout's responsibility.
#[derive(Clone, Debug, PartialEq)]
pub enum AnnotationSpec {
    /// Text attached to an anchor.
    Label {
        /// Geometry or deferred layout anchor.
        anchor: AnchorSpec,
        /// Segmented source text.
        text: TextValue,
        /// Requested placement direction.
        direction: LabelDirection,
        /// Optional nonnegative gap in scene units.
        gap: Option<f64>,
    },
    /// Directed annotation between two anchors.
    Arrow {
        /// Start anchor.
        from: AnchorSpec,
        /// End anchor.
        to: AnchorSpec,
        /// Arrow-head style.
        head: HeadKind,
        /// Bend angle in radians.
        bend: f64,
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
    /// Segment geometry.
    Segment(Segment),
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
    /// Returns the geometry-only bounds, if this object contributes geometry.
    #[must_use]
    pub fn bbox(&self) -> Option<BBox> {
        match self {
            Self::Point(point) => BBox::from_points([point.geometry]).ok(),
            Self::Line(line) => Some(BBox::for_line(*line)),
            Self::Ray(ray) => Some(BBox::for_ray(*ray)),
            Self::Segment(segment) => Some(BBox::for_segment(*segment)),
            Self::Num(_) | Self::Angle(_) | Self::Annotation(_) => None,
            Self::Circle(_)
            | Self::Arc(_)
            | Self::Ellipse(_)
            | Self::Bspline(_)
            | Self::Polygon(_)
            | Self::Rect(_)
            | Self::RoundedRect(_)
            | Self::Diamond(_) => self.curved_bbox(),
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
    /// Parsed relational program.
    pub file: File,
    /// Latest direct-evaluation result.
    pub evaluation: Evaluation,
}

impl Scene {
    /// Evaluates a parsed file and creates a scene.
    pub fn new(file: File) -> Result<Self, EvalError> {
        let evaluation = evaluate(&file)?;
        Ok(Self { file, evaluation })
    }

    /// Recomputes the scene from the retained parsed file.
    pub fn recompute(&mut self) -> Result<&Evaluation, EvalError> {
        self.evaluation = evaluate(&self.file)?;
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
