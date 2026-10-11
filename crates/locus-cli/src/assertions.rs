//! Source context for failed assertions, shared by human and JSON output.

use std::collections::BTreeSet;

use locus_core::{ActualValue, DependencyError, DependencyGraph, ReportDimension, SubtermActual};
use locus_syntax::{BinOp, Expr, File, Span, Statement};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AssertionReport {
    pub passed: bool,
    pub assertions: Vec<AssertionResult>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AssertionResult {
    pub index: usize,
    pub span: Span,
    pub tolerance: f64,
    pub passed: bool,
    pub actuals: Vec<NamedValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<AssertionFailure>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct NamedValue {
    pub expression: String,
    pub span: Option<Span>,
    #[serde(flatten)]
    pub actual: ActualValue,
    pub unit: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct AssertionFailure {
    pub expression: String,
    pub comparisons: Vec<Comparison>,
    pub upstream_definitions: Vec<Definition>,
    pub upstream_relations: Vec<SourceExpression>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Comparison {
    pub expression: String,
    pub operator: &'static str,
    pub actual: NamedValue,
    pub expected: NamedValue,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Definition {
    pub id: String,
    pub span: Span,
    pub source: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SourceExpression {
    pub span: Span,
    pub source: String,
}

pub(crate) fn report(
    report: locus_core::AssertionReport,
    scene: &locus_core::Scene,
    source: &str,
) -> Result<AssertionReport, DependencyError> {
    let file = &scene.file;
    let graph = DependencyGraph::extract_scene(scene)?;
    let expressions = file
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Assert(assertion) => Some(&assertion.expr),
            _ => None,
        });
    let assertions = report
        .assertions
        .into_iter()
        .zip(expressions)
        .map(|(result, expression)| {
            let failure = (!result.passed)
                .then(|| failure(expression, &result.actuals, file, &graph, source));
            AssertionResult {
                index: result.index,
                span: result.span,
                tolerance: result.tolerance,
                passed: result.passed,
                actuals: result
                    .actuals
                    .iter()
                    .map(|term| named_value(term, source))
                    .collect(),
                failure,
            }
        })
        .collect();
    Ok(AssertionReport {
        passed: report.passed,
        assertions,
    })
}

fn failure(
    expression: &Expr,
    actuals: &[SubtermActual],
    file: &File,
    graph: &DependencyGraph,
    source: &str,
) -> AssertionFailure {
    let mut comparisons = Vec::new();
    collect_comparisons(expression, actuals, source, &mut comparisons);
    let mut needed = references(expression);
    let relations = related_relations(file, graph, &mut needed);
    let upstream_definitions = file
        .statements
        .iter()
        .filter_map(|statement| {
            let Statement::Op(op) = statement else {
                return None;
            };
            needed.contains(&op.id.name).then(|| Definition {
                id: op.id.name.clone(),
                span: op.span,
                source: source_at(source, op.span),
            })
        })
        .collect();
    AssertionFailure {
        expression: source_at(source, expression.span()),
        comparisons,
        upstream_definitions,
        upstream_relations: relations
            .into_iter()
            .map(|span| SourceExpression {
                span,
                source: source_at(source, span),
            })
            .collect(),
    }
}

fn named_value(term: &SubtermActual, source: &str) -> NamedValue {
    NamedValue {
        expression: source_at(source, term.span),
        span: Some(term.span),
        actual: term.actual.clone(),
        unit: unit(&term.actual),
    }
}

pub(crate) fn unit(actual: &ActualValue) -> &'static str {
    match actual {
        ActualValue::Number {
            dimension: ReportDimension::Scalar,
            ..
        }
        | ActualValue::Boolean { .. } => "unitless",
        ActualValue::Number {
            dimension: ReportDimension::Length,
            ..
        } => "scene units",
        ActualValue::Number {
            dimension: ReportDimension::Area,
            ..
        } => "scene units squared",
        ActualValue::Angle { .. } => "rad",
    }
}

fn source_at(source: &str, span: Span) -> String {
    source
        .get(span.byte_start..span.byte_end)
        .unwrap_or_default()
        .to_owned()
}

fn measured(actuals: &[SubtermActual], expression: &Expr) -> Option<SubtermActual> {
    actuals
        .iter()
        .find(|term| term.span == expression.span())
        .cloned()
}

fn collect_comparisons(
    expression: &Expr,
    actuals: &[SubtermActual],
    source: &str,
    comparisons: &mut Vec<Comparison>,
) {
    let Some(term) = measured(actuals, expression) else {
        return;
    };
    if !matches!(term.actual, ActualValue::Boolean { value: false }) {
        return;
    }
    match expression {
        Expr::Group { expr, .. } => collect_comparisons(expr, actuals, source, comparisons),
        Expr::Binary {
            op: BinOp::And | BinOp::Or,
            lhs,
            rhs,
            ..
        } => {
            collect_comparisons(lhs, actuals, source, comparisons);
            collect_comparisons(rhs, actuals, source, comparisons);
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            comparisons.extend(numeric_comparison(*op, lhs, rhs, &term, actuals, source));
        }
        _ => comparisons.push(Comparison {
            expression: source_at(source, term.span),
            operator: "==",
            actual: named_value(&term, source),
            expected: NamedValue {
                expression: "true".to_owned(),
                span: None,
                actual: ActualValue::Boolean { value: true },
                unit: "unitless",
            },
        }),
    }
}

fn numeric_comparison(
    op: BinOp,
    lhs: &Expr,
    rhs: &Expr,
    term: &SubtermActual,
    actuals: &[SubtermActual],
    source: &str,
) -> Option<Comparison> {
    let operator = match op {
        BinOp::Eq => "==",
        BinOp::Lt => "<",
        BinOp::Gt => ">",
        BinOp::Le => "<=",
        BinOp::Ge => ">=",
        _ => return None,
    };
    Some(Comparison {
        expression: source_at(source, term.span),
        operator,
        actual: named_value(&measured(actuals, lhs)?, source),
        expected: named_value(&measured(actuals, rhs)?, source),
    })
}

fn children(expression: &Expr) -> Vec<&Expr> {
    match expression {
        Expr::Call { args, .. } => args.iter().collect(),
        Expr::Property { base, args, .. } => std::iter::once(base.as_ref()).chain(args).collect(),
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => vec![expr],
        Expr::Binary { lhs, rhs, .. } => vec![lhs, rhs],
        _ => Vec::new(),
    }
}

fn references(expression: &Expr) -> BTreeSet<String> {
    let mut refs = BTreeSet::new();
    collect_references(expression, &mut refs);
    refs
}

fn collect_references(expression: &Expr, refs: &mut BTreeSet<String>) {
    if let Expr::Ref(id) = expression {
        refs.insert(id.name.clone());
    }
    for child in children(expression) {
        collect_references(child, refs);
    }
}

fn expand_dependencies(graph: &DependencyGraph, needed: &mut BTreeSet<String>) {
    let mut pending: Vec<_> = needed.iter().cloned().collect();
    while let Some(id) = pending.pop() {
        for dependency in graph.dependencies_of(&id).into_iter().flatten() {
            if needed.insert(dependency.clone()) {
                pending.push(dependency.clone());
            }
        }
    }
}

fn related_relations(
    file: &File,
    graph: &DependencyGraph,
    needed: &mut BTreeSet<String>,
) -> Vec<Span> {
    let relations: Vec<_> = file
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Rel(relation) => Some((relation.span, references(&relation.expr))),
            _ => None,
        })
        .collect();
    let mut selected = BTreeSet::new();
    loop {
        expand_dependencies(graph, needed);
        let previous = needed.len();
        select_relations(&relations, needed, &mut selected);
        if needed.len() == previous {
            break;
        }
    }
    relations
        .into_iter()
        .enumerate()
        .filter_map(|(index, (span, _))| selected.contains(&index).then_some(span))
        .collect()
}

fn select_relations(
    relations: &[(Span, BTreeSet<String>)],
    needed: &mut BTreeSet<String>,
    selected: &mut BTreeSet<usize>,
) {
    for (index, (_, refs)) in relations.iter().enumerate() {
        if !refs.is_disjoint(needed) {
            selected.insert(index);
            needed.extend(refs.iter().cloned());
        }
    }
}
