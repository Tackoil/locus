//! Visible-content fitting and the Cartesian-to-SVG coordinate transform.

use crate::{RenderError, RenderOptions, style::StyleSheet};
use locus_core::{BBox, Point, PointMark, ResolvedObject, ResolvedScene, Vec2};
use locus_layout::{LayoutScene, Leader, PlacedAnnotation};

/// Padded scene-space bounds and physical scale, shared by all backends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Padded Cartesian bounds, before y-flip.
    pub bounds: BBox,
    /// CSS pixels per scene unit.
    pub pixels_per_unit: f64,
}

impl Viewport {
    /// Fits visible geometry and final annotation boxes, never cached bounds
    /// that may include invisible anchor points. An empty/flat axis gets one
    /// scene unit of extent before padding. Stroke and disk extents are included.
    ///
    /// # Errors
    /// Returns E415 for invalid content bounds, E416 for invalid options/extents.
    pub fn fit(
        scene: &ResolvedScene,
        layout: &LayoutScene,
        styles: &StyleSheet,
        options: RenderOptions,
    ) -> Result<Self, RenderError> {
        options.validate()?;
        let mut bounds = geometry_bounds(scene, styles, options)?;
        for annotation in layout.annotations.values() {
            include_annotation(&mut bounds, annotation, styles, options)?;
        }
        let mut bounds = bounds.unwrap_or(BBox {
            min: Point::new(-0.5, -0.5),
            max: Point::new(0.5, 0.5),
        });
        if bounds.width() == 0.0 {
            bounds.min.x -= 0.5;
            bounds.max.x += 0.5;
        }
        if bounds.height() == 0.0 {
            bounds.min.y -= 0.5;
            bounds.max.y += 0.5;
        }
        let viewport = Self {
            bounds: expand(bounds, options.margin),
            pixels_per_unit: options.pixels_per_unit,
        };
        if !valid(viewport.bounds)
            || !viewport.width().is_finite()
            || !viewport.height().is_finite()
            || viewport.width() <= 0.0
            || viewport.height() <= 0.0
            || crate::svg::format_float(viewport.width()) == "0.000000"
            || crate::svg::format_float(viewport.height()) == "0.000000"
        {
            return Err(RenderError::Options);
        }
        Ok(viewport)
    }

    /// Physical viewport width in CSS pixels.
    #[must_use]
    pub fn width(self) -> f64 {
        self.bounds.width() * self.pixels_per_unit
    }
    /// Physical viewport height in CSS pixels.
    #[must_use]
    pub fn height(self) -> f64 {
        self.bounds.height() * self.pixels_per_unit
    }
    /// Maps y-up scene coordinates to y-down SVG pixel coordinates.
    #[must_use]
    pub fn to_svg(self, point: Point) -> Point {
        Point::new(
            (point.x - self.bounds.min.x) * self.pixels_per_unit,
            (self.bounds.max.y - point.y) * self.pixels_per_unit,
        )
    }
}

pub(crate) fn valid(bounds: BBox) -> bool {
    bounds.min.is_finite()
        && bounds.max.is_finite()
        && bounds.width().is_finite()
        && bounds.height().is_finite()
        && bounds.width() >= 0.0
        && bounds.height() >= 0.0
}

fn expand(bounds: BBox, amount: f64) -> BBox {
    let delta = Vec2::new(amount, amount);
    BBox {
        min: bounds.min - delta,
        max: bounds.max + delta,
    }
}

fn include(
    bounds: &mut Option<BBox>,
    next: BBox,
    span: locus_syntax::Span,
) -> Result<(), RenderError> {
    if !valid(next) {
        return Err(RenderError::invalid(span, "invalid visible bounds"));
    }
    if let Some(current) = bounds {
        current.include_bbox(next);
    } else {
        *bounds = Some(next);
    }
    Ok(())
}

fn geometry_bounds(
    scene: &ResolvedScene,
    styles: &StyleSheet,
    options: RenderOptions,
) -> Result<Option<BBox>, RenderError> {
    let mut bounds = None;
    for (id, object) in &scene.objects {
        let style = styles.for_object(&id.name);
        if style.opacity == 0.0
            || matches!(object, ResolvedObject::Point(p) if p.mark == PointMark::None)
        {
            continue;
        }
        if let Some(bbox) = object.bbox() {
            let padding = geometry_padding(object, style.weight) / options.pixels_per_unit;
            include(&mut bounds, expand(bbox, padding), id.span)?;
        }
    }
    Ok(bounds)
}

fn geometry_padding(object: &ResolvedObject, weight: f64) -> f64 {
    let extra = match object {
        ResolvedObject::Point(_) => 2.0,
        ResolvedObject::Arc(arc) => {
            f64::from(arc.angle_mark_count.unwrap_or(1).saturating_sub(1)) * 3.0
        }
        _ => 0.0,
    };
    extra + weight / 2.0
}

fn include_annotation(
    bounds: &mut Option<BBox>,
    annotation: &PlacedAnnotation,
    styles: &StyleSheet,
    options: RenderOptions,
) -> Result<(), RenderError> {
    let style = styles.for_object(&annotation.id.name);
    if style.opacity == 0.0 {
        return Ok(());
    }
    let span = annotation.id.span;
    include(bounds, annotation.bounds, span)?;
    if let Some(text) = &annotation.text {
        include(bounds, text.bounds, span)?;
    }
    let mut stroke_bounds = None;
    for stroke in &annotation.strokes {
        include(&mut stroke_bounds, BBox::for_segment(*stroke), span)?;
    }
    let head_padding = include_leader(&mut stroke_bounds, &annotation.leader, span, style.weight)?;
    if let Some(strokes) = stroke_bounds {
        include(
            bounds,
            expand(
                strokes,
                (head_padding + style.weight / 2.0) / options.pixels_per_unit,
            ),
            span,
        )?;
    }
    Ok(())
}

fn include_leader(
    bounds: &mut Option<BBox>,
    leader: &Leader,
    span: locus_syntax::Span,
    weight: f64,
) -> Result<f64, RenderError> {
    match leader {
        Leader::None => Ok(0.0),
        Leader::Segment(segment) => {
            include(bounds, BBox::for_segment(*segment), span)?;
            Ok(0.0)
        }
        Leader::Arrow {
            from,
            to,
            control,
            head,
        } => {
            if let Some(control) = control {
                locus_core::geom::arrow::validate_control(*from, *to, *control)
                    .map_err(|error| RenderError::invalid(span, error.to_string()))?;
            }
            let points = [Some(*from), Some(*to), *control].into_iter().flatten();
            let bbox =
                BBox::from_points(points).map_err(|e| RenderError::invalid(span, e.to_string()))?;
            include(bounds, bbox, span)?;
            Ok(if *head == locus_syntax::HeadKind::None {
                0.0
            } else {
                9.0 + weight * 1.5
            })
        }
    }
}
