//! Measured text composition and final annotation box geometry.

use locus_core::{AnnotationSpec, BBox, Point, ResolvedObject, ResolvedScene, Segment, Vec2};
use locus_syntax::{Ident, TextSegmentKind};
use locus_tex::{FontSize, TexBox, TexEngine, TextSegment, render_segments};

use crate::{LabelBox, LayoutError, LayoutOptions, Leader, PlacedAnnotation, TextBoxes};

/// Measures every text annotation with its actual plain-text and TeX engines.
///
/// # Errors
/// Returns a source-anchored error if segment composition or rendering fails.
pub fn measure_scene(
    scene: &ResolvedScene,
    plain: &dyn TexEngine,
    tex: &dyn TexEngine,
    size: FontSize,
) -> Result<TextBoxes, LayoutError> {
    let mut result = TextBoxes::new();
    for (id, object) in &scene.objects {
        let Some(text) = annotation_text(object) else {
            continue;
        };
        let segments: Vec<_> = text
            .segments
            .iter()
            .map(|segment| match segment.kind {
                TextSegmentKind::Plain => TextSegment::Plain(&segment.source),
                TextSegmentKind::Tex => TextSegment::Tex(&segment.source),
            })
            .collect();
        let measured = render_segments(&segments, plain, tex, size).map_err(|source| {
            LayoutError::Measurement {
                id: id.name.clone(),
                span: text.span,
                source,
            }
        })?;
        result.insert(id.name.clone(), measured);
    }
    Ok(result)
}

pub(crate) fn annotation_text(object: &ResolvedObject) -> Option<&locus_syntax::TextValue> {
    match object {
        ResolvedObject::Annotation(
            AnnotationSpec::Label { text, .. } | AnnotationSpec::TextBlock { text, .. },
        ) => Some(text),
        _ => None,
    }
}

pub(crate) fn dimensions(id: &Ident, measured: &TexBox, scale: f64) -> Result<Vec2, LayoutError> {
    let metrics = measured.metrics;
    let values = [
        metrics.width,
        metrics.height,
        metrics.depth,
        measured.baseline_x,
    ];
    let size = Vec2::new(
        metrics.width * scale,
        (metrics.height + metrics.depth) * scale,
    );
    if values.iter().all(|v| v.is_finite() && *v >= 0.0)
        && measured.advance.is_finite()
        && size.is_finite()
    {
        Ok(size)
    } else {
        Err(LayoutError::invalid(id))
    }
}

pub(crate) fn centered(center: Point, size: Vec2) -> BBox {
    BBox {
        min: center - size * 0.5,
        max: center + size * 0.5,
    }
}

pub(crate) fn expand(bounds: BBox, amount: f64) -> BBox {
    let offset = Vec2::new(amount, amount);
    BBox {
        min: bounds.min - offset,
        max: bounds.max + offset,
    }
}

pub(crate) fn valid_bounds(bounds: BBox) -> bool {
    bounds.min.is_finite()
        && bounds.max.is_finite()
        && bounds.width().is_finite()
        && bounds.height().is_finite()
        && bounds.width() >= 0.0
        && bounds.height() >= 0.0
}

pub(crate) fn text_box(bounds: BBox, measured: &TexBox, scale: f64) -> LabelBox {
    LabelBox {
        bounds,
        baseline: Point::new(
            bounds.min.x + measured.baseline_x * scale,
            bounds.max.y - measured.metrics.height * scale,
        ),
        measured: measured.clone(),
        em_size: scale,
    }
}

pub(crate) fn connector(anchor: Point, bounds: BBox) -> Leader {
    let edge = Point::new(
        anchor.x.clamp(bounds.min.x, bounds.max.x),
        anchor.y.clamp(bounds.min.y, bounds.max.y),
    );
    if anchor == edge {
        Leader::None
    } else {
        Leader::Segment(Segment {
            start: anchor,
            end: edge,
        })
    }
}

pub(crate) fn mark(
    id: &Ident,
    spec: &AnnotationSpec,
    anchors: &[Point],
    options: LayoutOptions,
) -> Result<PlacedAnnotation, LayoutError> {
    let (strokes, leader) = match spec {
        AnnotationSpec::Tick { segment, count } => {
            (ticks(*segment, *count, options.mark_size), Leader::None)
        }
        AnnotationSpec::RightAngle {
            vertex,
            first,
            second,
            size,
        } => (
            right_angle(*vertex, *first, *second, size.unwrap_or(options.mark_size)),
            Leader::None,
        ),
        AnnotationSpec::Arrow { head, bend, .. } => {
            (Vec::new(), arrow(anchors[0], anchors[1], *head, *bend))
        }
        _ => return Err(LayoutError::invalid(id)),
    };
    let mut points: Vec<_> = strokes.iter().flat_map(|s| [s.start, s.end]).collect();
    if let Leader::Arrow {
        from, to, control, ..
    } = &leader
    {
        points.extend([*from, *to]);
        points.extend(*control);
    }
    let bounds = BBox::from_points(points).map_err(|_| LayoutError::invalid(id))?;
    let bounds = expand(bounds, options.stroke_width / 2.0);
    if !valid_bounds(bounds) {
        return Err(LayoutError::invalid(id));
    }
    Ok(PlacedAnnotation {
        id: id.clone(),
        spec: spec.clone(),
        bounds,
        text: None,
        leader,
        strokes,
        candidate_index: 0,
    })
}

fn ticks(segment: Segment, count: u8, size: f64) -> Vec<Segment> {
    let direction = unit(segment.direction());
    let normal = direction.perpendicular() * (size / 2.0);
    (0..count)
        .map(|index| {
            let offset = (f64::from(index) - (f64::from(count) - 1.0) / 2.0) * size * 0.4;
            let center = segment.midpoint() + direction * offset;
            Segment {
                start: center - normal,
                end: center + normal,
            }
        })
        .collect()
}

fn right_angle(vertex: Point, first: Point, second: Point, size: f64) -> Vec<Segment> {
    let first = unit(first - vertex) * size;
    let second = unit(second - vertex) * size;
    let corner = vertex + first + second;
    vec![
        Segment {
            start: vertex + first,
            end: corner,
        },
        Segment {
            start: corner,
            end: vertex + second,
        },
    ]
}

fn arrow(from: Point, to: Point, head: locus_syntax::HeadKind, bend: f64) -> Leader {
    // A quadratic whose initial tangent is rotated by `bend` from the chord.
    // Control-hull bounds conservatively contain the full curve.
    let control = (bend != 0.0)
        .then(|| from + (to - from) * 0.5 + (to - from).perpendicular() * (libm::tan(bend) * 0.5));
    Leader::Arrow {
        from,
        to,
        control,
        head,
    }
}

pub(crate) fn unit(vector: Vec2) -> Vec2 {
    let length = vector.norm();
    if length > 0.0 {
        vector * (1.0 / length)
    } else {
        Vec2::new(0.0, 0.0)
    }
}
