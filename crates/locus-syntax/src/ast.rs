//! Serializable source AST for Locus 0 beta 5.
#![allow(
    missing_docs,
    reason = "AST fields mirror the normative grammar directly"
)]

use crate::Span;
use serde::Serialize;

/// A parsed Locus source file.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct File {
    /// Statements in source order.
    pub statements: Vec<Statement>,
    /// Full source span.
    pub span: Span,
}

/// A top-level statement.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum Statement {
    /// Object or annotation declaration.
    Op(OpStmt),
    /// Solver relation.
    Rel(RelStmt),
    /// Post-solve assertion.
    Assert(AssertStmt),
    /// One metadata statement.
    Meta(MetaEntry),
    /// Rendering style selection.
    Style(StyleStmt),
    /// User-defined style class.
    Class(ClassStmt),
}

impl Statement {
    /// Returns this statement's source span.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Op(value) => value.span,
            Self::Rel(value) => value.span,
            Self::Assert(value) => value.span,
            Self::Meta(value) => value.span,
            Self::Style(value) => value.span,
            Self::Class(value) => value.span,
        }
    }
}

/// An object kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Point,
    Num,
    Line,
    Ray,
    Segment,
    Circle,
    Arc,
    Ellipse,
    Bspline,
    Polygon,
    Rect,
    RoundedRect,
    Diamond,
    Label,
    Arrow,
    Tick,
    RightAngle,
    TextBlock,
}

impl Kind {
    const ALL: [Self; 18] = [
        Self::Point,
        Self::Num,
        Self::Line,
        Self::Ray,
        Self::Segment,
        Self::Circle,
        Self::Arc,
        Self::Ellipse,
        Self::Bspline,
        Self::Polygon,
        Self::Rect,
        Self::RoundedRect,
        Self::Diamond,
        Self::Label,
        Self::Arrow,
        Self::Tick,
        Self::RightAngle,
        Self::TextBlock,
    ];

    /// Parses a kind keyword.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        crate::vocabulary::KIND_NAMES
            .iter()
            .position(|candidate| *candidate == name)
            .and_then(|index| Self::ALL.get(index).copied())
    }

    /// Returns the source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        crate::vocabulary::KIND_NAMES[self as usize]
    }
}

/// An identifier plus its source span.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

/// An object declaration.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct OpStmt {
    pub kind: Kind,
    pub id: Ident,
    pub ctor: Constructor,
    pub where_clauses: Vec<Expr>,
    pub attr: Option<StyleAttr>,
    pub props: Props,
    pub span: Span,
}

/// The right-hand side of an object declaration.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Constructor {
    /// A named derived constructor.
    Call {
        name: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    /// A derived numeric expression.
    Expr { expr: Box<Expr>, span: Span },
    /// A free numeric value and its mandatory scalar hint.
    FreeNum { hint: Box<Expr>, span: Span },
}

impl Constructor {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Call { span, .. } | Self::Expr { span, .. } | Self::FreeNum { span, .. } => *span,
        }
    }
}

/// A solver relation statement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RelStmt {
    pub expr: Expr,
    pub span: Span,
}

/// A post-solve assertion statement.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AssertStmt {
    pub expr: Expr,
    pub tolerance: Option<NumberExpr>,
    pub span: Span,
}

/// A style selection statement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StyleStmt {
    pub name: Ident,
    pub span: Span,
}

/// A style class declaration.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ClassStmt {
    pub name: Ident,
    pub props: Props,
    pub span: Span,
}

/// One line of metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MetaEntry {
    pub key: Ident,
    pub value: TextValue,
    pub span: Span,
}

/// A style attribute attached to an operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StyleAttr {
    pub name: Ident,
    pub span: Span,
}

/// A property block.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Props {
    pub items: Vec<Prop>,
    pub span: Span,
}

/// One named property.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Prop {
    pub name: Ident,
    pub value: PropValue,
    pub span: Span,
}

/// A property value.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PropValue {
    Expr(Expr),
    Text(TextValue),
    Head(HeadToken),
    HexColor(HexColor),
}

impl PropValue {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Expr(value) => value.span(),
            Self::Text(value) => value.span,
            Self::Head(value) => value.span,
            Self::HexColor(value) => value.span,
        }
    }
}

/// An arrow-head token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct HeadToken {
    pub kind: HeadKind,
    pub span: Span,
}

/// Arrow-head choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadKind {
    Forward,
    Both,
    None,
}

/// A validated six- or eight-digit source color.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HexColor {
    pub value: String,
    pub span: Span,
}

/// An expression.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Expr {
    Number(NumberExpr),
    Boolean {
        value: bool,
        span: Span,
    },
    String(TextValue),
    Tex(TextValue),
    Ref(Ident),
    Call {
        name: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    Property {
        base: Box<Expr>,
        name: Ident,
        args: Vec<Expr>,
        span: Span,
    },
    Group {
        expr: Box<Expr>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    /// Returns the expression's full source span.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Number(value) => value.span,
            Self::Boolean { span, .. }
            | Self::Call { span, .. }
            | Self::Property { span, .. }
            | Self::Group { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. } => *span,
            Self::String(value) | Self::Tex(value) => value.span,
            Self::Ref(value) => value.span,
        }
    }
}

/// A number literal with an optional mandatory-at-use-site angle unit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NumberExpr {
    pub value: f64,
    pub raw: String,
    pub unit: Option<AngleUnit>,
    pub span: Span,
}

/// Accepted angle units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AngleUnit {
    Deg,
    Rad,
}

/// Unary expression operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOp {
    Neg,
    Not,
}

/// Binary expression operators, ordered by grammar precedence in the parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

/// A segmented quoted string or bare TeX literal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TextValue {
    pub segments: Vec<TextSegment>,
    pub span: Span,
}

/// One plain-text or TeX segment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TextSegment {
    pub kind: TextSegmentKind,
    pub source: String,
    pub span: Span,
}

/// Text segment representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextSegmentKind {
    Plain,
    Tex,
}
