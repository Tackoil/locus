//! `TikZ` output from the identical physical display list used by SVG.

use crate::{
    RenderError, RenderOptions,
    primitives::{self, Command, Shape},
    style::{Color, Style, StyleSheet},
    svg::format_float as number,
};
use locus_core::{Point, ResolvedScene};
use locus_layout::LayoutScene;
use std::fmt::Write as _;

/// Emits a `tikzpicture`; include `\usepackage{tikz}` in the enclosing document.
/// All coordinates and glyph outlines come from the resolved layout. One CSS
/// pixel is converted to 0.75pt, including numeric widths and dash lengths.
///
/// # Errors
/// Returns the same source-aware validation failures as [`crate::svg::render`].
pub fn render(
    scene: &ResolvedScene,
    layout: &LayoutScene,
    styles: &StyleSheet,
    options: RenderOptions,
) -> Result<String, RenderError> {
    let drawing = primitives::prepare(scene, layout, styles, options)?;
    let width = number(drawing.viewport.width());
    let height = number(drawing.viewport.height());
    let mut result = format!(
        "\\begin{{tikzpicture}}[x=0.75pt,y=-0.75pt]\n  \\path[use as bounding box] (0,0) rectangle ({width},{height});\n  \\clip (0,0) rectangle ({width},{height});\n"
    );
    if let Some(background) = drawing.background {
        let _ = writeln!(
            result,
            "  \\path[fill={},draw=none] (0,0) rectangle ({width},{height});",
            color(background)
        );
    }
    for op in &drawing.ops {
        let _ = writeln!(
            result,
            "  % source {}..{}",
            op.id.span.byte_start, op.id.span.byte_end
        );
        let _ = writeln!(
            result,
            "  \\begin{{scope}}[transparency group,opacity={}]",
            number(op.style.opacity)
        );
        for shape in &op.shapes {
            element(&mut result, shape, &op.style);
        }
        result.push_str("  \\end{scope}\n");
    }
    result.push_str("\\end{tikzpicture}\n");
    Ok(result)
}

fn element(output: &mut String, shape: &Shape, style: &Style) {
    match shape {
        Shape::Circle {
            center,
            radius,
            disk,
        } => {
            let _ = writeln!(
                output,
                "    \\path[{}] {} circle[radius={}];",
                attributes(style, true, *disk),
                point(*center),
                number(*radius)
            );
        }
        Shape::Ellipse {
            center,
            radius_x,
            radius_y,
            rotation,
        } => {
            let _ = writeln!(
                output,
                "    \\path[{},rotate around={{{}:{} }}] {} ellipse[x radius={},y radius={}];",
                attributes(style, true, false),
                number(rotation * 180.0 / core::f64::consts::PI),
                point(*center),
                point(*center),
                number(*radius_x),
                number(*radius_y)
            );
        }
        Shape::Path {
            commands,
            closed,
            ink,
        } => {
            let _ = writeln!(
                output,
                "    \\path[{}] {};",
                attributes(style, *closed, *ink),
                path_data(commands)
            );
        }
        Shape::Arc {
            start,
            radius,
            sweep,
            angle,
            ..
        } => {
            let _ = writeln!(
                output,
                "    \\path[{}] {} arc[start angle={},delta angle={},radius={}];",
                attributes(style, false, false),
                point(*start),
                number(angle * 180.0 / core::f64::consts::PI),
                number(sweep * 180.0 / core::f64::consts::PI),
                number(*radius)
            );
        }
    }
}

fn point(p: Point) -> String {
    format!("({},{})", number(p.x), number(p.y))
}

fn path_data(commands: &[Command]) -> String {
    commands
        .iter()
        .map(|command| match command {
            Command::Move(p) => point(*p),
            Command::Line(p) => format!("-- {}", point(*p)),
            Command::Cubic(a, b, c) => format!(
                ".. controls {} and {} .. {}",
                point(*a),
                point(*b),
                point(*c)
            ),
            Command::Close => "-- cycle".into(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn color(c: Color) -> String {
    format!("{{rgb,255:red,{};green,{};blue,{}}}", c.0, c.1, c.2)
}

fn attributes(style: &Style, closed: bool, ink: bool) -> String {
    if ink {
        return format!("draw=none,fill={}", color(style.accent));
    }
    let fill = if closed {
        style.fill.map_or_else(|| "none".into(), color)
    } else {
        "none".into()
    };
    let cap = if style.cap == "square" {
        "rect"
    } else {
        style.cap
    };
    let mut result = format!(
        "draw={},fill={fill},line width={}pt,line cap={cap},line join={}",
        color(style.accent),
        number(style.weight * 0.75),
        style.join
    );
    if !style.dash.is_empty() {
        let dash = style
            .dash
            .iter()
            .enumerate()
            .map(|(i, v)| {
                format!(
                    "{} {}pt",
                    if i % 2 == 0 { "on" } else { "off" },
                    number(v * 0.75)
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(result, ",dash pattern={dash}");
    }
    result
}
