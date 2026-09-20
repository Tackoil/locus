//! Dependency extraction and orphan detection for future source edits.

use std::collections::BTreeSet;

use indexmap::{IndexMap, IndexSet};
use locus_syntax::{Constructor, Expr, File, PropValue, Span, Statement};
use thiserror::Error;

/// Stable E3xx dependency diagnostic.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{code} at {line}:{col}: {message}", line = .span.line, col = .span.col)]
pub struct DependencyError {
    /// Stable E3xx code.
    pub code: &'static str,
    /// Reference or declaration source span.
    pub span: Span,
    /// Human-readable diagnostic.
    pub message: String,
}

/// Source-order dependency graph keyed by declaration id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DependencyGraph {
    dependencies: IndexMap<String, IndexSet<String>>,
    statement_dependencies: Vec<StatementDependency>,
}

/// A relation or assertion that references a declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatementDependency {
    /// Kind of top-level statement containing the reference.
    pub kind: StatementDependencyKind,
    /// Referenced declaration id.
    pub referenced_id: String,
    /// Span of the reference expression.
    pub span: Span,
}

/// A non-declaration statement kind that can block removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatementDependencyKind {
    /// A solver relation.
    Relation,
    /// A post-solve assertion.
    Assertion,
}

impl StatementDependencyKind {
    /// Returns the source keyword for this statement kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relation => "rel",
            Self::Assertion => "assert",
        }
    }
}

/// Deterministic set of declarations and statements affected by a removal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemovalImpact {
    /// Transitively orphaned declarations in file order.
    pub orphaned_declarations: Vec<String>,
    /// Relations and assertions referencing the removed declaration closure in file order.
    pub dependent_statements: Vec<StatementDependency>,
}

impl RemovalImpact {
    /// Returns true when removing the target leaves no dependent source behind.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.orphaned_declarations.is_empty() && self.dependent_statements.is_empty()
    }
}

impl DependencyGraph {
    /// Extracts dependencies and independently rejects unknown/forward references.
    pub fn extract(file: &File) -> Result<Self, DependencyError> {
        let mut graph = Self::default();
        let mut defined = BTreeSet::new();
        for statement in &file.statements {
            if let Some((kind, expression)) = statement_expression(statement) {
                graph.record_statement_dependencies(kind, expression, &defined)?;
                continue;
            }
            let Statement::Op(op) = statement else {
                continue;
            };
            let mut refs = IndexMap::new();
            collect_constructor_refs(&op.ctor, &mut refs);
            for clause in &op.where_clauses {
                collect_expr_refs(clause, &mut refs);
            }
            for prop in &op.props.items {
                if property_carries_dependencies(&prop.name.name)
                    && let PropValue::Expr(expr) = &prop.value
                {
                    collect_expr_refs(expr, &mut refs);
                }
            }
            refs.shift_remove(&op.id.name);
            validate_references(&refs, &defined)?;
            graph.dependencies.insert(
                op.id.name.clone(),
                refs.into_keys().collect::<IndexSet<_>>(),
            );
            defined.insert(op.id.name.clone());
        }
        Ok(graph)
    }

    fn record_statement_dependencies(
        &mut self,
        kind: StatementDependencyKind,
        expression: &Expr,
        defined: &BTreeSet<String>,
    ) -> Result<(), DependencyError> {
        let mut refs = IndexMap::new();
        collect_expr_refs(expression, &mut refs);
        validate_references(&refs, defined)?;
        self.statement_dependencies
            .extend(
                refs.into_iter()
                    .map(|(referenced_id, span)| StatementDependency {
                        kind,
                        referenced_id,
                        span,
                    }),
            );
        Ok(())
    }

    /// Returns direct dependencies in first-reference order.
    #[must_use]
    pub fn dependencies_of(&self, id: &str) -> Option<&IndexSet<String>> {
        self.dependencies.get(id)
    }

    /// Returns all declarations orphaned by removing `id`, transitively in file order.
    #[must_use]
    pub fn orphans_after_removing(&self, id: &str) -> Vec<String> {
        let mut removed = BTreeSet::from([id.to_owned()]);
        let mut changed = true;
        while changed {
            changed = false;
            for (candidate, dependencies) in &self.dependencies {
                if !removed.contains(candidate)
                    && dependencies
                        .iter()
                        .any(|dependency| removed.contains(dependency))
                {
                    changed |= removed.insert(candidate.clone());
                }
            }
        }
        self.dependencies
            .keys()
            .filter(|candidate| candidate.as_str() != id && removed.contains(*candidate))
            .cloned()
            .collect()
    }

    /// Returns every declaration and statement made invalid by removing `id`.
    #[must_use]
    pub fn removal_impact(&self, id: &str) -> RemovalImpact {
        let orphaned_declarations = self.orphans_after_removing(id);
        let removed = orphaned_declarations
            .iter()
            .cloned()
            .chain(std::iter::once(id.to_owned()))
            .collect::<BTreeSet<_>>();
        let dependent_statements = self
            .statement_dependencies
            .iter()
            .filter(|dependency| removed.contains(&dependency.referenced_id))
            .cloned()
            .collect();
        RemovalImpact {
            orphaned_declarations,
            dependent_statements,
        }
    }

    /// Iterates graph nodes and their direct dependencies in file order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &IndexSet<String>)> {
        self.dependencies
            .iter()
            .map(|(id, dependencies)| (id.as_str(), dependencies))
    }
}

fn statement_expression(statement: &Statement) -> Option<(StatementDependencyKind, &Expr)> {
    match statement {
        Statement::Rel(relation) => Some((StatementDependencyKind::Relation, &relation.expr)),
        Statement::Assert(assertion) => Some((StatementDependencyKind::Assertion, &assertion.expr)),
        Statement::Op(_) | Statement::Meta(_) | Statement::Style(_) | Statement::Class(_) => None,
    }
}

fn property_carries_dependencies(name: &str) -> bool {
    matches!(name, "gap" | "bend" | "n" | "size" | "anchor")
}

fn validate_references(
    refs: &IndexMap<String, Span>,
    defined: &BTreeSet<String>,
) -> Result<(), DependencyError> {
    if let Some((name, span)) = refs.iter().find(|(name, _)| !defined.contains(*name)) {
        return Err(DependencyError {
            code: "E301",
            span: *span,
            message: format!("unknown or forward reference `{name}`"),
        });
    }
    Ok(())
}

fn collect_constructor_refs(constructor: &Constructor, refs: &mut IndexMap<String, Span>) {
    match constructor {
        Constructor::Call { args, .. } => collect_expr_list_refs(args, refs),
        Constructor::Expr { expr, .. } | Constructor::FreeNum { hint: expr, .. } => {
            collect_expr_refs(expr, refs);
        }
    }
}

fn collect_expr_list_refs(expressions: &[Expr], refs: &mut IndexMap<String, Span>) {
    for expression in expressions {
        collect_expr_refs(expression, refs);
    }
}

fn collect_expr_refs(expression: &Expr, refs: &mut IndexMap<String, Span>) {
    if collect_leaf_ref(expression, refs) {
        return;
    }
    collect_compound_refs(expression, refs);
}

fn collect_leaf_ref(expression: &Expr, refs: &mut IndexMap<String, Span>) -> bool {
    match expression {
        Expr::Ref(id) => {
            refs.entry(id.name.clone()).or_insert(id.span);
            true
        }
        Expr::Number(_) | Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) => true,
        _ => false,
    }
}

fn collect_compound_refs(expression: &Expr, refs: &mut IndexMap<String, Span>) {
    match expression {
        Expr::Call { args, .. } => collect_expr_list_refs(args, refs),
        Expr::Property { base, args, .. } => {
            collect_expr_refs(base, refs);
            collect_expr_list_refs(args, refs);
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => collect_expr_refs(expr, refs),
        Expr::Binary { lhs, rhs, .. } => {
            collect_expr_refs(lhs, refs);
            collect_expr_refs(rhs, refs);
        }
        Expr::Number(_) | Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_) | Expr::Ref(_) => {}
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use locus_syntax::parse;

    use super::*;

    #[test]
    fn reports_transitive_orphans_in_file_order() {
        let parsed =
            parse("locus 0 beta 4\nnum a = 2\nnum b = a + 1\nnum c = b * 2\nnum spare = 4\n");
        let graph = DependencyGraph::extract(&parsed.file).expect("valid dependency graph");
        assert_eq!(graph.orphans_after_removing("a"), ["b", "c"]);
        assert!(graph.orphans_after_removing("spare").is_empty());
    }

    #[test]
    fn independently_rejects_forward_references() {
        let parsed = parse("locus 0 beta 4\nnum a = later + 1\nnum later = 2\n");
        let error = DependencyGraph::extract(&parsed.file).expect_err("forward reference");
        assert_eq!(error.code, "E301");
        assert_eq!(error.span.line, 2);
    }

    #[test]
    fn removal_impact_includes_relations_and_assertions_in_file_order() {
        let parsed =
            parse("locus 0 beta 4\nnum a = free ~1\nrel a == 1\nassert a == 1\nnum spare = 2\n");
        let graph = DependencyGraph::extract(&parsed.file).expect("valid dependency graph");
        let impact = graph.removal_impact("a");
        assert!(impact.orphaned_declarations.is_empty());
        assert_eq!(impact.dependent_statements.len(), 2);
        assert_eq!(
            impact.dependent_statements[0].kind,
            StatementDependencyKind::Relation
        );
        assert_eq!(impact.dependent_statements[0].span.line, 3);
        assert_eq!(
            impact.dependent_statements[1].kind,
            StatementDependencyKind::Assertion
        );
        assert_eq!(impact.dependent_statements[1].span.line, 4);
    }
}
