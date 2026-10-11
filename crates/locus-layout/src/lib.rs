//! Deterministic measured annotation placement and bounded solve feedback.
//!
//! Measure expanded text with [`measurement::measure_file`] before evaluation,
//! then use [`solve_layout`] to agree on continuous and discrete placement.
//! [`boxes::measure_scene`] and [`layout`] also support already resolved scenes.
//! Measurements are in ems; [`LayoutOptions::em_size`] converts them to scene
//! units. Output retains the original annotations and source spans.

pub mod assign;
pub mod boxes;
pub mod candidates;
mod clipping;
pub mod discrete;
pub mod measurement;
mod path_labels;
pub mod report;
mod routing;
pub mod score;
pub use discrete::{LayoutFeedback, layout_program};
mod feedback;
pub use feedback::{SolveLayoutError, solve_layout};

use indexmap::IndexMap;
use locus_core::{AnnotationSpec, BBox, InspectDiagnostic, Point, ResolvedScene, Segment};
use locus_syntax::{Ident, Span};
use locus_tex::TexBox;
use thiserror::Error;

/// Measured and composed text, keyed by annotation name.
pub type TextBoxes = IndexMap<String, TexBox>;

/// Scene-space style values consumed by layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutOptions {
    /// Scene units per measured em (measurements already include font size).
    pub em_size: f64,
    /// Default gap from an anchor to the nearest box edge.
    pub gap: f64,
    /// Draw connectors from label anchors to their text boxes (off by default).
    pub label_connectors: bool,
    /// Allowed extension beyond the geometry bounds.
    pub margin: f64,
    /// Width used for geometry and annotation stroke collision checks.
    pub stroke_width: f64,
    /// Default tick length and right-angle square side.
    pub mark_size: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        let metrics = locus_core::preset::LayoutMetrics::default();
        Self {
            em_size: 1.0,
            gap: metrics.gap,
            label_connectors: false,
            margin: metrics.margin,
            stroke_width: metrics.stroke_width,
            mark_size: metrics.mark_size,
        }
    }
}

impl LayoutOptions {
    pub(crate) fn validate(self) -> Result<(), LayoutError> {
        let nonnegative = [self.gap, self.margin, self.stroke_width];
        let positive = [self.em_size, self.mark_size];
        if nonnegative.iter().all(|v| v.is_finite() && *v >= 0.0)
            && positive.iter().all(|v| v.is_finite() && *v > 0.0)
        {
            Ok(())
        } else {
            Err(LayoutError::Options)
        }
    }
}

/// Final text box, with baseline information for a renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelBox {
    /// Bounds in the scene's y-up coordinate system.
    pub bounds: BBox,
    /// Inline origin of the first baseline in scene coordinates.
    pub baseline: Point,
    /// Actual renderer output, including mixed segments and multiline content.
    pub measured: TexBox,
    /// Scale to apply to the measured SVG body (also flip its y axis).
    pub em_size: f64,
}

/// A connector or annotation stroke path.
#[derive(Clone, Debug, PartialEq)]
pub enum Leader {
    /// No leader for labels by default, centered text, or local marks.
    None,
    /// Connector from the anchor to the nearest edge of the text box.
    Segment(Segment),
    /// Accepted orthogonal arrow centerline.
    Routed {
        /// Shared logical route geometry.
        route: locus_core::geom::route::Route,
        /// Original arrow-head style.
        head: locus_syntax::HeadKind,
    },
    /// Arrow path; `control` is a quadratic Bézier control point when bent.
    Arrow {
        /// Resolved start anchor.
        from: Point,
        /// Resolved end anchor.
        to: Point,
        /// Optional quadratic control point.
        control: Option<Point>,
        /// Original arrow-head style.
        head: locus_syntax::HeadKind,
    },
}

/// One fully positioned annotation.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedAnnotation {
    /// Identifier and original source span.
    pub id: Ident,
    /// Original semantic annotation, including text and anchor spans.
    pub spec: AnnotationSpec,
    /// Final bounds (text bounds or annotation stroke bounds).
    pub bounds: BBox,
    /// Measured text for labels and text blocks.
    pub text: Option<LabelBox>,
    /// Resolved connector information.
    pub leader: Leader,
    /// Tick or right-angle strokes; arrow paths are stored in `leader`.
    pub strokes: Vec<Segment>,
    /// Selected candidate index; zero for pinned and non-text annotations.
    pub candidate_index: usize,
}

/// Layout output, retaining declaration order independently of assignment order.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutScene {
    /// Render-only retained paths, preserving logical object measurements.
    pub visible_paths: std::collections::BTreeMap<String, locus_core::geom::route::Route>,
    /// Every annotation, keyed by name in declaration order.
    pub annotations: IndexMap<String, PlacedAnnotation>,
    /// Unchanged geometry-only bounds.
    pub geometry_bbox: BBox,
    /// Geometry and annotation bounds combined (before renderer viewport policy).
    pub bbox: BBox,
    /// Residual collisions, suitable for extending an inspection report.
    pub diagnostics: Vec<InspectDiagnostic>,
}

/// Stable layout failures. E401–E404 remain reserved for existing core errors.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum LayoutError {
    /// A required measurement was omitted.
    #[error("E410: missing text measurement for {id}")]
    MissingMeasurement {
        /// Annotation name.
        id: String,
        /// Annotation source location.
        span: Span,
    },
    /// An anchor is missing or participates in an annotation dependency cycle.
    #[error("E411: unresolved annotation anchor {id}")]
    Anchor {
        /// Referenced annotation name.
        id: String,
        /// Anchor expression's source location.
        span: Span,
    },
    /// A measurement or position is not finite and valid.
    #[error("E412: invalid layout metrics or coordinates for {id}")]
    Invalid {
        /// Annotation name.
        id: String,
        /// Annotation source location.
        span: Span,
    },
    /// Global style options violate their domains.
    #[error("E413: layout options must be finite and in their documented domains")]
    Options,
    /// Unsatisfied route, clipping, or measurement feedback requirement.
    #[error("E413: {message}")]
    Conflict {
        /// Original requirement source span.
        span: Span,
        /// Implicated route or label identifiers.
        measurements: Vec<String>,
        /// Concrete conflict explanation.
        message: String,
    },
    /// Actual text renderer failure, anchored to its text value.
    #[error("E414: text measurement failed for {id}: {source}")]
    Measurement {
        /// Annotation name.
        id: String,
        /// Text source location.
        span: Span,
        /// Underlying engine error.
        source: locus_tex::TexError,
    },
}

impl LayoutError {
    /// Stable machine-readable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::MissingMeasurement { .. } => "E410",
            Self::Anchor { .. } => "E411",
            Self::Invalid { .. } => "E412",
            Self::Options | Self::Conflict { .. } => "E413",
            Self::Measurement { .. } => "E414",
        }
    }

    pub(crate) fn invalid(id: &Ident) -> Self {
        Self::Invalid {
            id: id.name.clone(),
            span: id.span,
        }
    }
}

/// Places all annotations without modifying or re-evaluating geometry.
///
/// # Errors
/// Returns a stable [`LayoutError`] for missing/invalid measurements, invalid
/// options, non-finite placement, or unresolved/cyclic annotation anchors.
pub fn layout(
    scene: &ResolvedScene,
    measured: &TextBoxes,
    options: LayoutOptions,
) -> Result<LayoutScene, LayoutError> {
    options.validate()?;
    assign::assign(scene, measured, options)
}
