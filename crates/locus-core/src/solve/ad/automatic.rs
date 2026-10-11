//! Differentiated automatic dimensions and independently placed text bounds.

use super::{
    AdEvaluator, DObject, DPoint, DRoundedRect, EvalError, Jet, Kind, OpStmt, diamond_vertices,
    rect_vertices, solver_error,
};

impl AdEvaluator<'_> {
    pub(super) fn automatic_object(&mut self, op: &OpStmt) -> Result<DObject, EvalError> {
        let center = DPoint {
            x: self.variable(op.span)?,
            y: self.variable(op.span)?,
        };
        if crate::eval::automatic::is_standalone_label(op) {
            let measured = self
                .measured
                .get(&op.id.name)
                .ok_or_else(|| solver_error(op.span, "missing text measurement"))?;
            let width = Jet::constant(measured.width, self.normalized.len());
            let height = Jet::constant(measured.height, self.normalized.len());
            return Ok(DObject::Polygon(
                rect_vertices(&center, &width, &height),
                true,
                false,
            ));
        }
        let width = self.variable(op.span)?;
        let height = self.variable(op.span)?;
        if width.value <= 0.0 || height.value <= 0.0 {
            return Err(solver_error(
                op.span,
                "automatic dimensions must stay positive",
            ));
        }
        let object = match op.kind {
            Kind::RoundedRect => {
                let shorter = if width.value <= height.value {
                    &width
                } else {
                    &height
                };
                let radius = shorter.scale(self.metrics.rounded_radius_fraction.clamp(0.0, 0.5));
                DObject::RoundedRect(DRoundedRect::automatic(center, width, height, radius))
            }
            Kind::Diamond => {
                DObject::Polygon(diamond_vertices(&center, &width, &height), true, false)
            }
            _ => DObject::Polygon(rect_vertices(&center, &width, &height), true, true),
        };
        Ok(object)
    }
}
