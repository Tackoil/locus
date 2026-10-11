//! Live normalized layout requirements and compactness preferences.

use super::ad::{DObject, DPoint, DifferentiatedFile, Jet};
use super::relations::{
    Equation, MeasuredBounds, SourceRelation, measured_bounds, measured_contributors,
};
use crate::eval::EvalError;
use locus_syntax::{Expr, Span};

#[derive(Default)]
pub(super) struct LayoutRows {
    pub(super) equalities: Vec<Equation>,
    pub(super) inequalities: Vec<Equation>,
    pub(super) preferences: Vec<Jet>,
}

pub(super) fn is_layout(expression: &Expr) -> bool {
    matches!(expression, Expr::Call {name,..} if matches!(name.name.as_str(),
        "gap_x" | "gap_y" | "row" | "column" | "contain" | "cover_x" | "cover_y" | "avoid" | "trim_endpoints"))
}

pub(super) fn lower(
    d: &DifferentiatedFile,
    sources: &[SourceRelation<'_>],
    diameter: f64,
    forced_axes: &std::collections::BTreeMap<usize, usize>,
) -> Result<LayoutRows, EvalError> {
    lower_with(d, sources, diameter, forced_axes, None)
}

/// Phase 1 retains its aggregate residual weights. Settling uses equivalent
/// individual faces, with avoidance choices mapped by original row identity.
pub(super) fn lower_settling(
    d: &DifferentiatedFile,
    sources: &[SourceRelation<'_>],
    diameter: f64,
    forced_axes: &std::collections::BTreeMap<usize, usize>,
) -> Result<LayoutRows, EvalError> {
    let aggregate = lower(d, sources, diameter, forced_axes)?;
    lower_with(d, sources, diameter, forced_axes, Some(&aggregate))
}

fn lower_with(
    d: &DifferentiatedFile,
    sources: &[SourceRelation<'_>],
    diameter: f64,
    forced_axes: &std::collections::BTreeMap<usize, usize>,
    aggregate: Option<&LayoutRows>,
) -> Result<LayoutRows, EvalError> {
    let mut rows = LayoutRows::default();
    for (index, source) in sources.iter().enumerate() {
        if !is_layout(source.expression) {
            continue;
        }
        lower_atomic(
            d,
            source,
            index,
            diameter,
            forced_axes,
            &mut rows,
            aggregate,
        )?;
    }
    Ok(rows)
}

fn lower_atomic(
    d: &DifferentiatedFile,
    source: &SourceRelation<'_>,
    index: usize,
    diameter: f64,
    axes: &std::collections::BTreeMap<usize, usize>,
    rows: &mut LayoutRows,
    aggregate: Option<&LayoutRows>,
) -> Result<(), EvalError> {
    let Expr::Call { name, args, span } = source.expression else {
        return Ok(());
    };
    let operands = if matches!(name.name.as_str(), "row" | "column") {
        crate::program::chain_operands(&d.collections, &d.aliases, args)
    } else {
        args.clone()
    };
    let padding = if source.padding_none {
        0.0
    } else {
        d.metrics.padding
    };
    let mut sink = Sink {
        rows,
        relation: index,
        scale: diameter,
        expanded: aggregate.is_some(),
        avoidance_offset: aggregate.map(|rows| {
            rows.inequalities
                .iter()
                .filter(|row| row.relation < index)
                .count()
        }),
    };
    match name.name.as_str() {
        "gap_x" | "gap_y" | "row" | "column" => chain(d, &operands, &name.name, *span, &mut sink),
        "cover_x" | "cover_y" => cover(d, args, &name.name, padding, *span, &mut sink),
        "contain" => contain(d, args, padding, *span, &mut sink),
        "avoid" => avoid(d, args, axes, *span, &mut sink),
        _ => Ok(()),
    }
}

pub(super) fn expand(d: &DifferentiatedFile, args: &[Expr]) -> Vec<Expr> {
    crate::program::avoidance_operands(&d.collections, &d.aliases, args)
}

struct Sink<'a> {
    rows: &'a mut LayoutRows,
    relation: usize,
    scale: f64,
    expanded: bool,
    avoidance_offset: Option<usize>,
}
impl Sink<'_> {
    fn requirement(&mut self, value: Jet) {
        self.rows.inequalities.push(Equation {
            residual: normalize(value, self.scale),
            relation: self.relation,
        });
    }
    fn equality(&mut self, value: Jet) {
        self.rows.equalities.push(Equation {
            residual: normalize(value, self.scale),
            relation: self.relation,
        });
    }
    fn compact(&mut self, value: Jet) {
        self.rows.preferences.push(normalize(value, self.scale));
    }
}

fn normalize(mut value: Jet, scale: f64) -> Jet {
    value.value /= scale;
    value.seed_value /= scale;
    for gradient in &mut value.gradient {
        *gradient /= scale;
    }
    value
}

fn chain(
    d: &DifferentiatedFile,
    args: &[Expr],
    name: &str,
    span: Span,
    sink: &mut Sink<'_>,
) -> Result<(), EvalError> {
    if args.len() < 2 {
        return Err(error(span, "layout chain requires at least two members"));
    }
    let horizontal = matches!(name, "row" | "gap_x");
    let bounds = args
        .iter()
        .map(|arg| measured_bounds(d, arg))
        .collect::<Result<Vec<_>, _>>()?;
    for (pair, operands) in bounds.windows(2).zip(args.windows(2)) {
        let g = gap_residual(&pair[0], &pair[1], horizontal, d.metrics.gap);
        if sink.expanded {
            chain_faces(d, operands, horizontal, sink)?;
        } else {
            sink.requirement(g.clone());
        }
        sink.compact(g);
        if matches!(name, "row" | "column") {
            let a = pair[0].clone().center();
            let b = pair[1].clone().center();
            sink.equality(if horizontal {
                a.y.sub(&b.y)
            } else {
                a.x.sub(&b.x)
            });
        }
    }
    Ok(())
}

fn chain_faces(
    d: &DifferentiatedFile,
    operands: &[Expr],
    horizontal: bool,
    sink: &mut Sink<'_>,
) -> Result<(), EvalError> {
    let first = measured_contributors(d, &operands[..1])?;
    let second = measured_contributors(d, &operands[1..])?;
    for a in &first {
        for b in &second {
            sink.requirement(gap_residual(a, b, horizontal, d.metrics.gap));
        }
    }
    Ok(())
}

fn gap_residual(a: &MeasuredBounds, b: &MeasuredBounds, horizontal: bool, gap: f64) -> Jet {
    let value = if horizontal {
        b.minimum.x.sub(&a.maximum.x)
    } else {
        a.minimum.y.sub(&b.maximum.y)
    };
    value.sub(&Jet::constant(gap, value.gradient.len()))
}

fn cover(
    d: &DifferentiatedFile,
    args: &[Expr],
    name: &str,
    padding: f64,
    span: Span,
    sink: &mut Sink<'_>,
) -> Result<(), EvalError> {
    if args.len() < 2 {
        return Err(error(span, "cover requires a container and content"));
    }
    let container = measured_bounds(d, &args[0])?;
    let content = if sink.expanded {
        measured_contributors(d, &args[1..])?
    } else {
        vec![aggregate_content(d, &args[1..])?]
    };
    let padding = Jet::constant(padding, container.minimum.x.gradient.len());
    for content in content {
        cover_faces(&container, &content, name == "cover_x", &padding, sink);
    }
    Ok(())
}

fn aggregate_content(d: &DifferentiatedFile, args: &[Expr]) -> Result<MeasuredBounds, EvalError> {
    let mut result = measured_bounds(d, &args[0])?;
    for argument in &args[1..] {
        let bounds = measured_bounds(d, argument)?;
        result.minimum.x = result.minimum.x.min_live(&bounds.minimum.x);
        result.minimum.y = result.minimum.y.min_live(&bounds.minimum.y);
        result.maximum.x = result.maximum.x.max_live(&bounds.maximum.x);
        result.maximum.y = result.maximum.y.max_live(&bounds.maximum.y);
    }
    Ok(result)
}

fn cover_faces(
    container: &MeasuredBounds,
    content: &MeasuredBounds,
    horizontal: bool,
    padding: &Jet,
    sink: &mut Sink<'_>,
) {
    let (low, high) = if horizontal {
        (
            content.minimum.x.sub(&container.minimum.x),
            container.maximum.x.sub(&content.maximum.x),
        )
    } else {
        (
            content.minimum.y.sub(&container.minimum.y),
            container.maximum.y.sub(&content.maximum.y),
        )
    };
    for value in [low, high] {
        sink.requirement(value.sub(padding));
    }
}

fn contain(
    d: &DifferentiatedFile,
    args: &[Expr],
    padding: f64,
    span: Span,
    sink: &mut Sink<'_>,
) -> Result<(), EvalError> {
    if args.len() < 2 {
        return Err(error(span, "contain requires a shape and content"));
    }
    let container = d.object(&args[0])?;
    if contain_straight_faces(d, &container, args, padding, sink)? {
        return Ok(());
    }
    for item in &args[1..] {
        let b = measured_bounds(d, item)?;
        for (x, y) in [
            (&b.minimum.x, &b.minimum.y),
            (&b.minimum.x, &b.maximum.y),
            (&b.maximum.x, &b.minimum.y),
            (&b.maximum.x, &b.maximum.y),
        ] {
            let sdf = super::ad::sdf::signed_distance(
                &container,
                &DPoint {
                    x: x.clone(),
                    y: y.clone(),
                },
                span,
            )?;
            sink.requirement(
                sdf.scale(-1.0)
                    .sub(&Jet::constant(padding, sdf.gradient.len())),
            );
        }
    }
    Ok(())
}

/// Returns whether the straight faces fully describe the padded container.
fn contain_straight_faces(
    d: &DifferentiatedFile,
    container: &DObject,
    args: &[Expr],
    padding: f64,
    sink: &mut Sink<'_>,
) -> Result<bool, EvalError> {
    if !sink.expanded
        || !matches!(
            container,
            DObject::Polygon(_, _, true) | DObject::RoundedRect(_)
        )
    {
        return Ok(false);
    }
    let bounds = measured_bounds(d, &args[0])?;
    let face_padding = match container {
        DObject::RoundedRect(rounded) => rounded.containment_face_padding(padding),
        _ => Jet::constant(padding, bounds.minimum.x.gradient.len()),
    };
    for content in measured_contributors(d, &args[1..])? {
        cover_faces(&bounds, &content, true, &face_padding, sink);
        cover_faces(&bounds, &content, false, &face_padding, sink);
    }
    // Rounded-distance rows still enforce arcs. The extra straight faces expose
    // core ties but stay slack on arcs, avoiding nearly parallel active constraints.
    Ok(matches!(container, DObject::Polygon(_, _, true)))
}

fn avoid(
    d: &DifferentiatedFile,
    args: &[Expr],
    axes: &std::collections::BTreeMap<usize, usize>,
    span: Span,
    sink: &mut Sink<'_>,
) -> Result<(), EvalError> {
    if args.len() < 2 {
        return Err(error(span, "avoid requires a subject and obstacles"));
    }
    let mut axis_index = sink
        .avoidance_offset
        .unwrap_or(sink.rows.inequalities.len());
    for subject in expand(d, &args[..1]) {
        let a = d.object(&subject)?;
        if matches!(a, DObject::Route(_)) {
            continue;
        }
        for obstacle in expand(d, &args[1..]) {
            let b = d.avoidance_object(&obstacle)?;
            let selected = axes.get(&axis_index).copied();
            let sep =
                super::ad::sdf::separation(&a, &b, d.metrics.stroke_width / 2.0, selected, span)?;
            sink.requirement(sep.sub(&Jet::constant(d.metrics.clearance, sep.gradient.len())));
            axis_index += 1;
        }
    }
    Ok(())
}

fn error(span: Span, message: &str) -> EvalError {
    EvalError::new("E404", span, message)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::eval::PreparedEvaluation;
    use crate::{EvaluationInputs, MeasuredRole, MeasuredText};
    use std::collections::BTreeMap;

    fn at(source: &str, x: f64, caption_height: f64) -> (LayoutRows, usize, f64) {
        let parsed = locus_syntax::parse(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let expanded = crate::program::expand_program(&parsed.file).expect("expanded");
        let mut inputs = EvaluationInputs::default();
        inputs.measured.insert(
            "caption".to_owned(),
            MeasuredText {
                width: 0.5,
                height: caption_height,
                role: MeasuredRole::Normal,
            },
        );
        let prepared =
            PreparedEvaluation::with_inputs(&expanded.file, &inputs, &expanded.collections)
                .expect("prepared");
        let sources = super::super::relations::collect(&expanded.file);
        let system = super::super::initialize(&prepared, &sources).expect("initialized");
        let index = system
            .objects
            .iter()
            .find(|o| o.id == "x")
            .expect("x")
            .start;
        let mut z = system.normalized_hints.clone();
        z[index] = x / system.scales[index];
        let tolerance = crate::Tolerance::with_epsilon(system.diameter, 1e-9).expect("tolerance");
        let d = DifferentiatedFile::evaluate(
            &prepared,
            &z,
            &system.normalized_hints,
            &system.scales,
            &system.dimensions,
            tolerance,
        )
        .expect("differentiated");
        (
            lower_settling(&d, &sources, system.diameter, &BTreeMap::new()).expect("lowered"),
            index,
            system.scales[index],
        )
    }

    fn minimum(rows: &LayoutRows) -> f64 {
        rows.inequalities
            .iter()
            .map(|r| r.residual.value)
            .fold(f64::INFINITY, f64::min)
    }

    fn assert_affine_rows(
        before: &(LayoutRows, usize, f64),
        after: &(LayoutRows, usize, f64),
        delta: f64,
    ) {
        assert_eq!(before.0.inequalities.len(), after.0.inequalities.len());
        for (a, b) in before.0.inequalities.iter().zip(&after.0.inequalities) {
            assert_eq!(a.relation, b.relation);
            let predicted = a.residual.value + a.residual.gradient[before.1] * delta / before.2;
            assert!((predicted - b.residual.value).abs() < 1e-12);
        }
    }

    #[test]
    fn rectangular_containment_keeps_all_faces_across_nearest_face_switch() {
        let source = "locus 0 beta 6\npoint origin()\nnum x = free 0\npoint axis = on_circle(circle(origin, 1), 0deg)\npoint level = on_circle(circle(origin, 0.5), 90deg)\npoint item = translate(lerp(origin, axis, x), origin, level)\nrect panel(origin,2,2)\nrel contain(panel,item)\n";
        let before = at(source, 0.5 - 1e-7, 0.5);
        let after = at(source, 0.5 + 1e-7, 0.5);
        assert_eq!(before.0.inequalities.len(), 4);
        assert!(minimum(&before.0) > 0.0 && minimum(&after.0) > 0.0);
        assert_affine_rows(&before, &after, 2e-7);
        assert!(before.0.preferences.is_empty());
    }

    #[test]
    fn rounded_containment_keeps_both_binding_edges_at_a_corner() {
        let source = "locus 0 beta 6\npoint origin()\nnum x = free 2\npoint east = on_circle(circle(origin, 0.75), 0deg)\npoint north = on_circle(circle(origin, 0.75), 90deg)\npoint item = translate(east, origin, north)\nrounded_rect panel(origin, 2, x, 0.08)\nrel contain(panel, item)\n";
        let result = settle_parameter(source, 2.0, 1.0);
        assert!(result.report.complete, "{:?}", result.report);
        assert!((result.z[0] - 2.0).abs() < 1e-12);
        assert!(minimum(&at(source, result.z[0], 0.5).0) > -1e-12);
    }

    fn settle_parameter(
        source: &str,
        initial: f64,
        target: f64,
    ) -> super::super::settling::SettlingResult {
        use super::super::settling::{ConstraintState, Rows, optimize};

        optimize(&[initial], |parameter| {
            let (layout, index, scale) = at(source, parameter[0], 0.5);
            Some(ConstraintState {
                inequalities: Rows {
                    values: layout
                        .inequalities
                        .iter()
                        .map(|r| r.residual.value)
                        .collect(),
                    jacobian: layout
                        .inequalities
                        .iter()
                        .map(|r| vec![r.residual.gradient[index] / scale])
                        .collect(),
                },
                preferences: Rows {
                    values: vec![parameter[0] - target],
                    jacobian: vec![vec![1.0]],
                },
                ..ConstraintState::default()
            })
        })
    }

    #[test]
    fn rounded_containment_allows_tangent_motion_near_a_straight_edge() {
        let source = "locus 0 beta 6\npoint origin()\nnum x = free 90deg\npoint east = on_circle(circle(origin, 0.5), 0deg)\npoint north = on_circle(circle(origin, 0.5), 90deg)\npoint corner = translate(east, origin, north)\npoint item = on_circle(circle(corner, 0.25), x)\nrounded_rect panel(origin, 2, 2, 0.5)\nrel contain(panel, item)\n";
        let target = core::f64::consts::PI / 3.0;
        let result = settle_parameter(source, core::f64::consts::FRAC_PI_2 - 1e-7, target);
        assert!(result.report.complete, "{:?}", result.report);
        assert!((result.z[0] - target).abs() < 1e-9);
        assert!(minimum(&at(source, result.z[0], 0.5).0) > -1e-12);
    }

    #[test]
    fn rounded_containment_still_enforces_curved_corner_clearance() {
        let source = "locus 0 beta 6\npoint origin()\nnum x = free 0\npoint axis = on_circle(circle(origin, 1), 0deg)\npoint level = on_circle(circle(origin, 0.72), 90deg)\npoint item = translate(lerp(origin, axis, x), origin, level)\nrounded_rect panel(origin, 2, 2, 0.5)\nrel contain(panel, item)\n";
        let (layout, _, _) = at(source, 0.72, 0.5);
        assert!(
            (1.0 - 0.72) > crate::preset::LayoutMetrics::default().padding,
            "the point clears both straight edges"
        );
        assert!(
            minimum(&layout) < 0.0,
            "the curved corner excludes the point"
        );
    }

    #[test]
    fn loop_caption_and_geometry_exchange_the_bottom_bound_without_changing_rows() {
        let source = "locus 0 beta 6\npoint origin()\nnum x = free 0\npoint below = on_circle(circle(origin, 3), 270deg)\npoint end = lerp(origin, below, x / 3)\narrow path(origin,end) { route: orthogonal, side: right }\nlabel caption(path,\"Loop\") { near: middle, side: right }\ngroup content(path,caption)\nrel gap_y(content,below)\n";
        let before = at(source, 1.0 - 1e-7, 1.0);
        let after = at(source, 1.0 + 1e-7, 1.0);
        assert!(before.0.inequalities.len() > 1);
        assert_eq!(
            before.0.preferences.len(),
            1,
            "one aggregate compact-gap preference"
        );
        assert_affine_rows(&before, &after, 2e-7);
        assert!(minimum(&before.0) > 0.0 && minimum(&after.0) > 0.0);
        // Caption is explicitly in the collection and attached to the arrow;
        // this must have the same rows and preference weight as the arrow alone.
        let direct = at(
            &source.replace("gap_y(content,below)", "gap_y(path,below)"),
            1.0 - 1e-7,
            1.0,
        );
        assert_eq!(direct.0.inequalities.len(), before.0.inequalities.len());
        for (a, b) in direct.0.inequalities.iter().zip(&before.0.inequalities) {
            assert!((a.residual.value - b.residual.value).abs() < 1e-12);
        }
    }
}
