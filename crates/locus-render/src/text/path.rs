//! Explicit SVG glyph path commands in shared physical coordinates.
use super::transform::{Matrix, map};
use crate::{
    RenderError,
    primitives::{Command, Shape, quadratic},
};
use locus_core::Point;
use locus_syntax::Span;
use svgtypes::{PathParser, PathSegment};

pub(super) fn parse_path(data: &str, transform: Matrix, span: Span) -> Result<Shape, RenderError> {
    let mut commands = Vec::new();
    let mut cursor = Point::default();
    let mut start = cursor;
    for segment in PathParser::from(data) {
        let segment = segment.map_err(|e| RenderError::invalid(span, e.to_string()))?;
        let command = path_command(segment, &mut cursor, &mut start, span)?;
        commands.push(map_command(command, transform));
    }
    Ok(Shape::Path {
        commands,
        closed: true,
        ink: true,
    })
}

fn map_command(command: Command, transform: Matrix) -> Command {
    match command {
        Command::Move(p) => Command::Move(map(transform, p)),
        Command::Line(p) => Command::Line(map(transform, p)),
        Command::Cubic(a, b, c) => {
            Command::Cubic(map(transform, a), map(transform, b), map(transform, c))
        }
        Command::Close => Command::Close,
    }
}

fn path_command(
    segment: PathSegment,
    cursor: &mut Point,
    start: &mut Point,
    span: Span,
) -> Result<Command, RenderError> {
    let point = |abs, x, y| {
        if abs {
            Point::new(x, y)
        } else {
            Point::new(cursor.x + x, cursor.y + y)
        }
    };
    let (command, end) = match segment {
        PathSegment::MoveTo { abs, x, y } => {
            let p = point(abs, x, y);
            *start = p;
            (Command::Move(p), p)
        }
        PathSegment::LineTo { abs, x, y } => {
            let p = point(abs, x, y);
            (Command::Line(p), p)
        }
        PathSegment::CurveTo {
            abs,
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => {
            let p = point(abs, x, y);
            (Command::Cubic(point(abs, x1, y1), point(abs, x2, y2), p), p)
        }
        PathSegment::Quadratic { abs, x1, y1, x, y } => {
            let p = point(abs, x, y);
            (quadratic(*cursor, point(abs, x1, y1), p), p)
        }
        PathSegment::ClosePath { .. } => (Command::Close, *start),
        other => linear_command(other, *cursor, span)?,
    };
    *cursor = end;
    Ok(command)
}

fn linear_command(
    segment: PathSegment,
    cursor: Point,
    span: Span,
) -> Result<(Command, Point), RenderError> {
    Ok(match segment {
        PathSegment::HorizontalLineTo { abs, x } => {
            let p = Point::new(if abs { x } else { cursor.x + x }, cursor.y);
            (Command::Line(p), p)
        }
        PathSegment::VerticalLineTo { abs, y } => {
            let p = Point::new(cursor.x, if abs { y } else { cursor.y + y });
            (Command::Line(p), p)
        }
        _ => {
            return Err(RenderError::invalid(
                span,
                "measured glyphs must use explicit move, line, quadratic, cubic, and close commands",
            ));
        }
    })
}
