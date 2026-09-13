//! Converts locus-tex's measured SVG outlines once for both output backends.

use crate::{
    RenderError,
    primitives::{Shape, path},
    viewport::Viewport,
};
use locus_core::Point;
use locus_layout::LabelBox;
use locus_syntax::Span;
mod path;
mod transform;
use path::parse_path;
use transform::{Matrix, map, multiply, transforms};

pub(crate) fn outlines(
    label: &LabelBox,
    view: Viewport,
    span: Span,
) -> Result<Vec<Shape>, RenderError> {
    if !label.baseline.is_finite() || !label.em_size.is_finite() || label.em_size <= 0.0 {
        return Err(RenderError::invalid(
            span,
            "invalid measured text placement",
        ));
    }
    // svg_body starts at its visual left, not its logical baseline_x.
    let origin = view.to_svg(Point::new(
        label.baseline.x - label.measured.baseline_x * label.em_size,
        label.baseline.y,
    ));
    let scale = label.em_size * view.pixels_per_unit;
    let transform = [scale, 0.0, 0.0, scale, origin.x, origin.y];
    let xml = format!("<svg>{}</svg>", label.measured.svg_body);
    let document =
        roxmltree::Document::parse(&xml).map_err(|e| RenderError::invalid(span, e.to_string()))?;
    let mut shapes = Vec::new();
    visit(document.root_element(), transform, span, &mut shapes)?;
    Ok(shapes)
}

fn visit(
    node: roxmltree::Node<'_, '_>,
    parent: Matrix,
    span: Span,
    shapes: &mut Vec<Shape>,
) -> Result<(), RenderError> {
    let transform = multiply(
        parent,
        transforms(node.attribute("transform").unwrap_or(""), span)?,
    );
    match node.tag_name().name() {
        "svg" | "g" => {
            for child in node.children().filter(roxmltree::Node::is_element) {
                visit(child, transform, span, shapes)?;
            }
        }
        "path" => shapes.push(parse_path(
            node.attribute("d").unwrap_or(""),
            transform,
            span,
        )?),
        "rect" => shapes.push(rectangle(node, transform, span)?),
        "title" | "desc" => {}
        name => {
            return Err(RenderError::invalid(
                span,
                format!("unsupported measured SVG element: {name}; supply path outlines"),
            ));
        }
    }
    Ok(())
}

fn rectangle(
    node: roxmltree::Node<'_, '_>,
    transform: Matrix,
    span: Span,
) -> Result<Shape, RenderError> {
    let number = |key| {
        node.attribute(key)
            .unwrap_or("0")
            .parse::<f64>()
            .map_err(|e| RenderError::invalid(span, e.to_string()))
    };
    let (x, y, w, h) = (
        number("x")?,
        number("y")?,
        number("width")?,
        number("height")?,
    );
    Ok(path(
        [
            Point::new(x, y),
            Point::new(x + w, y),
            Point::new(x + w, y + h),
            Point::new(x, y + h),
        ]
        .map(|p| map(transform, p)),
        true,
        true,
    ))
}
