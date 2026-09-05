//! Hover documentation for identifiers and constructors.

use std::fmt::Write as _;

use locus_core::{Dimension, ResolvedObject};
use locus_syntax::CONSTRUCTOR_SIGNATURES;
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind};

use super::target::{Target, at, class_definition, object_definition};
use crate::state::{State, resolved_scene};

pub(crate) fn handle(state: &State, params: &HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let document = state.document(uri)?;
    let analysis = state.analysis(uri)?;
    let byte = document.position_to_byte(params.text_document_position_params.position)?;
    let target = at(&analysis.file, byte)?;
    let (contents, span) = match target {
        Target::Object(id) => {
            let op = object_definition(&analysis.file, &id.name)?;
            (object_hover(document, analysis, op), id.span)
        }
        Target::Class(id) => {
            let class = class_definition(&analysis.file, &id.name)?;
            let source = document.source_for_span(class.span).trim();
            (
                format!("`class {}`\n\n```locus\n{source}\n```", id.name),
                id.span,
            )
        }
        Target::Callable(id) => {
            let signature = CONSTRUCTOR_SIGNATURES
                .iter()
                .find_map(|(name, signature)| (*name == id.name).then_some(*signature))?;
            (format!("`{signature}`"), id.span)
        }
    };
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: contents,
        }),
        range: Some(document.span_to_range(span)),
    })
}

fn object_hover(
    document: &crate::document::Document,
    analysis: &crate::state::Analysis,
    op: &locus_syntax::OpStmt,
) -> String {
    let mut text = format!("`{}` **{}**", op.kind.as_str(), op.id.name);
    let definition = document.source_for_span(op.span).trim();
    if !definition.is_empty() {
        let _ = write!(text, "\n\n```locus\n{definition}\n```");
    }
    if let Some(evaluation) = &analysis.evaluation {
        let scene = resolved_scene(evaluation);
        if let Some((_, object)) = scene.objects.iter().find(|(id, _)| id.name == op.id.name) {
            let _ = write!(text, "\n\nResolved: `{}`", resolved_value(object));
        }
    }
    text
}

fn resolved_value(object: &ResolvedObject) -> String {
    match object {
        ResolvedObject::Num(_) | ResolvedObject::Angle(_) | ResolvedObject::Point(_) => {
            scalar_value(object)
        }
        ResolvedObject::Line(_) | ResolvedObject::Ray(_) | ResolvedObject::Segment(_) => {
            linear_value(object)
        }
        ResolvedObject::Circle(_) | ResolvedObject::Arc(_) => curved_value(object),
        ResolvedObject::Polygon(value)
        | ResolvedObject::Rect(value)
        | ResolvedObject::Diamond(value) => format!(
            "area {}, perimeter {}",
            locus_core::geom::measure::area(value),
            locus_core::geom::measure::perimeter(value)
        ),
        ResolvedObject::Annotation(_) => "annotation".to_owned(),
    }
}

fn scalar_value(object: &ResolvedObject) -> String {
    match object {
        ResolvedObject::Num(value) => {
            format!("{} {}", value.value, dimension_name(value.dimension))
        }
        ResolvedObject::Angle(value) => format!("{value} rad"),
        ResolvedObject::Point(value) => {
            format!("({}, {})", value.geometry.x, value.geometry.y)
        }
        _ => String::new(),
    }
}

fn linear_value(object: &ResolvedObject) -> String {
    match object {
        ResolvedObject::Line(value) => format!(
            "line through ({}, {}) and ({}, {})",
            value.start.x, value.start.y, value.through.x, value.through.y
        ),
        ResolvedObject::Ray(value) => format!(
            "ray from ({}, {}) through ({}, {})",
            value.start.x, value.start.y, value.through.x, value.through.y
        ),
        ResolvedObject::Segment(value) => {
            format!("length {}", locus_core::geom::measure::length(*value))
        }
        _ => String::new(),
    }
}

fn curved_value(object: &ResolvedObject) -> String {
    match object {
        ResolvedObject::Circle(value) => format!(
            "center ({}, {}), radius {}",
            value.center.x, value.center.y, value.radius
        ),
        ResolvedObject::Arc(value) => format!("radius {}", value.geometry.circle.radius),
        _ => String::new(),
    }
}

const fn dimension_name(dimension: Dimension) -> &'static str {
    match dimension {
        Dimension::Scalar => "scalar",
        Dimension::Length => "length",
        Dimension::Area => "area",
    }
}
