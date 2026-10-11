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

/// Source and expanded Locus JSON interchange document.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Interchange {
    pub version: String,
    pub meta: Vec<MetaEntry>,
    pub style: String,
    pub classes: Vec<Class>,
    pub ops: Vec<Op>,
    pub rels: Vec<ExprNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub relation_properties: Vec<RelationProperties>,
    pub asserts: Vec<Assert>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<crate::ast::FnStmt>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub calls: Vec<crate::ast::CallStmt>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<crate::ast::GroupStmt>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub collections: BTreeMap<String, crate::Collection>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub expansion_origins: Vec<crate::ExpansionOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<Box<Interchange>>,
}

/// Properties attached to one relation, indexed in the `rels` array.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RelationProperties {
    pub relation_index: usize,
    pub props: Vec<Property>,
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
        relation_properties: Vec::new(),
        asserts: Vec::new(),
        functions: Vec::new(),
        calls: Vec::new(),
        groups: Vec::new(),
        collections: BTreeMap::new(),
        expansion_origins: Vec::new(),
        expanded: None,
    };
    for statement in &file.statements {
        push_statement(&mut output, statement, &dimensions);
    }
    if file.statements.iter().any(|statement| {
        matches!(
            statement,
            Statement::Fn(_) | Statement::Group(_) | Statement::Call(_)
        )
    }) {
        let expanded = crate::expand(file);
        output.collections = expanded.collections;
        output.expansion_origins = expanded.origins;
        output.expanded = Some(Box::new(to_interchange(&expanded.file)));
    }
    output
}

/// Returns inferred numeric dimensions keyed by each expression's UTF-8 byte range.
/// Ambiguous ranges shared by expanded instances with different dimensions are omitted.
#[must_use]
pub fn inferred_expression_dimensions(file: &File) -> BTreeMap<(usize, usize), Dimension> {
    Dimensions::infer(file).expression_dimensions()
}

/// Returns dimensions keyed by the identity of each expression in this AST.
///
/// Keys use `std::ptr::from_ref(expr) as usize` and remain valid while the AST is unmoved.
/// Unlike source spans, these keys distinguish repeated function-body expressions.
#[must_use]
pub fn inferred_expression_dimensions_by_node(file: &File) -> BTreeMap<usize, Dimension> {
    Dimensions::infer(file).node_dimensions()
}

/// Returns a source-span dimension map for each statement in declaration order.
#[must_use]
pub fn inferred_statement_expression_dimensions(
    file: &File,
) -> Vec<BTreeMap<(usize, usize), Dimension>> {
    Dimensions::infer(file).statement_dimensions(file.statements.len())
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
    if let Statement::Fn(function) = statement {
        output.functions.push(function.clone());
        return;
    }
    if let Statement::Call(call) = statement {
        output.calls.push(call.clone());
        return;
    }
    if let Statement::Group(group) = statement {
        output.groups.push(group.clone());
        return;
    }
    if let Statement::Class(class) = statement {
        output.classes.push(Class {
            name: class.name.name.clone(),
            props: properties(&class.props, dimensions),
        });
        return;
    }
    if let Statement::Style(style) = statement {
        output.style.clone_from(&style.name.name);
        return;
    }
    if let Statement::Meta(entry) = statement {
        output.meta.push(MetaEntry {
            key: entry.key.name.clone(),
            value: text_segments(&entry.value),
        });
        return;
    }
    match statement {
        Statement::Op(op) => output.ops.push(op_node(op, dimensions)),
        Statement::Rel(rel) => push_relation(output, rel, dimensions),
        Statement::Assert(assertion) => output.asserts.push(Assert {
            expr: expr_node(&assertion.expr, dimensions),
            tolerance: assertion
                .tolerance
                .as_ref()
                .map(|number| number_node(number, Dimension::Scalar)),
        }),
        _ => unreachable!(),
    }
}

fn push_relation(output: &mut Interchange, rel: &crate::ast::RelStmt, dimensions: &Dimensions) {
    if !rel.props.items.is_empty() {
        output.relation_properties.push(RelationProperties {
            relation_index: output.rels.len(),
            props: properties(&rel.props, dimensions),
        });
    }
    output.rels.push(expr_node(&rel.expr, dimensions));
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
    exprs: BTreeMap<usize, usize>,
    source_keys: BTreeMap<usize, ExprKey>,
    statement_keys: BTreeMap<usize, usize>,
    statement_index: usize,
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
            source_keys: BTreeMap::new(),
            statement_keys: BTreeMap::new(),
            statement_index: 0,
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
        for (index, statement) in file.statements.iter().enumerate() {
            self.statement_index = index;
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
            Statement::Meta(_)
            | Statement::Style(_)
            | Statement::Fn(_)
            | Statement::Group(_)
            | Statement::Call(_) => {}
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
        matches!(name, "width" | "height" | "length" | "corner_radius")
            .then(|| self.expr_var(expr, Some(Dimension::Length)))
    }

    fn expect_expr(&mut self, expr: &Expr, dimension: Dimension) {
        if let Some(variable) = self.visit_expr(expr) {
            self.constrain(variable, dimension);
        }
    }

    fn expr_var(&mut self, expr: &Expr, fixed: Option<Dimension>) -> usize {
        let key = std::ptr::from_ref(expr) as usize;
        self.source_keys
            .insert(key, (expr.span().byte_start, expr.span().byte_end));
        self.statement_keys.insert(key, self.statement_index);
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
        let key = std::ptr::from_ref(expr) as usize;
        self.exprs
            .get(&key)
            .and_then(|variable| self.known(*variable))
    }

    fn node_dimensions(&self) -> BTreeMap<usize, Dimension> {
        self.exprs
            .iter()
            .filter_map(|(key, variable)| self.known(*variable).map(|dimension| (*key, dimension)))
            .collect()
    }

    fn expression_dimensions(&self) -> BTreeMap<ExprKey, Dimension> {
        let mut dimensions = BTreeMap::new();
        let mut ambiguous = BTreeSet::new();
        for (key, dimension) in self.node_dimensions() {
            let source_key = self.source_keys[&key];
            if dimensions
                .insert(source_key, dimension)
                .is_some_and(|previous| previous != dimension)
            {
                ambiguous.insert(source_key);
            }
        }
        for key in ambiguous {
            dimensions.remove(&key);
        }
        dimensions
    }

    fn statement_dimensions(&self, count: usize) -> Vec<BTreeMap<ExprKey, Dimension>> {
        let mut dimensions = vec![BTreeMap::new(); count];
        for (key, dimension) in self.node_dimensions() {
            dimensions[self.statement_keys[&key]].insert(self.source_keys[&key], dimension);
        }
        dimensions
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
        "style_metric" | "dist" | "hdist" | "vdist" | "radius" | "length" | "perimeter" => {
            Some(Dimension::Length)
        }
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
