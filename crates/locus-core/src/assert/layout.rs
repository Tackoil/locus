//! Boolean layout checks sharing the solve's shape-distance kernel.
use super::{AssertionContext, EvalError, Expr, Point, Span, assertion_error};

pub(super) fn is_layout(name: &str) -> bool {
    matches!(
        name,
        "gap_x"
            | "gap_y"
            | "row"
            | "column"
            | "contain"
            | "cover_x"
            | "cover_y"
            | "avoid"
            | "trim_endpoints"
    )
}

impl AssertionContext<'_> {
    pub(super) fn eval_layout(
        &mut self,
        name: &str,
        args: &[Expr],
        span: Span,
    ) -> Result<bool, EvalError> {
        if name == "trim_endpoints" {
            return Ok(true);
        }
        if args.len() < 2 && !matches!(name, "row" | "column" | "gap_x" | "gap_y") {
            return Err(assertion_error(
                span,
                "layout relation requires at least two operands",
            ));
        }
        match name {
            "contain" => self.check_contain(args, span),
            "avoid" => self.check_avoid(args, span),
            "cover_x" | "cover_y" => self.check_cover(name, args),
            _ => self.check_chain(name, args),
        }
    }

    fn layout_members(&self, args: &[Expr]) -> Vec<Expr> {
        crate::program::avoidance_operands(self.collections, self.aliases, args)
    }

    fn satisfies(&self, value: f64) -> bool {
        value >= -self.tolerance.epsilon() * self.tolerance.diameter()
    }

    fn check_chain(&mut self, name: &str, args: &[Expr]) -> Result<bool, EvalError> {
        let args = if matches!(name, "row" | "column") {
            crate::program::chain_operands(self.collections, self.aliases, args)
        } else {
            args.to_vec()
        };
        if args.len() < 2 {
            return Err(assertion_error(
                args.first().map_or(Span::default(), Expr::span),
                "layout chain requires at least two items",
            ));
        }
        let boxes = args
            .iter()
            .map(|arg| self.measured_bounds(arg))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(boxes
            .windows(2)
            .all(|pair| self.check_chain_pair(name, pair[0], pair[1])))
    }

    fn check_chain_pair(&self, name: &str, first: super::BBox, second: super::BBox) -> bool {
        let horizontal = matches!(name, "row" | "gap_x");
        let gap = if horizontal {
            second.min.x - first.max.x
        } else {
            first.min.y - second.max.y
        };
        if !self.satisfies(gap - self.metrics.gap) {
            return false;
        }
        if matches!(name, "row" | "column") {
            let a = first.center();
            let b = second.center();
            return self
                .tolerance
                .length_zero(if horizontal { a.y - b.y } else { a.x - b.x });
        }
        true
    }

    fn check_cover(&mut self, name: &str, args: &[Expr]) -> Result<bool, EvalError> {
        let container = self.measured_bounds(&args[0])?;
        for item in &args[1..] {
            let b = self.measured_bounds(item)?;
            let margins = if name == "cover_x" {
                [b.min.x - container.min.x, container.max.x - b.max.x]
            } else {
                [b.min.y - container.min.y, container.max.y - b.max.y]
            };
            if margins
                .iter()
                .any(|margin| !self.satisfies(margin - self.padding))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn check_contain(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        let shape = self.expressions.eval_resolved_object(&args[0])?;
        for item in &args[1..] {
            let b = self.measured_bounds(item)?;
            for point in [
                b.min,
                b.max,
                Point::new(b.min.x, b.max.y),
                Point::new(b.max.x, b.min.y),
            ] {
                let sdf =
                    crate::geom::sdf::object_signed_distance(&shape, point).ok_or_else(|| {
                        assertion_error(span, "contain requires a closed container shape")
                    })?;
                if !self.satisfies(-sdf - self.padding) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn check_avoid(&mut self, args: &[Expr], span: Span) -> Result<bool, EvalError> {
        for subject in self.layout_members(&args[..1]) {
            let a = self.expressions.eval_resolved_object(&subject)?;
            for obstacle in self.layout_members(&args[1..]) {
                let b = self.expressions.eval_resolved_object(&obstacle)?;
                let sep = self
                    .avoidance_separation(&subject, &a, &obstacle, &b)
                    .ok_or_else(|| {
                        assertion_error(span, "avoid requires finite visible geometry")
                    })?;
                if !self.satisfies(sep - self.metrics.clearance) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn avoidance_separation(
        &self,
        subject: &Expr,
        a: &crate::ResolvedObject,
        obstacle: &Expr,
        b: &crate::ResolvedObject,
    ) -> Option<f64> {
        let a = self.visible_object(subject, a);
        let b = self.visible_object(obstacle, b);
        if let crate::ResolvedObject::Annotation(crate::AnnotationSpec::Arrow {
            route: Some(route),
            ..
        }) = &a
        {
            return crate::geom::sdf::attached_route_separation(
                route,
                &b,
                self.metrics.stroke_width / 2.0,
                self.metrics.clearance,
            );
        }
        crate::geom::sdf::object_separation(&a, &b, self.metrics.stroke_width / 2.0, None)
    }

    fn visible_object(
        &self,
        expression: &Expr,
        object: &crate::ResolvedObject,
    ) -> crate::ResolvedObject {
        let Expr::Ref(id) = expression else {
            return object.clone();
        };
        let mut visible = object.clone();
        match &mut visible {
            crate::ResolvedObject::Annotation(crate::AnnotationSpec::Arrow {
                route,
                from: crate::AnchorSpec::Point(from),
                to: crate::AnchorSpec::Point(to),
                bend,
                ..
            }) => {
                if let Some(logical) = route
                    .clone()
                    .or_else(|| crate::geom::route::Route::direct(*from, *to, *bend).ok())
                {
                    let range = crate::eval::path_label::retained_parameters(
                        &logical,
                        &self.choices.retained_routes,
                        &self.choices.retained_distances,
                        self.aliases,
                        &id.name,
                    );
                    *route = Some(logical.retained_parameters((&range.0, &range.1)));
                }
            }
            crate::ResolvedObject::Segment(segment) => {
                if let Ok(logical) =
                    crate::geom::route::Route::direct(segment.start, segment.end, 0.0)
                {
                    let range = crate::eval::path_label::retained_parameters(
                        &logical,
                        &self.choices.retained_routes,
                        &self.choices.retained_distances,
                        self.aliases,
                        &id.name,
                    );
                    let clipped = logical.retained_parameters((&range.0, &range.1));
                    segment.start = clipped.from.into();
                    segment.end = clipped.to.into();
                }
            }
            _ => {}
        }
        visible
    }
}
