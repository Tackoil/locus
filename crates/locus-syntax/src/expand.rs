//! Binding-preserving function expansion and ordered collection normalization.
#![allow(
    missing_docs,
    reason = "Expansion records form the public integration contract"
)]

use crate::{
    Diagnostic, Span,
    ast::{
        CallStmt, ClassStmt, Constructor, Expr, File, FnStmt, GroupStmt, Ident, Kind, OpStmt,
        PropValue, RelStmt, Statement, StyleAttr,
    },
    limits::{MAX_SYNTAX_DEPTH, depth_diagnostic, expression_depth_exceeded},
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExpandOutput {
    pub file: File,
    pub collections: BTreeMap<String, Collection>,
    pub origins: Vec<ExpansionOrigin>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectionKind {
    Group,
    Instance,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Collection {
    pub id: String,
    pub kind: CollectionKind,
    pub members: Vec<String>,
    pub span: Span,
    pub origin: ExpansionOrigin,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExpansionOrigin {
    pub span: Span,
    pub invocations: Vec<InvocationOrigin>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InvocationOrigin {
    pub function: String,
    pub instance: String,
    pub span: Span,
}

#[derive(Clone, Default)]
struct Scope {
    bindings: BTreeMap<String, Expr>,
    declared: BTreeSet<String>,
    parameters: BTreeSet<String>,
    functions: BTreeSet<String>,
    namespace: String,
    fallback: Option<StyleAttr>,
    invocations: Vec<InvocationOrigin>,
}

#[derive(Clone)]
struct Definition {
    statement: FnStmt,
    scope: Scope,
}

struct Expander {
    output: ExpandOutput,
    functions: BTreeMap<String, Definition>,
    kinds: BTreeMap<String, Kind>,
    aliases: BTreeMap<String, String>,
    classes: BTreeSet<String>,
}

/// Expands named calls in declaration order. Groups remain in a separate collection table.
#[must_use]
pub fn expand(file: &File) -> ExpandOutput {
    let mut expander = Expander {
        output: ExpandOutput {
            file: File {
                statements: Vec::new(),
                span: file.span,
            },
            collections: BTreeMap::new(),
            origins: Vec::new(),
            diagnostics: Vec::new(),
        },
        functions: BTreeMap::new(),
        kinds: BTreeMap::new(),
        aliases: BTreeMap::new(),
        classes: crate::vocabulary::BUILTIN_ATTR_NAMES
            .iter()
            .map(|name| (*name).to_owned())
            .collect(),
    };
    let mut scope = Scope::default();
    for class in crate::vocabulary::BUILTIN_ATTR_NAMES {
        scope
            .bindings
            .insert((*class).to_owned(), reference(class, file.span));
    }
    expander.statements(&file.statements, &mut scope);
    expander.output
}

impl Expander {
    fn statements(&mut self, statements: &[Statement], scope: &mut Scope) {
        for statement in statements {
            self.statement(statement, scope);
        }
    }

    fn statement(&mut self, statement: &Statement, scope: &mut Scope) {
        if let Statement::Fn(function) = statement {
            self.define(function, scope);
            return;
        }
        if let Statement::Call(call) = statement {
            self.call(call, scope);
            return;
        }
        if let Statement::Group(group) = statement {
            self.group(group, scope);
            return;
        }
        match statement {
            Statement::Op(op) => self.op(op, scope),
            Statement::Rel(rel) => {
                let mut rel = rel.clone();
                rel.expr = self.expr(&rel.expr, scope);
                self.emit(Statement::Rel(rel), scope);
            }
            Statement::Assert(assertion) => {
                let mut assertion = assertion.clone();
                assertion.expr = self.expr(&assertion.expr, scope);
                self.emit(Statement::Assert(assertion), scope);
            }
            Statement::Class(class) => self.class(class, scope),
            _ => self.emit(statement.clone(), scope),
        }
    }

    fn define(&mut self, function: &FnStmt, scope: &mut Scope) {
        if crate::vocabulary::is_known_call(&function.name.name) {
            self.error(
                "E158",
                function.name.span,
                "a function cannot replace a built-in name",
                scope,
            );
            return;
        }
        if !self.declare(&function.name, scope) {
            return;
        }
        let mut names = BTreeSet::new();
        for parameter in &function.params {
            if !names.insert(parameter.name.clone()) {
                self.error(
                    "E134",
                    parameter.span,
                    format!("duplicate parameter `{}`", parameter.name),
                    scope,
                );
            }
        }
        self.check_body_names(function, &mut names, scope);
        self.check_body_bindings(function, scope);
        self.functions.insert(
            function.name.name.clone(),
            Definition {
                statement: function.clone(),
                scope: scope.clone(),
            },
        );
        scope.functions.insert(function.name.name.clone());
    }

    fn check_body_names(&mut self, function: &FnStmt, names: &mut BTreeSet<String>, scope: &Scope) {
        for statement in &function.body {
            self.check_local_name(statement, names, scope);
            if let Statement::Call(call) = statement {
                self.check_function_name(call, scope);
            }
        }
    }

    fn check_body_bindings(&mut self, function: &FnStmt, scope: &Scope) {
        let mut visible: BTreeSet<_> = scope.bindings.keys().cloned().collect();
        visible.extend(function.params.iter().map(|param| param.name.clone()));
        for statement in &function.body {
            self.check_statement_bindings(statement, &mut visible, scope);
        }
    }

    fn check_statement_bindings(
        &mut self,
        statement: &Statement,
        visible: &mut BTreeSet<String>,
        scope: &Scope,
    ) {
        if let Statement::Op(op) = statement {
            for expr in constructor_expressions(&op.ctor) {
                self.check_expr_bindings(expr, visible, scope);
            }
            visible.insert(op.id.name.clone());
            for expr in &op.where_clauses {
                self.check_expr_bindings(expr, visible, scope);
            }
            return;
        }
        for expr in statement_expressions(statement) {
            self.check_expr_bindings(expr, visible, scope);
        }
        if let Some(id) = declaration_name(statement) {
            visible.insert(id.name.clone());
        }
    }

    fn check_expr_bindings(&mut self, expr: &Expr, visible: &BTreeSet<String>, scope: &Scope) {
        if let Expr::Ref(ident) = expr {
            if !visible.contains(&ident.name) {
                self.error(
                    "E135",
                    ident.span,
                    format!("unknown or forward reference `{}`", ident.name),
                    scope,
                );
            }
        }
        let children = match expr {
            Expr::Call { name, args, .. } if name.name == "edge" => {
                args.first().into_iter().collect()
            }
            _ => expression_children(expr),
        };
        for child in children {
            self.check_expr_bindings(child, visible, scope);
        }
    }

    fn check_local_name(
        &mut self,
        statement: &Statement,
        names: &mut BTreeSet<String>,
        scope: &Scope,
    ) {
        let id = declaration_name(statement);
        let Some(id) = id else {
            return;
        };
        if !names.insert(id.name.clone()) {
            self.error(
                "E134",
                id.span,
                format!("duplicate local identifier `{}`", id.name),
                scope,
            );
        }
    }

    fn check_function_name(&mut self, call: &CallStmt, scope: &Scope) {
        if !scope.functions.contains(&call.function.name) {
            self.error(
                "E158",
                call.function.span,
                format!("unknown or forward function `{}`", call.function.name),
                scope,
            );
        }
    }

    fn call(&mut self, call: &CallStmt, scope: &mut Scope) {
        if scope.invocations.len() >= MAX_SYNTAX_DEPTH {
            self.depth_error(call.span, scope);
            return;
        }
        if !scope.functions.contains(&call.function.name) {
            self.error(
                "E158",
                call.function.span,
                format!("unknown or forward function `{}`", call.function.name),
                scope,
            );
            return;
        }
        let Some(definition) = self.functions.get(&call.function.name).cloned() else {
            return;
        };
        if definition.statement.params.len() != call.args.len() {
            self.error(
                "E159",
                call.span,
                format!(
                    "function `{}` expects {} arguments, received {}",
                    call.function.name,
                    definition.statement.params.len(),
                    call.args.len()
                ),
                scope,
            );
            return;
        }
        let args: Vec<_> = call.args.iter().map(|arg| self.expr(arg, scope)).collect();
        let fallback = call
            .attr
            .as_ref()
            .map(|attr| self.attr(attr, scope))
            .or_else(|| scope.fallback.clone());
        if !self.declare(&call.id, scope) {
            return;
        }
        let id = qualify(&scope.namespace, &call.id.name);
        let mut child = definition.scope;
        child.namespace.clone_from(&id);
        child.declared.clear();
        child.parameters.clear();
        child.fallback = fallback;
        child.invocations.clone_from(&scope.invocations);
        child.invocations.push(InvocationOrigin {
            function: call.function.name.clone(),
            instance: id.clone(),
            span: call.span,
        });
        for (param, arg) in definition.statement.params.iter().zip(args) {
            child.declared.insert(param.name.clone());
            child.parameters.insert(param.name.clone());
            child.bindings.insert(param.name.clone(), arg);
        }
        let start = self.output.file.statements.len();
        self.statements(&definition.statement.body, &mut child);
        let members = self.owned_members(start);
        let origin = origin(call.span, &child);
        self.output.collections.insert(
            id.clone(),
            Collection {
                id: id.clone(),
                kind: CollectionKind::Instance,
                members,
                span: call.span,
                origin,
            },
        );
        scope
            .bindings
            .insert(call.id.name.clone(), reference(&id, call.id.span));
    }

    fn owned_members(&self, start: usize) -> Vec<String> {
        self.output.file.statements[start..]
            .iter()
            .filter_map(|statement| {
                let Statement::Op(op) = statement else {
                    return None;
                };
                if op.kind == Kind::Num
                    || self.aliases.contains_key(&op.id.name)
                    || hidden_point(op)
                {
                    return None;
                }
                Some(op.id.name.clone())
            })
            .collect()
    }

    fn op(&mut self, op: &OpStmt, scope: &mut Scope) {
        let mut expanded = op.clone();
        expanded.id.name = qualify(&scope.namespace, &op.id.name);
        expanded.ctor = self.constructor(&op.ctor, scope);
        expanded.attr = op
            .attr
            .as_ref()
            .map(|attr| self.attr(attr, scope))
            .or_else(|| scope.fallback.clone());
        for prop in &mut expanded.props.items {
            if crate::parser::prop_uses_references(&prop.name.name) {
                if let PropValue::Expr(expr) = &mut prop.value {
                    *expr = self.expr(expr, scope);
                }
            }
        }
        if !self.declare(&op.id, scope) {
            return;
        }
        self.kinds.insert(expanded.id.name.clone(), op.kind);
        if let Constructor::Expr { expr, .. } = &expanded.ctor {
            if let Expr::Ref(ident) = expr.as_ref() {
                self.aliases
                    .insert(expanded.id.name.clone(), self.identity(&ident.name));
            }
        }
        scope
            .bindings
            .insert(op.id.name.clone(), Expr::Ref(expanded.id.clone()));
        expanded.where_clauses = op
            .where_clauses
            .iter()
            .map(|expr| self.expr(expr, scope))
            .collect();
        self.emit(Statement::Op(expanded), scope);
    }

    fn group(&mut self, group: &GroupStmt, scope: &mut Scope) {
        let mut members = Vec::new();
        for member in &group.members {
            let expr = self.expr(member, scope);
            self.group_member(&expr, group, scope, &mut members);
        }
        if !self.declare(&group.name, scope) {
            return;
        }
        let id = qualify(&scope.namespace, &group.name.name);
        let mut seen = BTreeSet::new();
        members.retain(|member| seen.insert(member.clone()));
        self.output.collections.insert(
            id.clone(),
            Collection {
                id: id.clone(),
                kind: CollectionKind::Group,
                members,
                span: group.span,
                origin: origin(group.span, scope),
            },
        );
        scope
            .bindings
            .insert(group.name.name.clone(), reference(&id, group.name.span));
    }

    fn group_member(
        &mut self,
        expr: &Expr,
        group: &GroupStmt,
        scope: &Scope,
        members: &mut Vec<String>,
    ) {
        let Expr::Ref(ident) = expr else {
            self.error(
                "E157",
                expr.span(),
                format!(
                    "group `{}` members must name existing objects or collections",
                    group.name.name
                ),
                scope,
            );
            return;
        };
        if let Some(collection) = self.output.collections.get(&ident.name) {
            if collection.kind == CollectionKind::Group {
                members.extend(collection.members.clone());
            } else {
                members.push(ident.name.clone());
            }
            return;
        }
        if self
            .kinds
            .get(&ident.name)
            .is_some_and(|kind| *kind != Kind::Num)
        {
            members.push(self.identity(&ident.name));
        } else {
            self.error(
                "E157",
                ident.span,
                format!(
                    "group `{}` has invalid member `{}`",
                    group.name.name, ident.name
                ),
                scope,
            );
        }
    }

    fn class(&mut self, class: &ClassStmt, scope: &mut Scope) {
        if !self.declare(&class.name, scope) {
            return;
        }
        self.classes.insert(class.name.name.clone());
        scope
            .bindings
            .insert(class.name.name.clone(), Expr::Ref(class.name.clone()));
        self.emit(Statement::Class(class.clone()), scope);
    }

    fn constructor(&mut self, ctor: &Constructor, scope: &Scope) -> Constructor {
        match ctor {
            Constructor::Call { name, args, span } => Constructor::Call {
                name: name.clone(),
                args: self.call_args(&name.name, args, scope),
                span: *span,
            },
            Constructor::Expr { expr, span } => Constructor::Expr {
                expr: Box::new(self.expr(expr, scope)),
                span: *span,
            },
            Constructor::FreeNum { hint, span } => Constructor::FreeNum {
                hint: Box::new(self.expr(hint, scope)),
                span: *span,
            },
        }
    }

    fn expr(&mut self, expr: &Expr, scope: &Scope) -> Expr {
        let value = self.expr_inner(expr, scope);
        if expression_depth_exceeded(&value) {
            self.depth_error(expr.span(), scope);
            return Expr::Boolean {
                value: false,
                span: expr.span(),
            };
        }
        value
    }

    fn expr_inner(&mut self, expr: &Expr, scope: &Scope) -> Expr {
        if let Expr::Ref(ident) = expr {
            return self.resolve(ident, scope);
        }
        if let Expr::Property {
            base,
            name,
            args,
            span,
        } = expr
        {
            return self.property(base, name, args, *span, scope);
        }
        if let Expr::Call { name, args, span } = expr {
            return Expr::Call {
                name: name.clone(),
                args: self.call_args(&name.name, args, scope),
                span: *span,
            };
        }
        if let Expr::Group { expr, span } = expr {
            return Expr::Group {
                expr: Box::new(self.expr(expr, scope)),
                span: *span,
            };
        }
        match expr {
            Expr::Unary { op, expr, span } => Expr::Unary {
                op: *op,
                expr: Box::new(self.expr(expr, scope)),
                span: *span,
            },
            Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
                op: *op,
                lhs: Box::new(self.expr(lhs, scope)),
                rhs: Box::new(self.expr(rhs, scope)),
                span: *span,
            },
            _ => expr.clone(),
        }
    }

    fn resolve(&mut self, ident: &Ident, scope: &Scope) -> Expr {
        if let Some(value) = scope.bindings.get(&ident.name) {
            if scope.parameters.contains(&ident.name) {
                return value.clone();
            }
            if let Expr::Ref(bound) = value {
                return reference(&bound.name, ident.span);
            }
            return value.clone();
        }
        self.error(
            "E135",
            ident.span,
            format!("unknown or forward reference `{}`", ident.name),
            scope,
        );
        Expr::Ref(ident.clone())
    }

    fn property(
        &mut self,
        base: &Expr,
        name: &Ident,
        args: &[Expr],
        span: Span,
        scope: &Scope,
    ) -> Expr {
        let base = self.expr(base, scope);
        if let Expr::Ref(ident) = &base {
            if self
                .output
                .collections
                .get(&ident.name)
                .is_some_and(|collection| collection.kind == CollectionKind::Instance)
            {
                return self.member(ident, name, args, span, scope);
            }
        }
        if !crate::vocabulary::PROPERTY_NAMES.contains(&name.name.as_str()) {
            self.error(
                "E148",
                name.span,
                format!("unknown property or anchor `{}`", name.name),
                scope,
            );
        }
        Expr::Property {
            base: Box::new(base),
            name: name.clone(),
            args: args.iter().map(|arg| self.expr(arg, scope)).collect(),
            span,
        }
    }

    fn member(
        &mut self,
        base: &Ident,
        name: &Ident,
        args: &[Expr],
        span: Span,
        scope: &Scope,
    ) -> Expr {
        let qualified = format!("{}.{}", base.name, name.name);
        if !args.is_empty()
            || (!self.kinds.contains_key(&qualified)
                && !self.output.collections.contains_key(&qualified))
        {
            self.error(
                "E160",
                name.span,
                format!("instance `{}` has no member `{}`", base.name, name.name),
                scope,
            );
        }
        reference(&qualified, span)
    }

    fn call_args(&mut self, name: &str, args: &[Expr], scope: &Scope) -> Vec<Expr> {
        args.iter()
            .enumerate()
            .map(|(index, arg)| {
                if name == "edge" && index == 1 {
                    return arg.clone();
                }
                self.expr(arg, scope)
            })
            .collect()
    }

    fn attr(&mut self, attr: &StyleAttr, scope: &Scope) -> StyleAttr {
        let resolved = self.resolve(&attr.name, scope);
        if let Expr::Ref(name) = resolved {
            if self.classes.contains(&name.name) {
                return StyleAttr {
                    name,
                    span: attr.span,
                };
            }
        }
        self.error(
            "E136",
            attr.span,
            "a style argument must name a previously declared style class",
            scope,
        );
        attr.clone()
    }

    fn identity(&self, id: &str) -> String {
        self.aliases
            .get(id)
            .cloned()
            .unwrap_or_else(|| id.to_owned())
    }

    fn declare(&mut self, id: &Ident, scope: &mut Scope) -> bool {
        if scope.declared.insert(id.name.clone()) {
            return true;
        }
        self.error(
            "E134",
            id.span,
            format!("duplicate identifier `{}`", id.name),
            scope,
        );
        false
    }

    fn emit(&mut self, statement: Statement, scope: &Scope) {
        self.output.origins.push(origin(statement.span(), scope));
        self.output.file.statements.push(statement);
    }

    fn depth_error(&mut self, span: Span, scope: &Scope) {
        let diagnostic = depth_diagnostic(span);
        self.error(diagnostic.code, diagnostic.span, diagnostic.message, scope);
    }

    fn error(&mut self, code: &'static str, span: Span, message: impl Into<String>, scope: &Scope) {
        let mut diagnostic = Diagnostic::new(code, span, message);
        for invocation in &scope.invocations {
            diagnostic = diagnostic.with_note(format!(
                "in `{}` invoked as `{}` at line {}",
                invocation.function, invocation.instance, invocation.span.line
            ));
        }
        self.output.diagnostics.push(diagnostic);
    }
}

fn qualify(namespace: &str, name: &str) -> String {
    if namespace.is_empty() {
        name.to_owned()
    } else {
        format!("{namespace}.{name}")
    }
}
fn reference(name: &str, span: Span) -> Expr {
    Expr::Ref(Ident {
        name: name.to_owned(),
        span,
    })
}
fn origin(span: Span, scope: &Scope) -> ExpansionOrigin {
    ExpansionOrigin {
        span,
        invocations: scope.invocations.clone(),
    }
}
fn hidden_point(op: &OpStmt) -> bool {
    op.kind == Kind::Point
        && op.props.items.iter().any(|prop| {
            prop.name.name == "mark"
                && matches!(&prop.value, PropValue::Expr(Expr::Ref(value)) if value.name == "none")
        })
}

pub(crate) fn validate_beta6_statement(statement: &Statement, diagnostics: &mut Vec<Diagnostic>) {
    match statement {
        Statement::Op(op) => validate_op(op, diagnostics),
        Statement::Rel(rel) => {
            validate_expr(&rel.expr, diagnostics);
            validate_relation_props(rel, diagnostics);
        }
        Statement::Assert(assertion) => {
            validate_expr(&assertion.expr, diagnostics);
            if matches!(&assertion.expr, Expr::Call { name, .. } if name.name == "trim_endpoints") {
                diagnostics.push(Diagnostic::new(
                    "E144",
                    assertion.expr.span(),
                    "`trim_endpoints` is a rendering operation, not a Boolean predicate",
                ));
            }
        }
        _ => {}
    }
}

fn validate_op(op: &OpStmt, diagnostics: &mut Vec<Diagnostic>) {
    match &op.ctor {
        Constructor::Call { name, args, span } => {
            validate_declaration_call(op, name, args, *span, diagnostics);
        }
        Constructor::Expr { expr, .. } => validate_expr(expr, diagnostics),
        Constructor::FreeNum { hint, .. } => validate_expr(hint, diagnostics),
    }
    for clause in &op.where_clauses {
        validate_expr(clause, diagnostics);
    }
    if op.kind == Kind::Arrow {
        validate_arrow_props(op, diagnostics);
    }
}

fn validate_declaration_call(
    op: &OpStmt,
    name: &Ident,
    args: &[Expr],
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if name.name == "point" && !args.is_empty() {
        diagnostics.push(Diagnostic::new(
            "E142",
            span,
            "`point()` takes no arguments",
        ));
    }
    for arg in args {
        validate_expr(arg, diagnostics);
    }
    if op.kind == Kind::Label && args.len() == 1 {
        reject_props(
            op,
            &["dir", "gap", "near", "side"],
            "standalone labels accept only `role`",
            diagnostics,
        );
    }
}

fn reject_props(op: &OpStmt, rejected: &[&str], message: &str, diagnostics: &mut Vec<Diagnostic>) {
    for prop in &op.props.items {
        if rejected.contains(&prop.name.name.as_str()) {
            diagnostics.push(Diagnostic::new("E131", prop.span, message));
        }
    }
}

fn validate_arrow_props(op: &OpStmt, diagnostics: &mut Vec<Diagnostic>) {
    let automatic = op.props.items.iter().any(|prop| prop.name.name == "route" && matches!(&prop.value, PropValue::Expr(Expr::Ref(value)) if matches!(value.name.as_str(), "curved" | "orthogonal")));
    if automatic {
        reject_props(
            op,
            &["bend"],
            "automatic routes do not accept `bend`",
            diagnostics,
        );
    } else {
        reject_props(
            op,
            &["side"],
            "direct routes do not accept `side`",
            diagnostics,
        );
    }
}

fn validate_relation_props(rel: &RelStmt, diagnostics: &mut Vec<Diagnostic>) {
    let allowed = matches!(&rel.expr, Expr::Call { name, .. } if matches!(name.name.as_str(), "inset_left" | "inset_right" | "inset_top" | "inset_bottom"));
    for prop in &rel.props.items {
        let none = matches!(&prop.value, PropValue::Expr(Expr::Ref(value)) if value.name == "none");
        if !allowed || prop.name.name != "padding" || !none {
            diagnostics.push(Diagnostic::new(
                "E131",
                prop.span,
                "only a standalone inset relation accepts `{ padding: none }`",
            ));
        }
    }
}

fn validate_expr(expr: &Expr, diagnostics: &mut Vec<Diagnostic>) {
    if let Expr::Call { name, args, .. } = expr {
        if crate::vocabulary::LEGACY_PREDICATE_NAMES.contains(&name.name.as_str()) {
            diagnostics.push(Diagnostic::new(
                "E144",
                name.span,
                format!("`{}` is not part of the beta 6 vocabulary", name.name),
            ));
        }
        validate_nested_placement(name, args, diagnostics);
    }
    for child in expression_children(expr) {
        validate_expr(child, diagnostics);
    }
}

fn expression_children(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::Call { args, .. } => args.iter().collect(),
        Expr::Property { base, args, .. } => {
            std::iter::once(base.as_ref()).chain(args.iter()).collect()
        }
        Expr::Group { expr, .. } | Expr::Unary { expr, .. } => vec![expr],
        Expr::Binary { lhs, rhs, .. } => vec![lhs, rhs],
        _ => Vec::new(),
    }
}

fn constructor_expressions(ctor: &Constructor) -> Vec<&Expr> {
    match ctor {
        Constructor::Call { name, args, .. } if name.name == "edge" => {
            args.first().into_iter().collect()
        }
        Constructor::Call { args, .. } => args.iter().collect(),
        Constructor::Expr { expr, .. } => vec![expr],
        Constructor::FreeNum { hint, .. } => vec![hint],
    }
}

fn statement_expressions(statement: &Statement) -> Vec<&Expr> {
    match statement {
        Statement::Call(call) => call.args.iter().collect(),
        Statement::Group(group) => group.members.iter().collect(),
        Statement::Rel(rel) => vec![&rel.expr],
        Statement::Assert(assertion) => vec![&assertion.expr],
        _ => Vec::new(),
    }
}

fn declaration_name(statement: &Statement) -> Option<&Ident> {
    match statement {
        Statement::Op(op) => Some(&op.id),
        Statement::Group(group) => Some(&group.name),
        Statement::Call(call) => Some(&call.id),
        _ => None,
    }
}

fn validate_nested_placement(name: &Ident, args: &[Expr], diagnostics: &mut Vec<Diagnostic>) {
    let placed = match name.name.as_str() {
        "point" => true,
        "circle" | "label" => args.len() == 1,
        "ellipse" | "rect" | "diamond" => args.len() <= 2,
        "rounded_rect" => args.len() <= 3,
        _ => false,
    };
    if placed {
        diagnostics.push(Diagnostic::new(
            "E156",
            name.span,
            "placement-owning constructors require a named constructor declaration",
        ));
    }
}
