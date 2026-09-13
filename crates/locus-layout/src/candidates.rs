//! Ordered compass candidates and preferences inferred from resolved geometry.
//!
//! Geometry anchors have already been reduced to points by core. Preferences
//! therefore use geometric incidence, with core's shared tolerance. Preferences
//! precede compass candidates; each group uses the near radius before the far
//! radius. Compass order is N, NE, E, SE, S, SW, W, NW.

use crate::boxes::{centered, unit};
use locus_core::scene::{CompassDirection, LabelDirection};
use locus_core::{BBox, Point, ResolvedObject, ResolvedScene, Segment, Tolerance, Vec2};

/// One ordered placement possibility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// Final measured box bounds.
    pub bounds: BBox,
}

/// Generates domain preferences plus 8 directions × 2 gap radii.
///
/// Explicit directions return exactly one candidate. `Center` ignores `gap`.
/// The far gap is `2 * gap + half the larger box dimension`, so the two
/// radii remain distinct even when a caller requests zero gap.
#[must_use]
pub fn generate(
    scene: &ResolvedScene,
    anchor: Point,
    size: Vec2,
    direction: LabelDirection,
    gap: f64,
) -> Vec<Candidate> {
    match direction {
        LabelDirection::Center => vec![Candidate {
            bounds: centered(anchor, size),
        }],
        LabelDirection::Compass(compass) => {
            vec![offset(anchor, size, compass_vector(compass), gap)]
        }
        LabelDirection::Auto => automatic(scene, anchor, size, gap),
    }
}

fn automatic(scene: &ResolvedScene, anchor: Point, size: Vec2, gap: f64) -> Vec<Candidate> {
    let preferences = preferred(scene, anchor);
    let compass = [
        CompassDirection::N,
        CompassDirection::Ne,
        CompassDirection::E,
        CompassDirection::Se,
        CompassDirection::S,
        CompassDirection::Sw,
        CompassDirection::W,
        CompassDirection::Nw,
    ];
    let mut result = Vec::new();
    let radii = [gap, 2.0 * gap + size.x.max(size.y) / 2.0];
    for radius in radii {
        result.extend(preferences.iter().map(|v| offset(anchor, size, *v, radius)));
    }
    for radius in radii {
        result.extend(
            compass
                .iter()
                .map(|d| offset(anchor, size, compass_vector(*d), radius)),
        );
    }
    result
}

fn offset(anchor: Point, size: Vec2, direction: Vec2, gap: f64) -> Candidate {
    let direction = unit(direction);
    // Distance to the rectangle boundary along the selected direction.
    let x = if direction.x == 0.0 {
        f64::INFINITY
    } else {
        size.x / (2.0 * direction.x.abs())
    };
    let y = if direction.y == 0.0 {
        f64::INFINITY
    } else {
        size.y / (2.0 * direction.y.abs())
    };
    Candidate {
        bounds: centered(anchor + direction * (x.min(y) + gap), size),
    }
}

fn compass_vector(direction: CompassDirection) -> Vec2 {
    // The core enum is declared in this same clockwise compass order.
    const VECTORS: [(f64, f64); 8] = [
        (0.0, 1.0),
        (1.0, 1.0),
        (1.0, 0.0),
        (1.0, -1.0),
        (0.0, -1.0),
        (-1.0, -1.0),
        (-1.0, 0.0),
        (-1.0, 1.0),
    ];
    let (x, y) = VECTORS[direction as usize];
    Vec2::new(x, y)
}

fn preferred(scene: &ResolvedScene, anchor: Point) -> Vec<Vec2> {
    let tolerance = Tolerance::from_bbox(&scene.bbox);
    let mut directions = Vec::new();
    let mut incident = Vec::new();
    for object in scene.objects.values() {
        geometry_preferences(object, anchor, tolerance, &mut directions, &mut incident);
    }
    if incident.len() == 2 {
        let outward = (incident[0] + incident[1]) * -1.0;
        if !tolerance.scalar_zero(outward.norm(), 0.0, 1.0) {
            directions.insert(0, unit(outward));
        }
    }
    directions
}

fn geometry_preferences(
    object: &ResolvedObject,
    anchor: Point,
    tolerance: Tolerance,
    directions: &mut Vec<Vec2>,
    incident: &mut Vec<Vec2>,
) {
    match object {
        ResolvedObject::Segment(segment) => {
            segment_preferences(*segment, anchor, tolerance, directions, incident);
        }
        ResolvedObject::Polygon(polygon)
        | ResolvedObject::Rect(polygon)
        | ResolvedObject::Diamond(polygon) => {
            let vertices = polygon.vertices();
            for index in 0..vertices.len() {
                segment_preferences(
                    Segment {
                        start: vertices[index],
                        end: vertices[(index + 1) % vertices.len()],
                    },
                    anchor,
                    tolerance,
                    directions,
                    incident,
                );
            }
        }
        ResolvedObject::Circle(circle) => {
            radial_preference(circle.center, circle.radius, anchor, tolerance, directions);
        }
        ResolvedObject::Arc(arc) => arc_preferences(*arc, anchor, tolerance, directions, incident),
        _ => {}
    }
}

fn arc_preferences(
    arc: locus_core::ResolvedArc,
    anchor: Point,
    tolerance: Tolerance,
    directions: &mut Vec<Vec2>,
    incident: &mut Vec<Vec2>,
) {
    let geometry = arc.geometry;
    radial_preference(
        geometry.circle.center,
        geometry.circle.radius,
        anchor,
        tolerance,
        directions,
    );
    if arc.angle_mark_count.is_some() && tolerance.points_eq(geometry.circle.center, anchor) {
        push_incident(incident, unit(geometry.start - anchor), tolerance);
        push_incident(incident, unit(geometry.end - anchor), tolerance);
    }
}

fn radial_preference(
    center: Point,
    radius: f64,
    anchor: Point,
    tolerance: Tolerance,
    directions: &mut Vec<Vec2>,
) {
    let radial = anchor - center;
    if tolerance.length_zero(radial.norm() - radius) && radius > 0.0 {
        directions.push(unit(radial));
    }
}

fn segment_preferences(
    segment: Segment,
    anchor: Point,
    tolerance: Tolerance,
    directions: &mut Vec<Vec2>,
    incident: &mut Vec<Vec2>,
) {
    let direction = unit(segment.direction());
    if direction.norm_squared() == 0.0 {
        return;
    }
    if tolerance.points_eq(segment.start, anchor) {
        push_incident(incident, direction, tolerance);
    }
    if tolerance.points_eq(segment.end, anchor) {
        push_incident(incident, direction * -1.0, tolerance);
    }
    if tolerance.points_eq(segment.midpoint(), anchor) {
        directions.extend([direction.perpendicular(), direction.perpendicular() * -1.0]);
    }
}

fn push_incident(incident: &mut Vec<Vec2>, direction: Vec2, tolerance: Tolerance) {
    if !incident
        .iter()
        .any(|v| tolerance.scalar_zero((*v - direction).norm(), 0.0, 1.0))
    {
        incident.push(direction);
    }
}
