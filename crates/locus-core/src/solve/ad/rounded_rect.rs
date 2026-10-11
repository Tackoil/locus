//! Differentiated rounded-rectangle dimensions, measurements, and anchors.
use super::{
    DObject, DPoint, DValue, DVector, EvalError, Expr, Jet, NumericDimension, RelationEvaluator,
    Span, polygon_property, rect_vertices, require_arity, solver_error,
};

#[derive(Clone, Debug)]
pub(crate) struct DRoundedRect {
    pub(super) center: DPoint,
    pub(super) width: Jet,
    pub(super) height: Jet,
    pub(super) radius: Jet,
}

impl RelationEvaluator<'_> {
    pub(super) fn rounded_rect_at(
        &mut self,
        center: DPoint,
        args: &[Expr],
        span: Span,
    ) -> Result<DObject, EvalError> {
        require_arity("rounded_rect dimensions", args, 3, span)?;
        Ok(DObject::RoundedRect(DRoundedRect {
            center,
            width: self.eval_number_as(&args[0], NumericDimension::Length)?.0,
            height: self.eval_number_as(&args[1], NumericDimension::Length)?.0,
            radius: self.eval_number_as(&args[2], NumericDimension::Length)?.0,
        }))
    }
}

impl DRoundedRect {
    /// Straight support faces bind only when padding reaches the rounding radius.
    /// Curved-distance rows retain the exact containment requirement everywhere.
    pub(crate) fn containment_face_padding(&self, padding: f64) -> Jet {
        let padding = Jet::constant(padding, self.radius.gradient.len());
        let zero = Jet::constant(0.0, self.radius.gradient.len());
        let slack = zero.max_live(&self.radius.sub(&padding));
        padding.sub(&slack)
    }

    pub(super) fn automatic(center: DPoint, width: Jet, height: Jet, radius: Jet) -> Self {
        Self {
            center,
            width,
            height,
            radius,
        }
    }
    pub(super) fn measure(&self, name: &str) -> DValue {
        let (value, dimension) = if name == "area" {
            (
                self.width.mul(&self.height).sub(
                    &self
                        .radius
                        .mul(&self.radius)
                        .scale(4.0 - core::f64::consts::PI),
                ),
                NumericDimension::Area,
            )
        } else {
            (
                self.width
                    .add(&self.height)
                    .scale(2.0)
                    .sub(&self.radius.scale(8.0 - core::f64::consts::TAU)),
                NumericDimension::Length,
            )
        };
        DValue::Number(value, dimension)
    }

    pub(super) fn property(
        &self,
        name: &str,
        span: Span,
        count: usize,
    ) -> Result<DValue, EvalError> {
        match name {
            "corner_radius" => Ok(DValue::Number(
                self.radius.clone(),
                NumericDimension::Length,
            )),
            "center" | "centroid" => Ok(DValue::Object(DObject::Point(self.center.clone()))),
            _ => polygon_property(
                &rect_vertices(&self.center, &self.width, &self.height),
                true,
                name,
                span,
                count,
            ),
        }
    }

    pub(crate) fn signed_distance(&self, point: &DPoint, span: Span) -> Result<Jet, EvalError> {
        let offset = point.sub(&self.center);
        let x = offset
            .x
            .abs_live()
            .sub(&self.width.scale(0.5).sub(&self.radius));
        let y = offset
            .y
            .abs_live()
            .sub(&self.height.scale(0.5).sub(&self.radius));
        let zero = Jet::constant(0.0, x.gradient.len());
        let outside = DVector {
            x: x.max_live(&zero),
            y: y.max_live(&zero),
        };
        let distance = outside
            .norm()
            .map_err(|()| solver_error(span, "undefined rounded rectangle distance"))?;
        Ok(distance
            .add(&x.max_live(&y).min_live(&zero))
            .sub(&self.radius))
    }
}
