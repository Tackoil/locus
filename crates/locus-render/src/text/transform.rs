//! SVG transform composition with pinned libm trigonometry.
use crate::RenderError;
use locus_core::Point;
use locus_syntax::Span;
use svgtypes::{TransformListParser, TransformListToken};
pub(super) type Matrix = [f64; 6];
const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub(super) fn multiply(a: Matrix, b: Matrix) -> Matrix {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

pub(super) fn map(m: Matrix, p: Point) -> Point {
    Point::new(
        m[0] * p.x + m[2] * p.y + m[4],
        m[1] * p.x + m[3] * p.y + m[5],
    )
}

pub(super) fn transforms(value: &str, span: Span) -> Result<Matrix, RenderError> {
    let mut result = IDENTITY;
    for token in TransformListParser::from(value) {
        let token = token.map_err(|e| RenderError::invalid(span, e.to_string()))?;
        result = multiply(result, transform_token(token));
    }
    Ok(result)
}

fn transform_token(token: TransformListToken) -> Matrix {
    match token {
        TransformListToken::Matrix { a, b, c, d, e, f } => [a, b, c, d, e, f],
        TransformListToken::Translate { tx, ty } => [1.0, 0.0, 0.0, 1.0, tx, ty],
        TransformListToken::Scale { sx, sy } => [sx, 0.0, 0.0, sy, 0.0, 0.0],
        TransformListToken::Rotate { angle } => {
            let angle = angle * core::f64::consts::PI / 180.0;
            let (s, c) = (libm::sin(angle), libm::cos(angle));
            [c, s, -s, c, 0.0, 0.0]
        }
        TransformListToken::SkewX { angle } => [
            1.0,
            0.0,
            libm::tan(angle * core::f64::consts::PI / 180.0),
            1.0,
            0.0,
            0.0,
        ],
        TransformListToken::SkewY { angle } => [
            1.0,
            libm::tan(angle * core::f64::consts::PI / 180.0),
            0.0,
            1.0,
            0.0,
            0.0,
        ],
    }
}
