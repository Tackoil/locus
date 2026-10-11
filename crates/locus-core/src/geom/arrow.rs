//! Bounded quadratic arrow construction, shared by evaluation, layout, and export.

use super::{ConstructError, Point, validate_finite_points, validate_finite_result};

// The control point may extend at most 500 chord lengths from the midpoint.
const MAX_BEND_TANGENT: f64 = 1000.0;

/// Returns the tangent of an arrow bend in radians, rejecting singular and
/// near-singular angles where `|tan(bend)| > 1000` (about 0.0573 degrees from
/// an odd multiple of 90 degrees).
pub fn bend_tangent(bend: f64) -> Result<f64, ConstructError> {
    if !bend.is_finite() {
        return Err(ConstructError::NonFinite {
            operation: "arrow bend",
        });
    }
    let tangent = libm::tan(bend);
    if !tangent.is_finite() || tangent.abs() > MAX_BEND_TANGENT {
        return Err(ConstructError::ArrowBend);
    }
    Ok(tangent)
}

/// Constructs a bounded quadratic control point; a zero bend uses a straight
/// segment. The initial tangent is rotated from the chord by `bend`, modulo pi.
pub fn control(from: Point, to: Point, bend: f64) -> Result<Option<Point>, ConstructError> {
    let tangent = bend_tangent(bend)?;
    validate_finite_points("arrow", &[from, to])?;
    if bend == 0.0 {
        return Ok(None);
    }
    let chord = to - from;
    let control = validate_finite_result(
        "arrow",
        from + chord * 0.5 + chord.perpendicular() * (tangent * 0.5),
    )?;
    validate_control(from, to, control)?;
    Ok(Some(control))
}

/// Rejects non-finite or excessive control-point bounds before exporting a
/// supplied layout. The limit matches [`bend_tangent`], with rounding slack.
pub fn validate_control(from: Point, to: Point, control: Point) -> Result<(), ConstructError> {
    validate_finite_points("arrow", &[from, to, control])?;
    let chord = to - from;
    let displacement = control - (from + chord * 0.5);
    let offset = libm::hypot(displacement.x, displacement.y);
    let length = libm::hypot(chord.x, chord.y);
    if !offset.is_finite() || !length.is_finite() {
        return Err(ConstructError::NonFinite {
            operation: "arrow bounds",
        });
    }
    if offset == 0.0 || offset / length <= MAX_BEND_TANGENT * 0.5 * (1.0 + 1e-12) {
        Ok(())
    } else {
        Err(ConstructError::Domain {
            operation: "arrow bounds",
            reason: "control point is too far from the chord; use a gentler bend such as 45deg",
        })
    }
}
