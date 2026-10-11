//! Renderer-independent measured inputs for scene evaluation.

use std::collections::BTreeMap;

use crate::preset::LayoutMetrics;

/// Typography selected before measuring the content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeasuredRole {
    /// Ordinary content typography.
    #[default]
    Normal,
    /// Preset title typography.
    Title,
}

/// Immutable measured text dimensions in scene units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeasuredText {
    /// Full composed width.
    pub width: f64,
    /// Full composed height, including baseline depth.
    pub height: f64,
    /// Typography role used by the measuring engine.
    pub role: MeasuredRole,
}

/// Measured content keyed by expanded, possibly qualified declaration id.
pub type MeasuredContent = BTreeMap<String, MeasuredText>;

/// A frozen attached-label candidate selected by the layout pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelChoice {
    /// Page side of the text box.
    pub side: crate::geom::route::RouteSide,
    /// Deterministic candidate index within the requested region.
    pub candidate: usize,
}

/// Renderer-independent choices frozen during each continuous solve.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EvaluationChoices {
    /// Accepted route sides and topologies, keyed by path declaration.
    pub routes: BTreeMap<String, crate::geom::route::RouteChoice>,
    /// Minimum continuous detours required by discrete obstacle search.
    pub route_min_offsets: BTreeMap<String, f64>,
    /// Visible parameter intervals after endpoint clipping.
    pub retained_routes: BTreeMap<String, (f64, f64)>,
    /// Physical arc lengths removed at the tail and tip of retained paths.
    pub retained_distances: BTreeMap<String, (f64, f64)>,
    /// Attached-label candidates, keyed by label declaration.
    pub labels: BTreeMap<String, LabelChoice>,
}

/// External inputs to the geometry engine, independent of font libraries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EvaluationInputs {
    /// Text extents measured before solving.
    pub measured: MeasuredContent,
    /// Shared scene-space preset constants.
    pub metrics: LayoutMetrics,
    /// Discrete route and retained-label choices supplied by layout feedback.
    pub choices: EvaluationChoices,
}

impl EvaluationInputs {
    pub(crate) fn validate(&self) -> Result<(), crate::EvalError> {
        self.choices.validate()?;
        let metrics = self.metrics;
        let positive = [
            metrics.default_box_width,
            metrics.default_box_height,
            metrics.em_size,
            metrics.short_length,
            metrics.narrow_width,
            metrics.normal_font_size,
            metrics.title_font_size,
            metrics.mark_size,
        ];
        let nonnegative = [
            metrics.padding,
            metrics.gap,
            metrics.clearance,
            metrics.stroke_width,
            metrics.margin,
        ];
        if positive
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || nonnegative
                .iter()
                .any(|value| !value.is_finite() || *value < 0.0)
            || !metrics.rounded_radius_fraction.is_finite()
            || !(0.0..=0.5).contains(&metrics.rounded_radius_fraction)
        {
            return Err(crate::EvalError::new(
                "E413",
                locus_syntax::Span::default(),
                "invalid layout preset metrics",
            ));
        }
        Ok(())
    }
}

impl EvaluationChoices {
    fn validate(&self) -> Result<(), crate::EvalError> {
        let invalid_offset = self
            .route_min_offsets
            .values()
            .any(|v| !v.is_finite() || *v < 0.0);
        let invalid_range = self.retained_routes.values().any(|(lo, hi)| {
            !lo.is_finite() || !hi.is_finite() || *lo < 0.0 || *hi > 1.0 || lo >= hi
        });
        let invalid_label = self.labels.values().any(|choice| choice.candidate > 2);
        let invalid_distance = self.retained_distances.values().any(|(start, end)| {
            !start.is_finite() || !end.is_finite() || *start < 0.0 || *end < 0.0
        });
        if invalid_offset || invalid_range || invalid_label || invalid_distance {
            return Err(crate::EvalError::new(
                "E413",
                locus_syntax::Span::default(),
                "invalid frozen layout choices",
            ));
        }
        Ok(())
    }
}
