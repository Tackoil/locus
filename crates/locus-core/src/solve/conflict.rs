//! Conservative certificates for incompatible numeric targets.
//!
//! Failure of LM alone cannot establish infeasibility. These certificates use
//! disjoint tolerance bounds on the same named quantity. Other systems remain
//! unclassified instead of being reported as proven minimal conflicts.

use std::collections::BTreeMap;

use locus_syntax::{AngleUnit, BinOp, Expr, UnaryOp};

use super::UnknownSystem;
use super::ad::NumericDimension;
use super::relations::SourceRelation;
use crate::tol::solve_targets_disjoint;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Quantity<'a> {
    Number(&'a str),
}

struct Target<'a> {
    quantity: Quantity<'a>,
    value: f64,
    scale: f64,
}

pub(super) fn certified(
    sources: &[SourceRelation<'_>],
    system: &UnknownSystem,
    indices: &[usize],
) -> bool {
    let mut seen: BTreeMap<Quantity<'_>, Vec<Target<'_>>> = BTreeMap::new();
    for index in indices {
        for target in targets(sources[*index].expression, system) {
            let previous = seen.entry(target.quantity).or_default();
            if previous.iter().any(|other| {
                solve_targets_disjoint(other.value, target.value, other.scale.max(target.scale))
            }) {
                return true;
            }
            previous.push(target);
        }
    }
    false
}

fn targets<'a>(expression: &'a Expr, system: &UnknownSystem) -> Vec<Target<'a>> {
    match ungroup(expression) {
        Expr::Binary {
            op: BinOp::Eq,
            lhs,
            rhs,
            ..
        } => numeric_target(lhs, rhs, system)
            .or_else(|| numeric_target(rhs, lhs, system))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn numeric_target<'a>(lhs: &'a Expr, rhs: &Expr, system: &UnknownSystem) -> Option<Target<'a>> {
    let Expr::Ref(id) = ungroup(lhs) else {
        return None;
    };
    let dimension = system.dimensions.get(&id.name)?;
    let scale = match dimension {
        NumericDimension::Scalar | NumericDimension::Angle => 1.0,
        NumericDimension::Length => system.diameter,
        NumericDimension::Area => system.diameter * system.diameter,
    };
    Some(Target {
        quantity: Quantity::Number(&id.name),
        value: literal(rhs)?,
        scale,
    })
}

fn literal(expression: &Expr) -> Option<f64> {
    let value = match ungroup(expression) {
        Expr::Number(number) => literal_value(number),
        Expr::Unary {
            op: UnaryOp::Neg,
            expr,
            ..
        } => -literal(expr)?,
        _ => return None,
    };
    value.is_finite().then_some(value)
}

fn literal_value(number: &locus_syntax::NumberExpr) -> f64 {
    match number.unit {
        Some(AngleUnit::Deg) => number.value * core::f64::consts::PI / 180.0,
        Some(AngleUnit::Rad) | None => number.value,
    }
}

fn ungroup(mut expression: &Expr) -> &Expr {
    while let Expr::Group { expr, .. } = expression {
        expression = expr;
    }
    expression
}
