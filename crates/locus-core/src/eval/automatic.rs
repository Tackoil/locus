//! Placement and dimension unknowns for measured content and automatic boxes.

use locus_syntax::{Constructor, Expr, File, Kind, OpStmt, Statement};

use super::{
    EvalError, FileEvaluator, UnknownCursor, deterministic_placement, eval_text, include_bbox,
};
use crate::scene::LabelDirection;
use crate::{AnchorSpec, AnnotationSpec, BBox, Point, ResolvedObject};

pub(crate) fn is_automatic_box(op: &OpStmt) -> bool {
    matches!(&op.ctor, Constructor::Call { args, .. } if args.is_empty())
        && matches!(op.kind, Kind::Rect | Kind::Diamond | Kind::RoundedRect)
}

pub(crate) fn is_standalone_label(op: &OpStmt) -> bool {
    matches!(&op.ctor, Constructor::Call { args, .. } if args.len() == 1) && op.kind == Kind::Label
}

impl FileEvaluator {
    pub(super) fn evaluate_automatic_op(
        &mut self,
        op: &OpStmt,
        unknowns: &mut UnknownCursor<'_>,
    ) -> Result<(), EvalError> {
        let seed = self.automatic_center_seed(op);
        let center = Point::new(
            unknowns
                .next(op.span)?
                .unwrap_or(seed.x * self.placement_scale),
            unknowns
                .next(op.span)?
                .unwrap_or(seed.y * self.placement_scale),
        );
        let object = if is_automatic_box(op) {
            self.automatic_box(op, unknowns, center)?
        } else {
            self.standalone_label(op, center)?
        };
        self.placement_index += 1;
        if let Some(bounds) = object.bbox() {
            include_bbox(&mut self.bbox, bounds);
        }
        self.env
            .insert(op.id.clone(), object)
            .map_err(|error| EvalError::new("E302", op.id.span, error.to_string()))
    }

    fn automatic_box(
        &self,
        op: &OpStmt,
        unknowns: &mut UnknownCursor<'_>,
        center: Point,
    ) -> Result<ResolvedObject, EvalError> {
        let metrics = self.inputs.metrics;
        let (seed_width, seed_height) = self.automatic_dimension_seed(op);
        let width = unknowns.next(op.span)?.unwrap_or(seed_width);
        let height = unknowns.next(op.span)?.unwrap_or(seed_height);
        if width <= 0.0 || height <= 0.0 {
            return Err(EvalError::new(
                "E204",
                op.span,
                "automatic box dimensions must stay positive",
            ));
        }
        let tolerance = super::tolerance_for(self.bbox, &self.length_fallbacks);
        let object = match op.kind {
            Kind::RoundedRect => crate::RoundedRect::new(
                center,
                width,
                height,
                metrics.automatic_radius(width, height),
            )
            .map(ResolvedObject::RoundedRect),
            Kind::Diamond => crate::geom::construct::diamond(center, width, height, tolerance)
                .map(ResolvedObject::Diamond),
            _ => crate::geom::construct::rect(center, width, height, tolerance)
                .map(ResolvedObject::Rect),
        };
        object.map_err(|error| EvalError::construction(op.span, error))
    }

    fn automatic_center_seed(&self, op: &OpStmt) -> Point {
        if is_standalone_label(op) {
            if let Some(partner) = self.center_partners.get(&op.id.name) {
                if let Some(bounds) = self.env.get(partner).and_then(ResolvedObject::bbox) {
                    let center = bounds.center();
                    return Point::new(
                        center.x / self.placement_scale,
                        center.y / self.placement_scale,
                    );
                }
                if let Some(center) = self.placement_seeds.get(partner) {
                    return *center;
                }
            }
        }
        deterministic_placement(self.placement_index)
    }

    fn automatic_dimension_seed(&self, op: &OpStmt) -> (f64, f64) {
        let metrics = self.inputs.metrics;
        let centered = self
            .center_partners
            .get(&op.id.name)
            .and_then(|name| self.inputs.measured.get(name));
        if centered.is_none() && !self.content_extents.contains_key(&op.id.name) {
            return (metrics.default_box_width, metrics.default_box_height);
        }
        let extent = self
            .content_extents
            .get(&op.id.name)
            .copied()
            .unwrap_or((0.0, 0.0));
        let width = centered.map_or(extent.0, |text| text.width.max(extent.0));
        let height = centered.map_or(extent.1, |text| text.height.max(extent.1));
        (
            metrics.default_box_width.max(width + 2.0 * metrics.padding),
            metrics
                .default_box_height
                .max(height + 2.0 * metrics.padding),
        )
    }

    fn standalone_label(&self, op: &OpStmt, center: Point) -> Result<ResolvedObject, EvalError> {
        let measured = self.inputs.measured.get(&op.id.name).ok_or_else(|| {
            EvalError::new(
                "E410",
                op.span,
                format!("missing text measurement for {}", op.id.name),
            )
        })?;
        if !measured.width.is_finite()
            || !measured.height.is_finite()
            || measured.width < 0.0
            || measured.height <= 0.0
        {
            return Err(EvalError::new(
                "E412",
                op.span,
                "invalid measured text dimensions",
            ));
        }
        let Constructor::Call { args, .. } = &op.ctor else {
            return Err(EvalError::new("E401", op.span, "invalid standalone label"));
        };
        let bounds = BBox::from_points([
            Point::new(
                center.x - measured.width / 2.0,
                center.y - measured.height / 2.0,
            ),
            Point::new(
                center.x + measured.width / 2.0,
                center.y + measured.height / 2.0,
            ),
        ])
        .map_err(|error| EvalError::construction(op.span, error))?;
        Ok(ResolvedObject::Annotation(AnnotationSpec::Label {
            path: None,
            bounds: Some(bounds),
            anchor: AnchorSpec::Point(center),
            text: eval_text(&args[0])?,
            direction: LabelDirection::Center,
            gap: None,
        }))
    }
}

pub(super) fn content_extents(
    file: &File,
    inputs: &crate::EvaluationInputs,
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
) -> std::collections::BTreeMap<String, (f64, f64)> {
    let mut extents = std::collections::BTreeMap::new();
    for statement in &file.statements {
        for expression in relation_expressions(statement) {
            record_content(expression, inputs, collections, &mut extents);
        }
    }
    extents
}

fn record_content(
    expression: &Expr,
    inputs: &crate::EvaluationInputs,
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
    extents: &mut std::collections::BTreeMap<String, (f64, f64)>,
) {
    match expression {
        Expr::Group { expr, .. } => record_content(expr, inputs, collections, extents),
        Expr::Binary {
            op: locus_syntax::BinOp::And,
            lhs,
            rhs,
            ..
        } => {
            record_content(lhs, inputs, collections, extents);
            record_content(rhs, inputs, collections, extents);
        }
        Expr::Call { name, args, .. } if name.name == "contain" => {
            record_container(args, inputs, collections, extents);
        }
        _ => {}
    }
}

fn record_container(
    args: &[Expr],
    inputs: &crate::EvaluationInputs,
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
    extents: &mut std::collections::BTreeMap<String, (f64, f64)>,
) {
    let Some(Expr::Ref(container)) = args.first() else {
        return;
    };
    for argument in args.iter().skip(1) {
        if let Expr::Ref(id) = argument {
            let extent = measured_extent(&id.name, inputs, collections);
            let current = extents.entry(container.name.clone()).or_insert((0.0, 0.0));
            current.0 = current.0.max(extent.0);
            current.1 = current.1.max(extent.1);
        }
    }
}

fn measured_extent(
    name: &str,
    inputs: &crate::EvaluationInputs,
    collections: &std::collections::BTreeMap<String, locus_syntax::Collection>,
) -> (f64, f64) {
    if let Some(text) = inputs.measured.get(name) {
        return (text.width, text.height);
    }
    let Some(collection) = collections.get(name) else {
        return (0.0, 0.0);
    };
    collection
        .members
        .iter()
        .map(|member| measured_extent(member, inputs, collections))
        .fold((0.0, 0.0), |extent, member| {
            (extent.0.max(member.0), extent.1.max(member.1))
        })
}

pub(super) fn center_partners(file: &File) -> std::collections::BTreeMap<String, String> {
    let mut partners = std::collections::BTreeMap::new();
    for statement in &file.statements {
        for expression in relation_expressions(statement) {
            record_center(expression, &mut partners);
        }
    }
    partners
}

fn relation_expressions(statement: &Statement) -> Vec<&Expr> {
    match statement {
        Statement::Rel(relation) => vec![&relation.expr],
        Statement::Op(op) => op.where_clauses.iter().collect(),
        _ => Vec::new(),
    }
}

fn record_center(expression: &Expr, partners: &mut std::collections::BTreeMap<String, String>) {
    match expression {
        Expr::Group { expr, .. } => record_center(expr, partners),
        Expr::Binary {
            op: locus_syntax::BinOp::And,
            lhs,
            rhs,
            ..
        } => {
            record_center(lhs, partners);
            record_center(rhs, partners);
        }
        Expr::Call { name, args, .. } if name.name == "center" => {
            if let [Expr::Ref(first), Expr::Ref(second)] = args.as_slice() {
                partners
                    .entry(first.name.clone())
                    .or_insert_with(|| second.name.clone());
                partners
                    .entry(second.name.clone())
                    .or_insert_with(|| first.name.clone());
            }
        }
        _ => {}
    }
}
