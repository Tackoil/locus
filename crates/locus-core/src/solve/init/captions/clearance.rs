//! Obstacle reach at guide lines and local loop-caption spacing.
use super::{BBox, Expr, Vec2, carrier_spacing, find_object};
use crate::{ResolvedObject, preset::LayoutMetrics};

pub(super) fn carrier_padding(
    trial: &crate::eval::TrialEvaluation,
    carriers: &[String; 2],
    obstacles: &[Expr],
    normal: Vec2,
) -> [f64; 2] {
    std::array::from_fn(|side| {
        let Some(carrier) = find_object(trial, &carriers[side]).and_then(ResolvedObject::bbox)
        else {
            return 0.0;
        };
        let coordinate = projection(carrier, normal).0;
        obstacles
            .iter()
            .filter_map(|expression| obstacle_bounds(trial, expression))
            .filter_map(|bounds| reach(bounds, normal, coordinate, side == 0))
            .fold(0.0, f64::max)
    })
}

fn obstacle_bounds(trial: &crate::eval::TrialEvaluation, expression: &Expr) -> Option<BBox> {
    let Expr::Ref(id) = expression else {
        return None;
    };
    match find_object(trial, &id.name)? {
        object @ (ResolvedObject::Rect(_)
        | ResolvedObject::RoundedRect(_)
        | ResolvedObject::Diamond(_)) => object.bbox(),
        _ => None,
    }
}

fn projection(bounds: BBox, direction: Vec2) -> (f64, f64) {
    let center = bounds.center();
    let center = center.x * direction.x + center.y * direction.y;
    let half = (direction.x.abs() * bounds.width()).midpoint(direction.y.abs() * bounds.height());
    (center, half)
}

fn reach(bounds: BBox, normal: Vec2, coordinate: f64, forward: bool) -> Option<f64> {
    let (center, half) = projection(bounds, normal);
    if (center - coordinate).abs() > half + 1e-10 {
        return None;
    }
    Some(if forward {
        center + half - coordinate
    } else {
        coordinate - center + half
    })
}

pub(super) fn loop_spacing(
    trial: &crate::eval::TrialEvaluation,
    carrier: &str,
    caption: BBox,
    obstacles: &[Expr],
    metrics: LayoutMetrics,
) -> Option<([String; 2], Vec2, f64)> {
    obstacles
        .iter()
        .filter_map(|expression| {
            let Expr::Ref(id) = expression else {
                return None;
            };
            if id.name == carrier {
                return None;
            }
            let pair = [carrier.to_owned(), id.name.clone()];
            let (normal, gap) = carrier_spacing(trial, &pair)?;
            let guide = find_object(trial, &pair[0])?.bbox()?;
            let obstacle = find_object(trial, &pair[1])?.bbox()?;
            if !overlaps_along(caption, obstacle, normal.perpendicular(), metrics.clearance) {
                return None;
            }
            let (center, half) = projection(caption, normal);
            let source = projection(guide, normal).0;
            let extra =
                center + half - source + metrics.clearance + metrics.stroke_width / 2.0 - gap;
            (extra > 0.0).then_some((pair, normal, extra))
        })
        .max_by(|a, b| a.2.total_cmp(&b.2))
}

fn overlaps_along(a: BBox, b: BBox, direction: Vec2, padding: f64) -> bool {
    let (a, ah) = projection(a, direction);
    let (b, bh) = projection(b, direction);
    (a - b).abs() <= ah + bh + padding
}
