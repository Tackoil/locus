//! Source bindings for function scopes and qualified instance members.

use locus_syntax::{CallStmt, Expr, File, GroupStmt, Ident, OpStmt, Span, Statement};

#[derive(Clone, Copy)]
pub(super) enum Binding<'a> {
    Object(&'a OpStmt),
    Group(&'a GroupStmt),
    Instance(&'a CallStmt),
    Parameter(&'a Ident),
}

impl<'a> Binding<'a> {
    pub(super) fn id(self) -> &'a Ident {
        match self {
            Self::Object(value) => &value.id,
            Self::Group(value) => &value.name,
            Self::Instance(value) => &value.id,
            Self::Parameter(value) => value,
        }
    }

    pub(super) fn detail(self) -> &'a str {
        match self {
            Self::Object(value) => value.kind.as_str(),
            Self::Group(_) => "group",
            Self::Instance(value) => &value.function.name,
            Self::Parameter(_) => "parameter",
        }
    }

    pub(super) fn span(self) -> Span {
        match self {
            Self::Object(value) => value.span,
            Self::Group(value) => value.span,
            Self::Instance(value) => value.span,
            Self::Parameter(value) => value.span,
        }
    }
}

fn binding(statement: &Statement) -> Option<Binding<'_>> {
    match statement {
        Statement::Op(value) => Some(Binding::Object(value)),
        Statement::Group(value) => Some(Binding::Group(value)),
        Statement::Call(value) => Some(Binding::Instance(value)),
        _ => None,
    }
}

pub(super) fn visible(file: &File, byte: usize) -> Vec<Binding<'_>> {
    let function = file
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Fn(value) if value.span.byte_start <= byte && byte < value.span.byte_end => {
                Some(value)
            }
            _ => None,
        });
    let cutoff = function.map_or(byte, |value| value.span.byte_start);
    let mut values = Vec::new();
    if let Some(function) = function {
        values.extend(function.params.iter().map(Binding::Parameter));
        values.extend(
            function
                .body
                .iter()
                .filter_map(binding)
                .filter(|value| available(*value, byte)),
        );
    }
    values.extend(
        file.statements
            .iter()
            .filter_map(binding)
            .filter(|value| available(*value, cutoff)),
    );
    values
}

fn available(value: Binding<'_>, byte: usize) -> bool {
    if value.span().byte_end <= byte {
        return true;
    }
    let Binding::Object(op) = value else {
        return false;
    };
    op.where_clauses
        .iter()
        .any(|clause| clause.span().byte_start <= byte && byte <= clause.span().byte_end)
}

pub(super) fn resolve<'a>(file: &'a File, name: &str, byte: usize) -> Option<Binding<'a>> {
    let mut parts = name.split('.');
    let root = parts.next()?;
    let mut value = declaration_at(file, root, byte).or_else(|| {
        visible(file, byte)
            .into_iter()
            .find(|value| value.id().name == root)
    })?;
    for part in parts {
        value = members(file, value)
            .into_iter()
            .find(|value| value.id().name == part)?;
    }
    Some(value)
}

fn declaration_at<'a>(file: &'a File, name: &str, byte: usize) -> Option<Binding<'a>> {
    file.statements.iter().find_map(|statement| {
        if let Statement::Fn(function) = statement {
            return function
                .body
                .iter()
                .filter_map(binding)
                .find(|value| is_declaration(*value, name, byte));
        }
        binding(statement).filter(|value| is_declaration(*value, name, byte))
    })
}

fn is_declaration(value: Binding<'_>, name: &str, byte: usize) -> bool {
    let id = value.id();
    id.name == name && id.span.byte_start <= byte && byte < id.span.byte_end
}

pub(super) fn members<'a>(file: &'a File, value: Binding<'a>) -> Vec<Binding<'a>> {
    let Binding::Instance(call) = value else {
        return Vec::new();
    };
    file.statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Fn(function)
                if function.name.name == call.function.name
                    && function.span.byte_end <= call.span.byte_start =>
            {
                Some(function.body.iter().filter_map(binding).collect())
            }
            _ => None,
        })
        .unwrap_or_default()
}

pub(super) fn qualified_name(expression: &Expr) -> Option<String> {
    match expression {
        Expr::Ref(id) => Some(id.name.clone()),
        Expr::Property {
            base, name, args, ..
        } if args.is_empty() => Some(format!("{}.{}", qualified_name(base)?, name.name)),
        _ => None,
    }
}
