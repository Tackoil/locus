//! Stable JSON interchange derived from the source AST.
#![allow(
    missing_docs,
    reason = "interchange fields mirror the normative JSON contract directly"
)]

use crate::FORMAT_VERSION;
use crate::ast::{
    AngleUnit, BinOp, Constructor, Expr, File, HeadKind, Kind, NumberExpr, PropValue, Props,
    Statement, TextSegmentKind, TextValue, UnaryOp,
};
use crate::format::format_expr;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// The Locus 0 beta 5 JSON interchange document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Interchange {
    pub version: String,
    pub meta: Vec<MetaEntry>,
    pub style: String,
    pub classes: Vec<Class>,
    pub ops: Vec<Op>,
    pub rels: Vec<ExprNode>,
    pub asserts: Vec<Assert>,
}

/// One metadata entry in source order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MetaEntry {
    pub key: String,
    pub value: Vec<TextSegment>,
}

/// One plain or TeX text segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TextSegment {
    pub kind: TextSegmentKind,
    pub source: String,
}

/// One declared style class.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Class {
    pub name: String,
    pub props: Vec<Property>,
}

/// One operation in source order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Op {
    pub id: String,
    pub kind: Kind,
    pub def: Option<Definition>,
    pub hint: Option<Hint>,
    #[serde(rename = "where")]
    pub where_clauses: Vec<ExprNode>,
    pub attr: Option<String>,
    pub props: Vec<Property>,
}

/// A definition retains both canonical source and its parsed tree.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Definition {
    pub source: String,
    pub tree: ExprNode,
}

/// A free scalar or point hint.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Hint {
    Scalar(ExprNode),
    Point([ExprNode; 2]),
}

/// One assertion in source order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Assert {
    pub expr: ExprNode,
    pub tolerance: Option<NumberNode>,
}

/// One property in source order.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Property {
    pub name: String,
    pub value: PropertyValue,
}

/// A JSON property value.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PropertyValue {
    Expr { expr: ExprNode },
    Text { segments: Vec<TextSegment> },
    Head { value: HeadKind },
    Color { value: String },
}

/// Inferred numeric dimensions from specification §6.1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Scalar,
    Length,
    Area,
    Angle,
}

/// A parsed expression tree without source spans.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExprNode {
    Number(NumberNode),
    Boolean {
        value: bool,
    },
    Text {
        segments: Vec<TextSegment>,
    },
    Ref {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimension: Option<Dimension>,
    },
    Call {
        name: String,
        args: Vec<ExprNode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimension: Option<Dimension>,
    },
    Property {
        base: Box<ExprNode>,
        name: String,
        args: Vec<ExprNode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimension: Option<Dimension>,
    },
    Group {
        expr: Box<ExprNode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimension: Option<Dimension>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<ExprNode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimension: Option<Dimension>,
    },
    Binary {
        op: BinOp,
        lhs: Box<ExprNode>,
        rhs: Box<ExprNode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimension: Option<Dimension>,
    },
}

/// A deterministic finite number node.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NumberNode {
    pub source: String,
    pub value: f64,
    pub dimension: Dimension,
}

/// Converts a parsed file into the stable interchange representation.
#[must_use]
pub fn to_interchange(file: &File) -> Interchange {
    let dimensions = Dimensions::infer(file);
    let mut output = Interchange {
        version: FORMAT_VERSION.to_owned(),
        meta: Vec::new(),
        style: "textbook".to_owned(),
        classes: Vec::new(),
        ops: Vec::new(),
        rels: Vec::new(),
        asserts: Vec::new(),
    };
    for statement in &file.statements {
        push_statement(&mut output, statement, &dimensions);
    }
    output
}

/// Returns inferred numeric dimensions keyed by each expression's UTF-8 byte range.
#[must_use]
pub fn inferred_expression_dimensions(file: &File) -> BTreeMap<(usize, usize), Dimension> {
    Dimensions::infer(file).expression_dimensions()
}

/// Serializes interchange JSON without insignificant whitespace.
///
/// # Errors
///
/// Returns an error if serde cannot serialize the interchange document.
pub fn to_json(file: &File) -> Result<String, serde_json::Error> {
    serde_json::to_string(&to_interchange(file))
}

/// Serializes interchange JSON with stable two-space indentation.
///
/// # Errors
///
/// Returns an error if serde cannot serialize the interchange document.
pub fn to_json_pretty(file: &File) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&to_interchange(file))
}

fn push_statement(output: &mut Interchange, statement: &Statement, dimensions: &Dimensions) {
    match statement {
        Statement::Meta(entry) => output.meta.push(MetaEntry {
            key: entry.key.name.clone(),
            value: text_segments(&entry.value),
        }),
        Statement::Style(style) => output.style.clone_from(&style.name.name),
        Statement::Class(class) => output.classes.push(Class {
            name: class.name.name.clone(),
            props: properties(&class.props, dimensions),
        }),
        Statement::Op(op) => output.ops.push(op_node(op, dimensions)),
        Statement::Rel(rel) => output.rels.push(expr_node(&rel.expr, dimensions)),
        Statement::Assert(assertion) => output.asserts.push(Assert {
            expr: expr_node(&assertion.expr, dimensions),
            tolerance: assertion
                .tolerance
                .as_ref()
                .map(|number| number_node(number, Dimension::Scalar)),
        }),
    }
}

fn op_node(op: &crate::ast::OpStmt, dimensions: &Dimensions) -> Op {
    let (def, hint) = definition_and_hint(&op.ctor, dimensions);
    Op {
        id: op.id.name.clone(),
        kind: op.kind,
        def,
        hint,
        where_clauses: op
            .where_clauses
            .iter()
            .map(|expr| expr_node(expr, dimensions))
            .collect(),
        attr: op.attr.as_ref().map(|attr| attr.name.name.clone()),
        props: properties(&op.props, dimensions),
    }
}

fn definition_and_hint(
    constructor: &Constructor,
    dimensions: &Dimensions,
) -> (Option<Definition>, Option<Hint>) {
    match constructor {
        Constructor::Call { name, args, .. } => {
            let tree = ExprNode::Call {
                name: name.name.clone(),
                args: args.iter().map(|arg| expr_node(arg, dimensions)).collect(),
                dimension: None,
            };
            (
                Some(Definition {
                    source: format!(
                        "{}({})",
                        name.name,
                        args.iter().map(format_expr).collect::<Vec<_>>().join(", ")
                    ),
                    tree,
                }),
                None,
            )
        }
        Constructor::Expr { expr, .. } => (
            Some(Definition {
                source: format_expr(expr),
                tree: expr_node(expr, dimensions),
            }),
            None,
        ),
        Constructor::FreeNum { hint, .. } => {
            (None, Some(Hint::Scalar(expr_node(hint, dimensions))))
        }
    }
}

fn properties(props: &Props, dimensions: &Dimensions) -> Vec<Property> {
    props
        .items
        .iter()
        .map(|prop| Property {
            name: prop.name.name.clone(),
            value: property_value(&prop.value, dimensions),
        })
        .collect()
}

fn property_value(value: &PropValue, dimensions: &Dimensions) -> PropertyValue {
    match value {
        PropValue::Expr(expr) => PropertyValue::Expr {
            expr: expr_node(expr, dimensions),
        },
        PropValue::Text(text) => PropertyValue::Text {
            segments: text_segments(text),
        },
        PropValue::Head(head) => PropertyValue::Head { value: head.kind },
        PropValue::HexColor(color) => PropertyValue::Color {
            value: format!("#{}", color.value),
        },
    }
}

fn text_segments(text: &TextValue) -> Vec<TextSegment> {
    text.segments
        .iter()
        .map(|segment| TextSegment {
            kind: segment.kind,
            source: segment.source.clone(),
        })
        .collect()
}

fn expr_node(expr: &Expr, dimensions: &Dimensions) -> ExprNode {
    if let Expr::Number(number) = expr {
        return ExprNode::Number(number_node(
            number,
            dimensions.dimension(expr).unwrap_or(Dimension::Scalar),
        ));
    }
    if let Expr::Boolean { value, .. } = expr {
        return ExprNode::Boolean { value: *value };
    }
    if let Expr::String(text) | Expr::Tex(text) = expr {
        return ExprNode::Text {
            segments: text_segments(text),
        };
    }
    if let Expr::Ref(ident) = expr {
        return ExprNode::Ref {
            id: ident.name.clone(),
            dimension: dimensions.dimension(expr),
        };
    }
    if let Expr::Call { name, args, .. } = expr {
        return ExprNode::Call {
            name: name.name.clone(),
            args: args.iter().map(|arg| expr_node(arg, dimensions)).collect(),
            dimension: dimensions.dimension(expr),
        };
    }
    if let Expr::Property {
        base, name, args, ..
    } = expr
    {
        return ExprNode::Property {
            base: Box::new(expr_node(base, dimensions)),
            name: name.name.clone(),
            args: args.iter().map(|arg| expr_node(arg, dimensions)).collect(),
            dimension: dimensions.dimension(expr),
        };
    }
    if let Expr::Group { expr: inner, .. } = expr {
        return ExprNode::Group {
            expr: Box::new(expr_node(inner, dimensions)),
            dimension: dimensions.dimension(expr),
        };
    }
    if let Expr::Unary {
        op, expr: inner, ..
    } = expr
    {
        return ExprNode::Unary {
            op: *op,
            expr: Box::new(expr_node(inner, dimensions)),
            dimension: dimensions.dimension(expr),
        };
    }
    let Expr::Binary { op, lhs, rhs, .. } = expr else {
        unreachable!("all expression variants handled")
    };
    ExprNode::Binary {
        op: *op,
        lhs: Box::new(expr_node(lhs, dimensions)),
        rhs: Box::new(expr_node(rhs, dimensions)),
        dimension: dimensions.dimension(expr),
    }
}

fn number_node(number: &NumberExpr, dimension: Dimension) -> NumberNode {
    let unit = match number.unit {
        Some(AngleUnit::Deg) => "deg",
        Some(AngleUnit::Rad) => "rad",
        None => "",
    };
    NumberNode {
        source: format!("{}{unit}", number.raw),
        value: number.value,
        dimension,
    }
}

type ExprKey = (usize, usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DimensionSet(u8);

impl DimensionSet {
    const ALL: Self = Self(0b1111);
    const EMPTY: Self = Self(0);
    const UNITLESS: Self = Self(0b0111);

    fn singleton(dimension: Dimension) -> Self {
        Self(1 << dimension_index(dimension))
    }

    fn contains(self, dimension: Dimension) -> bool {
        self.0 & Self::singleton(dimension).0 != 0
    }

    fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    fn insert(&mut self, dimension: Dimension) {
        self.0 |= Self::singleton(dimension).0;
    }

    fn known(self) -> Option<Dimension> {
        match self.0 {
            0b0001 => Some(Dimension::Scalar),
            0b0010 => Some(Dimension::Length),
            0b0100 => Some(Dimension::Area),
            0b1000 => Some(Dimension::Angle),
            _ => None,
        }
    }

    fn width(self) -> u32 {
        self.0.count_ones()
    }
}

const fn dimension_index(dimension: Dimension) -> u8 {
    match dimension {
        Dimension::Scalar => 0,
        Dimension::Length => 1,
        Dimension::Area => 2,
        Dimension::Angle => 3,
    }
}

struct Dimensions {
    parent: Vec<usize>,
    domains: Vec<DimensionSet>,
    exprs: BTreeMap<ExprKey, usize>,
    nums: BTreeMap<String, usize>,
    kinds: BTreeMap<String, Kind>,
    products: Vec<Product>,
    scalar_defaults: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Product {
    op: ProductOp,
    lhs: usize,
    rhs: usize,
    result: usize,
}

#[derive(Clone, Copy)]
enum ProductOp {
    Mul,
    Div,
    Sqrt,
}

struct ProductGroup {
    products: Vec<Product>,
    roots: BTreeSet<usize>,
}

impl Dimensions {
    fn infer(file: &File) -> Self {
        let mut dimensions = Self {
            parent: Vec::new(),
            domains: Vec::new(),
            exprs: BTreeMap::new(),
            nums: BTreeMap::new(),
            kinds: BTreeMap::new(),
            products: Vec::new(),
            scalar_defaults: Vec::new(),
        };
        dimensions.declare_names(file);
        dimensions.visit_file(file);
        dimensions.solve();
        dimensions
    }

    fn declare_names(&mut self, file: &File) {
        for statement in &file.statements {
            let Statement::Op(op) = statement else {
                continue;
            };
            self.kinds.insert(op.id.name.clone(), op.kind);
            if op.kind == Kind::Num {
                let variable = self.new_var(None);
                self.nums.insert(op.id.name.clone(), variable);
            }
        }
    }

    fn visit_file(&mut self, file: &File) {
        for statement in &file.statements {
            self.visit_statement(statement);
        }
    }

    fn visit_statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Op(op) => self.visit_op(op),
            Statement::Rel(rel) => {
                self.visit_expr(&rel.expr);
            }
            Statement::Assert(assertion) => {
                self.visit_expr(&assertion.expr);
            }
            Statement::Class(class) => self.visit_props(&class.props),
            Statement::Meta(_) | Statement::Style(_) => {}
        }
    }

    fn visit_op(&mut self, op: &crate::ast::OpStmt) {
        self.visit_constructor(&op.id.name, &op.ctor);
        self.visit_exprs(&op.where_clauses);
        self.visit_props(&op.props);
    }

    fn visit_constructor(&mut self, id: &str, constructor: &Constructor) {
        match constructor {
            Constructor::Call { name, args, .. } => {
                self.visit_call(&name.name, args, None);
            }
            Constructor::Expr { expr, .. } => {
                if let (Some(expr_var), Some(num_var)) =
                    (self.visit_expr(expr), self.nums.get(id).copied())
                {
                    self.unify(expr_var, num_var);
                }
            }
            Constructor::FreeNum { hint, .. } => {
                if let (Some(hint_var), Some(num_var)) =
                    (self.visit_expr(hint), self.nums.get(id).copied())
                {
                    self.unify(hint_var, num_var);
                }
            }
        }
    }

    fn visit_exprs(&mut self, expressions: &[Expr]) {
        for expression in expressions {
            self.visit_expr(expression);
        }
    }

    fn visit_props(&mut self, props: &Props) {
        for prop in &props.items {
            let PropValue::Expr(expr) = &prop.value else {
                continue;
            };
            let expected = prop_dimension(&prop.name.name);
            if let Some(dimension) = expected {
                self.expect_expr(expr, dimension);
            } else {
                self.visit_expr(expr);
            }
        }
    }

    fn visit_expr(&mut self, expr: &Expr) -> Option<usize> {
        if let Expr::Number(number) = expr {
            let fixed = number.unit.map(|_| Dimension::Angle);
            let variable = self.expr_var(expr, fixed);
            if number.unit.is_none() {
                self.restrict(variable, DimensionSet::UNITLESS);
                self.scalar_defaults.push(variable);
            }
            return Some(variable);
        }
        if matches!(expr, Expr::Boolean { .. } | Expr::String(_) | Expr::Tex(_)) {
            return None;
        }
        if let Expr::Ref(ident) = expr {
            return self.visit_ref(expr, &ident.name);
        }
        if let Expr::Call { name, args, .. } = expr {
            return self.visit_call(&name.name, args, Some(expr));
        }
        if let Expr::Property {
            base, name, args, ..
        } = expr
        {
            return self.visit_property(expr, base, &name.name, args);
        }
        if let Expr::Group { expr: inner, .. } = expr {
            return self.visit_wrapped(expr, inner);
        }
        if let Expr::Unary {
            op, expr: inner, ..
        } = expr
        {
            return self.visit_unary(expr, *op, inner);
        }
        let Expr::Binary { op, lhs, rhs, .. } = expr else {
            unreachable!("all expression variants handled")
        };
        self.visit_binary(expr, *op, lhs, rhs)
    }

    fn visit_ref(&mut self, expr: &Expr, name: &str) -> Option<usize> {
        let num_var = self.nums.get(name).copied()?;
        let expr_var = self.expr_var(expr, None);
        self.unify(expr_var, num_var);
        Some(expr_var)
    }

    fn visit_wrapped(&mut self, expr: &Expr, inner: &Expr) -> Option<usize> {
        let inner_var = self.visit_expr(inner)?;
        let expr_var = self.expr_var(expr, None);
        self.unify(expr_var, inner_var);
        Some(expr_var)
    }

    fn visit_unary(&mut self, expr: &Expr, op: UnaryOp, inner: &Expr) -> Option<usize> {
        let inner_var = self.visit_expr(inner)?;
        if op == UnaryOp::Not {
            return None;
        }
        let expr_var = self.expr_var(expr, None);
        self.unify(expr_var, inner_var);
        Some(expr_var)
    }

    fn visit_binary(&mut self, expr: &Expr, op: BinOp, lhs: &Expr, rhs: &Expr) -> Option<usize> {
        let lhs_var = self.visit_expr(lhs);
        let rhs_var = self.visit_expr(rhs);
        if matches!(op, BinOp::And | BinOp::Or) {
            return None;
        }
        if matches!(
            op,
            BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
        ) {
            self.unify_options(lhs_var, rhs_var);
            return None;
        }
        let (Some(lhs_var), Some(rhs_var)) = (lhs_var, rhs_var) else {
            return None;
        };
        let result = self.expr_var(expr, None);
        if matches!(op, BinOp::Add | BinOp::Sub) {
            self.unify(lhs_var, rhs_var);
            self.unify(lhs_var, result);
        } else {
            self.products.push(Product {
                op: if op == BinOp::Mul {
                    ProductOp::Mul
                } else {
                    ProductOp::Div
                },
                lhs: lhs_var,
                rhs: rhs_var,
                result,
            });
        }
        Some(result)
    }

    fn visit_call(&mut self, name: &str, args: &[Expr], expr: Option<&Expr>) -> Option<usize> {
        let arg_vars: Vec<_> = args.iter().map(|arg| self.visit_expr(arg)).collect();
        self.constrain_call_args(name, &arg_vars);
        let result_dimension = fixed_call_dimension(name);
        if let Some(dimension) = result_dimension {
            return expr.map(|expr| self.expr_var(expr, Some(dimension)));
        }
        if matches!(name, "abs" | "min" | "max") {
            return self.preserved_call(expr, &arg_vars);
        }
        if name == "sqrt" {
            return self.sqrt_call(expr, &arg_vars);
        }
        None
    }

    fn constrain_call_args(&mut self, name: &str, args: &[Option<usize>]) {
        for (index, dimension) in call_arg_dimensions(name, args) {
            if let Some(variable) = args.get(*index).copied().flatten() {
                self.constrain(variable, *dimension);
            }
        }
        if name == "ratio" || matches!(name, "min" | "max") {
            self.unify_options(
                args.first().copied().flatten(),
                args.get(1).copied().flatten(),
            );
        }
    }

    fn preserved_call(&mut self, expr: Option<&Expr>, args: &[Option<usize>]) -> Option<usize> {
        let expr_var = self.expr_var(expr?, None);
        if let Some(arg_var) = args.first().copied().flatten() {
            self.unify(expr_var, arg_var);
        }
        Some(expr_var)
    }

    fn sqrt_call(&mut self, expr: Option<&Expr>, args: &[Option<usize>]) -> Option<usize> {
        let expr_var = self.expr_var(expr?, None);
        if let Some(arg_var) = args.first().copied().flatten() {
            self.products.push(Product {
                op: ProductOp::Sqrt,
                lhs: arg_var,
                rhs: arg_var,
                result: expr_var,
            });
        }
        Some(expr_var)
    }

    fn visit_property(
        &mut self,
        expr: &Expr,
        base: &Expr,
        name: &str,
        args: &[Expr],
    ) -> Option<usize> {
        self.visit_expr(base);
        for arg in args {
            self.visit_expr(arg);
        }
        if name == "at" {
            if let Some(arg) = args.first() {
                self.expect_expr(arg, Dimension::Angle);
            }
            return None;
        }
        matches!(name, "width" | "height" | "corner_radius")
            .then(|| self.expr_var(expr, Some(Dimension::Length)))
    }

    fn expect_expr(&mut self, expr: &Expr, dimension: Dimension) {
        if let Some(variable) = self.visit_expr(expr) {
            self.constrain(variable, dimension);
        }
    }

    fn expr_var(&mut self, expr: &Expr, fixed: Option<Dimension>) -> usize {
        let key = (expr.span().byte_start, expr.span().byte_end);
        if let Some(variable) = self.exprs.get(&key).copied() {
            if let Some(dimension) = fixed {
                self.constrain(variable, dimension);
            }
            return variable;
        }
        let variable = self.new_var(fixed);
        self.exprs.insert(key, variable);
        variable
    }

    fn new_var(&mut self, fixed: Option<Dimension>) -> usize {
        let variable = self.parent.len();
        self.parent.push(variable);
        self.domains
            .push(fixed.map_or(DimensionSet::ALL, DimensionSet::singleton));
        variable
    }

    fn root(&self, mut variable: usize) -> usize {
        while self.parent[variable] != variable {
            variable = self.parent[variable];
        }
        variable
    }

    fn unify(&mut self, left: usize, right: usize) {
        let left = self.root(left);
        let right = self.root(right);
        if left == right {
            return;
        }
        let (root, child) = if left < right {
            (left, right)
        } else {
            (right, left)
        };
        let shared = self.domains[root].intersect(self.domains[child]);
        self.parent[child] = root;
        if shared != DimensionSet::EMPTY {
            self.domains[root] = shared;
        }
    }

    fn unify_options(&mut self, left: Option<usize>, right: Option<usize>) {
        if let (Some(left), Some(right)) = (left, right) {
            self.unify(left, right);
        }
    }

    fn constrain(&mut self, variable: usize, dimension: Dimension) {
        self.restrict(variable, DimensionSet::singleton(dimension));
    }

    fn restrict(&mut self, variable: usize, allowed: DimensionSet) {
        let root = self.root(variable);
        let narrowed = self.domains[root].intersect(allowed);
        // Semantic conflict reporting belongs to the evaluator. Keeping the established domain
        // here avoids letting a later default overwrite a valid earlier constraint.
        if narrowed != DimensionSet::EMPTY {
            self.domains[root] = narrowed;
        }
    }

    fn known(&self, variable: usize) -> Option<Dimension> {
        self.domains[self.root(variable)].known()
    }

    fn solve(&mut self) {
        let products = self.products.clone();
        if !self.propagate_products(&products) {
            return;
        }
        let groups = self.product_groups();
        let grouped_roots: BTreeSet<_> = groups
            .iter()
            .flat_map(|group| group.roots.iter().copied())
            .collect();
        for group in &groups {
            self.solve_group(group);
        }
        for variable in self.scalar_defaults.clone() {
            if !grouped_roots.contains(&self.root(variable)) {
                self.constrain(variable, Dimension::Scalar);
            }
        }
    }

    fn solve_group(&mut self, group: &ProductGroup) {
        let original = self.domains.clone();
        if !self.propagate_products(&group.products) || !self.solve_consistently(group) {
            self.domains = original;
        }
    }

    fn solve_consistently(&mut self, group: &ProductGroup) -> bool {
        let groups = self.product_groups_for(&group.products);
        if groups.len() > 1 {
            return groups.iter().all(|group| self.solve_consistently(group));
        }
        let Some(group) = groups.first() else {
            return true;
        };
        let Some(variable) = self.unresolved_variable(&group.roots) else {
            return true;
        };
        let original = self.domains.clone();
        let domain = self.domains[self.root(variable)];
        for dimension in [
            Dimension::Scalar,
            Dimension::Length,
            Dimension::Area,
            Dimension::Angle,
        ] {
            if !domain.contains(dimension) {
                continue;
            }
            self.domains.clone_from(&original);
            self.constrain(variable, dimension);
            if self.propagate_products(&group.products) && self.solve_consistently(group) {
                return true;
            }
        }
        self.domains = original;
        false
    }

    fn unresolved_variable(&self, roots: &BTreeSet<usize>) -> Option<usize> {
        self.scalar_defaults
            .iter()
            .copied()
            .map(|variable| self.root(variable))
            .filter(|root| roots.contains(root))
            .chain(roots.iter().copied())
            .find(|root| self.domains[*root].known().is_none())
    }

    fn propagate_products(&mut self, products: &[Product]) -> bool {
        loop {
            let before = self.domain_state();
            for product in products {
                if !self.solve_product(*product) {
                    return false;
                }
            }
            if self.domain_state() == before {
                return true;
            }
        }
    }

    fn product_groups(&self) -> Vec<ProductGroup> {
        self.product_groups_for(&self.products)
    }

    fn product_groups_for(&self, products: &[Product]) -> Vec<ProductGroup> {
        let mut assigned = vec![false; products.len()];
        let mut groups = Vec::new();
        for seed in 0..products.len() {
            if assigned[seed] {
                continue;
            }
            let roots = self.unresolved_product_roots(products[seed]);
            if roots.is_empty() {
                assigned[seed] = true;
                continue;
            }
            let mut group = ProductGroup {
                products: Vec::new(),
                roots,
            };
            while self.add_touching_products(products, &mut group, &mut assigned) {}
            groups.push(group);
        }
        groups
    }

    fn add_touching_products(
        &self,
        products: &[Product],
        group: &mut ProductGroup,
        assigned: &mut [bool],
    ) -> bool {
        let mut changed = false;
        for (index, product) in products.iter().copied().enumerate() {
            if assigned[index] {
                continue;
            }
            let roots = self.unresolved_product_roots(product);
            if roots.iter().any(|root| group.roots.contains(root)) {
                assigned[index] = true;
                group.roots.extend(roots);
                group.products.push(product);
                changed = true;
            }
        }
        changed
    }

    fn unresolved_product_roots(&self, product: Product) -> BTreeSet<usize> {
        self.product_roots(product)
            .into_iter()
            .filter(|root| self.domains[*root].known().is_none())
            .collect()
    }

    fn product_roots(&self, product: Product) -> [usize; 3] {
        [
            self.root(product.lhs),
            self.root(product.rhs),
            self.root(product.result),
        ]
    }

    fn domain_state(&self) -> (u32, usize) {
        let variables = 0..self.parent.len();
        let width = variables
            .clone()
            .map(|variable| self.domains[self.root(variable)].width())
            .sum();
        let roots = variables
            .filter(|variable| self.root(*variable) == *variable)
            .count();
        (width, roots)
    }

    fn solve_product(&mut self, product: Product) -> bool {
        let variables = [product.lhs, product.rhs, product.result];
        let roots = variables.map(|variable| self.root(variable));
        let domains = roots.map(|root| self.domains[root]);
        let mut allowed = [DimensionSet::EMPTY; 3];
        let mut compatible = false;
        for tuple in product_tuples(product.op) {
            if tuple_is_compatible(*tuple, roots, domains) {
                compatible = true;
                for (index, dimension) in tuple.iter().enumerate() {
                    allowed[index].insert(*dimension);
                }
            }
        }
        if !compatible {
            return false;
        }
        for (variable, allowed) in variables.into_iter().zip(allowed) {
            self.restrict(variable, allowed);
        }
        true
    }

    fn dimension(&self, expr: &Expr) -> Option<Dimension> {
        let key = (expr.span().byte_start, expr.span().byte_end);
        self.exprs
            .get(&key)
            .and_then(|variable| self.known(*variable))
    }

    fn expression_dimensions(&self) -> BTreeMap<ExprKey, Dimension> {
        self.exprs
            .iter()
            .filter_map(|(key, variable)| self.known(*variable).map(|dimension| (*key, dimension)))
            .collect()
    }
}

fn product_tuples(op: ProductOp) -> &'static [[Dimension; 3]] {
    use Dimension::{Angle, Area, Length, Scalar};
    const MUL: &[[Dimension; 3]] = &[
        [Scalar, Scalar, Scalar],
        [Scalar, Length, Length],
        [Length, Scalar, Length],
        [Scalar, Area, Area],
        [Area, Scalar, Area],
        [Scalar, Angle, Angle],
        [Angle, Scalar, Angle],
        [Length, Length, Area],
    ];
    const DIV: &[[Dimension; 3]] = &[
        [Scalar, Scalar, Scalar],
        [Length, Scalar, Length],
        [Area, Scalar, Area],
        [Angle, Scalar, Angle],
        [Length, Length, Scalar],
        [Area, Area, Scalar],
        [Angle, Angle, Scalar],
        [Area, Length, Length],
    ];
    const SQRT: &[[Dimension; 3]] = &[[Scalar, Scalar, Scalar], [Area, Area, Length]];
    match op {
        ProductOp::Mul => MUL,
        ProductOp::Div => DIV,
        ProductOp::Sqrt => SQRT,
    }
}

fn tuple_is_compatible(
    tuple: [Dimension; 3],
    roots: [usize; 3],
    domains: [DimensionSet; 3],
) -> bool {
    for index in 0..3 {
        if !domains[index].contains(tuple[index]) {
            return false;
        }
        for prior in 0..index {
            if roots[index] == roots[prior] && tuple[index] != tuple[prior] {
                return false;
            }
        }
    }
    true
}

fn fixed_call_dimension(name: &str) -> Option<Dimension> {
    match name {
        "dist" | "hdist" | "vdist" | "radius" | "length" | "perimeter" => Some(Dimension::Length),
        "area" => Some(Dimension::Area),
        "angle" | "dir_angle" => Some(Dimension::Angle),
        "ratio" => Some(Dimension::Scalar),
        _ => None,
    }
}

fn prop_dimension(name: &str) -> Option<Dimension> {
    const DIMENSIONS: &[(&str, Dimension)] = &[
        ("gap", Dimension::Length),
        ("size", Dimension::Length),
        ("bend", Dimension::Angle),
        ("n", Dimension::Scalar),
    ];
    DIMENSIONS
        .iter()
        .find_map(|(candidate, dimension)| (*candidate == name).then_some(*dimension))
}

fn call_arg_dimensions(name: &str, args: &[Option<usize>]) -> &'static [(usize, Dimension)] {
    use Dimension::{Angle, Length, Scalar};
    const CALL_ARGS: &[(&str, &[(usize, Dimension)])] = &[
        ("lerp", &[(2, Scalar)]),
        ("offset", &[(1, Length), (2, Length)]),
        ("rotate", &[(2, Angle)]),
        ("on_circle", &[(1, Angle)]),
        ("extend", &[(2, Length)]),
        ("intersect", &[(2, Scalar)]),
        ("tangent_line", &[(2, Scalar)]),
        ("circle", &[(1, Length)]),
        ("angle_arc", &[(3, Length)]),
        ("rect", &[(1, Length), (2, Length)]),
        ("diamond", &[(1, Length), (2, Length)]),
        ("tick", &[(1, Scalar)]),
    ];
    if name == "circle" && args.len() == 1 {
        return &[(0, Length)];
    }
    if name == "rect" || name == "diamond" {
        return if args.len() == 2 {
            &[(0, Length), (1, Length)]
        } else {
            &[(1, Length), (2, Length)]
        };
    }
    if name == "rounded_rect" {
        return if args.len() == 3 {
            &[(0, Length), (1, Length), (2, Length)]
        } else {
            &[(1, Length), (2, Length), (3, Length)]
        };
    }
    if name == "ellipse" {
        return ellipse_arg_dimensions(args);
    }
    CALL_ARGS
        .iter()
        .find_map(|(candidate, dimensions)| (*candidate == name).then_some(*dimensions))
        .unwrap_or(&[])
}

fn ellipse_arg_dimensions(args: &[Option<usize>]) -> &'static [(usize, Dimension)] {
    use Dimension::{Angle, Length};
    match (args.len(), args.first().is_some_and(Option::is_some)) {
        (2, _) => &[(0, Length), (1, Length)],
        (3, true) => &[(0, Length), (1, Length), (2, Angle)],
        (3, false) => &[(1, Length), (2, Length)],
        (4, _) => &[(1, Length), (2, Length), (3, Angle)],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::{Dimension, Dimensions, ExprNode, Hint, NumberNode, to_interchange};
    use crate::parse;

    #[test]
    fn infers_dimensions_through_declarations_and_uses() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nnum r = 2\ncircle c = circle(A, r)\nassert dist(A, B) == r\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        let Some(definition) = &json.ops[2].def else {
            assert!(json.ops[2].def.is_some());
            return;
        };
        assert!(matches!(
            definition.tree,
            ExprNode::Number(super::NumberNode {
                dimension: Dimension::Length,
                ..
            })
        ));
    }

    #[test]
    fn defaults_style_and_unconstrained_numbers() {
        let parsed = parse("locus 0 beta 5\nnum n = 3\n");
        let json = to_interchange(&parsed.file);
        assert_eq!(json.style, "textbook");
        assert!(matches!(
            json.ops[0].def.as_ref().map(|def| &def.tree),
            Some(ExprNode::Number(super::NumberNode {
                dimension: Dimension::Scalar,
                ..
            }))
        ));
    }

    #[test]
    fn infers_scalar_coefficient_of_angle() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint O = point()\npoint B = point()\nassert angle(A, O, B) == 2 * angle(A, O, B)\n",
        );
        let json = to_interchange(&parsed.file);
        let ExprNode::Binary { rhs, .. } = &json.asserts[0].expr else {
            assert!(matches!(json.asserts[0].expr, ExprNode::Binary { .. }));
            return;
        };
        let ExprNode::Binary { lhs, .. } = rhs.as_ref() else {
            assert!(matches!(rhs.as_ref(), ExprNode::Binary { .. }));
            return;
        };
        assert!(matches!(
            lhs.as_ref(),
            ExprNode::Number(super::NumberNode {
                dimension: Dimension::Scalar,
                ..
            })
        ));
    }

    #[test]
    fn resolves_literal_defaults_after_known_product_dimensions() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nnum mul = 2 * dist(A, B)\nnum div = dist(A, B) / 2\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        for definition in json.ops[2..4].iter().filter_map(|op| op.def.as_ref()) {
            let ExprNode::Binary {
                lhs,
                rhs,
                dimension,
                ..
            } = &definition.tree
            else {
                assert!(matches!(definition.tree, ExprNode::Binary { .. }));
                return;
            };
            assert_eq!(*dimension, Some(Dimension::Length));
            assert!(matches!(
                (lhs.as_ref(), rhs.as_ref()),
                (
                    ExprNode::Number(NumberNode {
                        dimension: Dimension::Scalar,
                        ..
                    }),
                    ExprNode::Call {
                        dimension: Some(Dimension::Length),
                        ..
                    }
                ) | (
                    ExprNode::Call {
                        dimension: Some(Dimension::Length),
                        ..
                    },
                    ExprNode::Number(NumberNode {
                        dimension: Dimension::Scalar,
                        ..
                    })
                )
            ));
        }
    }

    #[test]
    fn reverse_infers_area_and_length_operands() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\npoint C = point()\npolygon tri = polygon(A, B, C)\nnum a = free 4\nnum r = a / dist(A, B)\ncircle c = circle(A, r)\nnum side = free 2\nnum product = side * dist(A, B)\nassert product == area(tri)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        let Some(Hint::Scalar(ExprNode::Number(area_hint))) = &json.ops[4].hint else {
            assert!(json.ops[4].hint.is_some());
            return;
        };
        assert_eq!(area_hint.dimension, Dimension::Area);
        let Some(Hint::Scalar(ExprNode::Number(side_hint))) = &json.ops[7].hint else {
            assert!(json.ops[7].hint.is_some());
            return;
        };
        assert_eq!(side_hint.dimension, Dimension::Length);
        assert!(matches!(
            json.ops[5].def.as_ref().map(|definition| &definition.tree),
            Some(ExprNode::Binary {
                dimension: Some(Dimension::Length),
                ..
            })
        ));
        assert!(matches!(
            json.ops[8].def.as_ref().map(|definition| &definition.tree),
            Some(ExprNode::Binary {
                dimension: Some(Dimension::Area),
                ..
            })
        ));
    }

    #[test]
    fn leaves_sqrt_pending_until_its_input_dimension_is_known() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\npoint C = point()\npolygon tri = polygon(A, B, C)\nnum n = sqrt(2 * area(tri))\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        let Some(ExprNode::Call {
            args, dimension, ..
        }) = json.ops[4].def.as_ref().map(|definition| &definition.tree)
        else {
            assert!(matches!(
                json.ops[4].def.as_ref().map(|definition| &definition.tree),
                Some(ExprNode::Call { .. })
            ));
            return;
        };
        assert_eq!(*dimension, Some(Dimension::Length));
        assert!(matches!(
            args.as_slice(),
            [ExprNode::Binary {
                lhs,
                rhs,
                dimension: Some(Dimension::Area),
                ..
            }] if matches!(lhs.as_ref(), ExprNode::Number(NumberNode {
                dimension: Dimension::Scalar,
                ..
            })) && matches!(rhs.as_ref(), ExprNode::Call {
                dimension: Some(Dimension::Area),
                ..
            })
        ));
    }

    #[test]
    fn infers_shared_multiplication_operands_from_an_area_result() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\npoint C = point()\npolygon tri = polygon(A, B, C)\nnum r = free 2\nnum a = r * r\nassert a == area(tri)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        assert!(matches!(
            &json.ops[4].hint,
            Some(Hint::Scalar(ExprNode::Number(NumberNode {
                dimension: Dimension::Length,
                ..
            })))
        ));
        assert!(matches!(
            json.ops[5].def.as_ref().map(|definition| &definition.tree),
            Some(ExprNode::Binary {
                lhs,
                rhs,
                dimension: Some(Dimension::Area),
                ..
            }) if matches!(lhs.as_ref(), ExprNode::Ref {
                dimension: Some(Dimension::Length),
                ..
            }) && matches!(rhs.as_ref(), ExprNode::Ref {
                dimension: Some(Dimension::Length),
                ..
            })
        ));
        assert!(matches!(
            &json.asserts[0].expr,
            ExprNode::Binary { lhs, rhs, .. }
                if matches!(lhs.as_ref(), ExprNode::Ref {
                    dimension: Some(Dimension::Area),
                    ..
                }) && matches!(rhs.as_ref(), ExprNode::Call {
                    dimension: Some(Dimension::Area),
                    ..
                })
        ));
    }

    #[test]
    fn shape_dimensions_constrain_numeric_declarations_to_length() {
        let parsed = parse("locus 0 beta 5\nnum x = free 1\ncircle c = circle(x)\n");
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        assert!(matches!(
            &json.ops[0].hint,
            Some(Hint::Scalar(ExprNode::Number(NumberNode {
                dimension: Dimension::Length,
                ..
            })))
        ));
        assert!(matches!(
            json.ops[1].def.as_ref().map(|definition| &definition.tree),
            Some(ExprNode::Call { args, .. })
                if matches!(args.first(), Some(ExprNode::Ref {
                    dimension: Some(Dimension::Length),
                    ..
                }))
        ));
    }

    #[test]
    fn resolves_constraints_across_sqrt_and_division_before_defaults() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\nnum a = free 4\nnum n = sqrt(a)\ncircle c = circle(A, n / 2)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        assert!(matches!(
            &json.ops[1].hint,
            Some(Hint::Scalar(ExprNode::Number(NumberNode {
                dimension: Dimension::Area,
                ..
            })))
        ));
        assert!(matches!(
            json.ops[2].def.as_ref().map(|definition| &definition.tree),
            Some(ExprNode::Call {
                args,
                dimension: Some(Dimension::Length),
                ..
            }) if matches!(args.as_slice(), [ExprNode::Ref {
                dimension: Some(Dimension::Area),
                ..
            }])
        ));
        let Some(ExprNode::Call { args, .. }) =
            json.ops[3].def.as_ref().map(|definition| &definition.tree)
        else {
            assert!(matches!(
                json.ops[3].def.as_ref().map(|definition| &definition.tree),
                Some(ExprNode::Call { .. })
            ));
            return;
        };
        assert!(matches!(
            args.get(1),
            Some(ExprNode::Binary {
                lhs,
                rhs,
                dimension: Some(Dimension::Length),
                ..
            }) if matches!(lhs.as_ref(), ExprNode::Ref {
                dimension: Some(Dimension::Length),
                ..
            }) && matches!(rhs.as_ref(), ExprNode::Number(NumberNode {
                dimension: Dimension::Scalar,
                ..
            }))
        ));
    }

    #[test]
    fn scalar_division_carries_equal_operand_constraints() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\npoint C = point()\npolygon tri = polygon(A, B, C)\nnum r = free 2\nnum s = free 3\nnum a = r * s\nassert a == area(tri)\ntick t = tick(seg(A, B), r / s)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        for op in &json.ops[4..6] {
            assert!(matches!(
                &op.hint,
                Some(Hint::Scalar(ExprNode::Number(NumberNode {
                    dimension: Dimension::Length,
                    ..
                })))
            ));
        }
        assert!(matches!(
            json.ops[6].def.as_ref().map(|definition| &definition.tree),
            Some(ExprNode::Binary {
                dimension: Some(Dimension::Area),
                ..
            })
        ));
        let Some(ExprNode::Call { args, .. }) =
            json.ops[7].def.as_ref().map(|definition| &definition.tree)
        else {
            assert!(matches!(
                json.ops[7].def.as_ref().map(|definition| &definition.tree),
                Some(ExprNode::Call { .. })
            ));
            return;
        };
        assert!(matches!(
            args.get(1),
            Some(ExprNode::Binary {
                lhs,
                rhs,
                dimension: Some(Dimension::Scalar),
                ..
            }) if matches!(lhs.as_ref(), ExprNode::Ref {
                dimension: Some(Dimension::Length),
                ..
            }) && matches!(rhs.as_ref(), ExprNode::Ref {
                dimension: Some(Dimension::Length),
                ..
            })
        ));
    }

    #[test]
    fn backtracks_scalar_defaults_across_cyclic_product_constraints() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\npoint C = point()\npolygon tri = polygon(A, B, C)\nnum r = free 2\nnum s = free 3\nnum t = free 4\nrel r * s == area(tri)\nrel s * t == area(tri)\nrel r * t == area(tri)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let json = to_interchange(&parsed.file);
        for op in &json.ops[4..7] {
            assert!(matches!(
                &op.hint,
                Some(Hint::Scalar(ExprNode::Number(NumberNode {
                    dimension: Dimension::Length,
                    ..
                })))
            ));
        }
        for relation in &json.rels {
            assert!(matches!(
                relation,
                ExprNode::Binary { lhs, rhs, .. }
                    if matches!(lhs.as_ref(), ExprNode::Binary {
                        lhs: factor_a,
                        rhs: factor_b,
                        dimension: Some(Dimension::Area),
                        ..
                    } if matches!(factor_a.as_ref(), ExprNode::Ref {
                        dimension: Some(Dimension::Length),
                        ..
                    }) && matches!(factor_b.as_ref(), ExprNode::Ref {
                        dimension: Some(Dimension::Length),
                        ..
                    })) && matches!(rhs.as_ref(), ExprNode::Call {
                        dimension: Some(Dimension::Area),
                        ..
                    })
            ));
        }
    }

    #[test]
    fn excludes_unrelated_declarations_from_failed_product_search() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nnum n0 = 1\nnum n1 = 1\nnum n2 = 1\nnum n3 = 1\nnum n4 = 1\nnum n5 = 1\nnum n6 = 1\nnum n7 = 1\nnum n8 = 1\nnum n9 = 1\nnum n10 = 1\nnum n11 = 1\nnum r = free 2\nnum s = free 3\nnum t = free 4\nrel r * s == dist(A, B)\nrel s * t == dist(A, B)\nrel r * t == dist(A, B)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let dimensions = Dimensions::infer(&parsed.file);
        let groups = dimensions.product_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].products.len(), 3);
        for index in 0..12 {
            let variable = dimensions.nums[&format!("n{index}")];
            assert!(!groups[0].roots.contains(&dimensions.root(variable)));
            assert_eq!(dimensions.known(variable), Some(Dimension::Scalar));
        }
    }

    #[test]
    fn fixed_dimensions_do_not_connect_independent_product_groups() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nnum k = 1\ntick mark = tick(seg(A, B), k)\nnum n0 = 1\nnum p0 = n0 * k\nnum n1 = 1\nnum p1 = n1 * k\nnum n2 = 1\nnum p2 = n2 * k\nnum n3 = 1\nnum p3 = n3 * k\nnum n4 = 1\nnum p4 = n4 * k\nnum n5 = 1\nnum p5 = n5 * k\nnum n6 = 1\nnum p6 = n6 * k\nnum n7 = 1\nnum p7 = n7 * k\nnum n8 = 1\nnum p8 = n8 * k\nnum n9 = 1\nnum p9 = n9 * k\nnum r = free 2\nnum s = free 3\nnum t = free 4\nrel (r * k) * s == dist(A, B)\nrel s * t == dist(A, B)\nrel r * t == dist(A, B)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let dimensions = Dimensions::infer(&parsed.file);
        let groups = dimensions.product_groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].products.len(), 4);
        let fixed = dimensions.nums["k"];
        assert_eq!(dimensions.known(fixed), Some(Dimension::Scalar));
        assert!(!groups[0].roots.contains(&dimensions.root(fixed)));
        for index in 0..10 {
            for prefix in ["n", "p"] {
                let variable = dimensions.nums[&format!("{prefix}{index}")];
                assert!(!groups[0].roots.contains(&dimensions.root(variable)));
                assert_eq!(dimensions.known(variable), Some(Dimension::Scalar));
            }
        }
    }

    #[test]
    fn repartitions_product_groups_after_a_branch_fixes_a_dimension() {
        let parsed = parse(
            "locus 0 beta 5\npoint A = point()\npoint B = point()\nnum k = 1\nnum n0 = 1\nnum p0 = n0 * k\nnum n1 = 1\nnum p1 = n1 * k\nnum n2 = 1\nnum p2 = n2 * k\nnum n3 = 1\nnum p3 = n3 * k\nnum n4 = 1\nnum p4 = n4 * k\nnum n5 = 1\nnum p5 = n5 * k\nnum n6 = 1\nnum p6 = n6 * k\nnum n7 = 1\nnum p7 = n7 * k\nnum n8 = 1\nnum p8 = n8 * k\nnum n9 = 1\nnum p9 = n9 * k\nnum r = free 2\nnum s = free 3\nnum t = free 4\nrel (r * k) * s == dist(A, B)\nrel s * t == dist(A, B)\nrel r * t == dist(A, B)\n",
        );
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let dimensions = Dimensions::infer(&parsed.file);
        for (name, expected) in [
            ("k", Dimension::Length),
            ("r", Dimension::Scalar),
            ("s", Dimension::Scalar),
            ("t", Dimension::Length),
        ] {
            assert_eq!(dimensions.known(dimensions.nums[name]), Some(expected));
        }
        for index in 0..10 {
            let input = dimensions.nums[&format!("n{index}")];
            let product = dimensions.nums[&format!("p{index}")];
            assert_eq!(dimensions.known(input), Some(Dimension::Scalar));
            assert_eq!(dimensions.known(product), Some(Dimension::Length));
        }
    }
}
