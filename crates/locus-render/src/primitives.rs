//! A single backend-neutral display list in physical, y-down CSS pixels.
mod geometry;
use geometry::{geometry, segment_shape};
use locus_core::{PathGeometry, ResolvedPath};

use crate::{
    RenderError, RenderOptions,
    style::{Color, Style, StyleSheet},
    viewport::Viewport,
};
use locus_core::{Point, ResolvedObject, ResolvedScene, Vec2};
use locus_layout::{LayoutScene, Leader, PlacedAnnotation};
use locus_syntax::{HeadKind, Ident};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Command {
    Move(Point),
    Line(Point),
    Cubic(Point, Point, Point),
    Close,
}

#[derive(Clone, Debug)]
pub(crate) enum Shape {
    Path {
        commands: Vec<Command>,
        closed: bool,
        ink: bool,
    },
    RoundedRect {
        min: Point,
        max: Point,
        radius: f64,
    },
    Circle {
        center: Point,
        radius: f64,
        disk: bool,
    },
    Ellipse {
        center: Point,
        radius_x: f64,
        radius_y: f64,
        rotation: f64,
    },
    Arc {
        start: Point,
        end: Point,
        radius: f64,
        sweep: f64,
        angle: f64,
    },
}

pub(crate) struct Op {
    pub id: Ident,
    pub style: Style,
    pub shapes: Vec<Shape>,
}
pub(crate) struct Drawing {
    pub viewport: Viewport,
    pub ops: Vec<Op>,
    pub background: Option<Color>,
}

pub(crate) fn prepare(
    scene: &ResolvedScene,
    layout: &LayoutScene,
    styles: &StyleSheet,
    options: RenderOptions,
) -> Result<Drawing, RenderError> {
    let viewport = Viewport::fit(scene, layout, styles, options)?;
    let mut ops = Vec::new();
    for (id, object) in &scene.objects {
        if matches!(
            object,
            ResolvedObject::Annotation(_)
                | ResolvedObject::Path(ResolvedPath {
                    geometry: PathGeometry::Routed { .. },
                    ..
                })
        ) || styles.for_object(&id.name).opacity == 0.0
        {
            continue;
        }
        let style = styles.for_object(&id.name);
        let shapes = object_shapes(
            object,
            layout.visible_paths.get(&id.name),
            viewport,
            style.weight,
        );
        push(&mut ops, id, style, shapes)?;
    }
    for annotation in layout.annotations.values() {
        let style = styles.for_object(&annotation.id.name);
        if style.opacity == 0.0 {
            continue;
        }
        let shapes = annotation_shapes(annotation, viewport, &style)?;
        push(&mut ops, &annotation.id, style, shapes)?;
    }
    Ok(Drawing {
        viewport,
        ops,
        background: options.background,
    })
}

fn object_shapes(
    object: &ResolvedObject,
    visible: Option<&locus_core::geom::route::Route>,
    view: Viewport,
    weight: f64,
) -> Vec<Shape> {
    let Some(path) = object.as_path() else {
        return geometry(object, view);
    };
    let logical = path.route().ok().flatten();
    visible.or(logical.as_ref()).map_or_else(
        || geometry(object, view),
        |route| routed_arrow(route, path.head, view, weight),
    )
}

fn push(
    ops: &mut Vec<Op>,
    id: &Ident,
    style: Style,
    shapes: Vec<Shape>,
) -> Result<(), RenderError> {
    if !shapes.iter().all(valid_shape) {
        return Err(RenderError::invalid(
            id.span,
            "non-finite rendered geometry",
        ));
    }
    if !shapes.is_empty() && style.opacity != 0.0 {
        ops.push(Op {
            id: id.clone(),
            style,
            shapes,
        });
    }
    Ok(())
}

fn valid_shape(shape: &Shape) -> bool {
    match shape {
        Shape::Path { commands, .. } => commands.iter().all(valid_command),
        Shape::RoundedRect { min, max, radius } => {
            min.is_finite() && max.is_finite() && radius.is_finite() && *radius >= 0.0
        }
        Shape::Circle { center, radius, .. } => {
            center.is_finite() && radius.is_finite() && *radius > 0.0
        }
        Shape::Ellipse {
            center,
            radius_x,
            radius_y,
            rotation,
        } => {
            center.is_finite()
                && radius_x.is_finite()
                && *radius_x > 0.0
                && radius_y.is_finite()
                && *radius_y > 0.0
                && rotation.is_finite()
        }
        Shape::Arc {
            start,
            end,
            radius,
            sweep,
            angle,
        } => {
            start.is_finite()
                && end.is_finite()
                && radius.is_finite()
                && *radius > 0.0
                && sweep.is_finite()
                && angle.is_finite()
        }
    }
}

fn valid_command(command: &Command) -> bool {
    match command {
        Command::Move(p) | Command::Line(p) => p.is_finite(),
        Command::Cubic(a, b, c) => a.is_finite() && b.is_finite() && c.is_finite(),
        Command::Close => true,
    }
}

pub(crate) fn path(points: impl IntoIterator<Item = Point>, closed: bool, ink: bool) -> Shape {
    let mut iter = points.into_iter();
    let mut commands = Vec::new();
    if let Some(first) = iter.next() {
        commands.push(Command::Move(first));
    }
    commands.extend(iter.map(Command::Line));
    if closed {
        commands.push(Command::Close);
    }
    Shape::Path {
        commands,
        closed,
        ink,
    }
}

fn annotation_shapes(
    annotation: &PlacedAnnotation,
    view: Viewport,
    style: &Style,
) -> Result<Vec<Shape>, RenderError> {
    let mut result: Vec<_> = annotation
        .strokes
        .iter()
        .map(|s| segment_shape(*s, view))
        .collect();
    match &annotation.leader {
        Leader::None => {}
        Leader::Segment(segment) => result.push(segment_shape(*segment, view)),
        Leader::Routed { route, head } => {
            result.extend(routed_arrow(route, *head, view, style.weight));
        }
        Leader::Arrow {
            from,
            to,
            control,
            head,
        } => {
            result.extend(arrow(
                view.to_svg(*from),
                view.to_svg(*to),
                control.map(|p| view.to_svg(p)),
                *head,
                style.weight,
            ));
        }
    }
    if let Some(label) = &annotation.text {
        result.extend(crate::text::outlines(label, view, annotation.id.span)?);
    }
    Ok(result)
}

fn routed_arrow(
    route: &locus_core::geom::route::Route,
    head: HeadKind,
    view: Viewport,
    weight: f64,
) -> Vec<Shape> {
    if route.via.is_empty() {
        return arrow(
            view.to_svg(route.from.clone().into()),
            view.to_svg(route.to.clone().into()),
            route.control.clone().map(|point| view.to_svg(point.into())),
            head,
            weight,
        );
    }
    let points: Vec<Point> = core::iter::once(&route.from)
        .chain(route.via.iter())
        .chain(core::iter::once(&route.to))
        .cloned()
        .map(|point| view.to_svg(point.into()))
        .collect();
    let mut commands = vec![Command::Move(points[0])];
    commands.extend(points.iter().skip(1).copied().map(Command::Line));
    let mut result = vec![Shape::Path {
        commands,
        closed: false,
        ink: false,
    }];
    if head != HeadKind::None {
        let last = points.len() - 1;
        result.push(arrowhead(
            points[last],
            points[last] - points[last - 1],
            weight,
        ));
    }
    if head == HeadKind::Both {
        result.push(arrowhead(points[0], points[0] - points[1], weight));
    }
    result
}

fn arrow(
    from: Point,
    to: Point,
    control: Option<Point>,
    head: HeadKind,
    weight: f64,
) -> Vec<Shape> {
    let mut commands = vec![Command::Move(from)];
    if let Some(control) = control {
        commands.push(quadratic(from, control, to));
    } else {
        commands.push(Command::Line(to));
    }
    let mut result = vec![Shape::Path {
        commands,
        closed: false,
        ink: false,
    }];
    if head != HeadKind::None && from != to {
        result.push(arrowhead(to, to - control.unwrap_or(from), weight));
    }
    if head == HeadKind::Both && from != to {
        result.push(arrowhead(from, from - control.unwrap_or(to), weight));
    }
    result
}

fn arrowhead(tip: Point, direction: Vec2, weight: f64) -> Shape {
    let length = direction.norm();
    let direction = if length == 0.0 {
        Vec2::new(0.0, 0.0)
    } else {
        direction / length
    };
    let back = tip - direction * (6.0 + weight);
    let normal = direction.perpendicular() * (2.5 + weight * 0.5);
    path([tip, back + normal, back - normal], true, true)
}

pub(crate) fn quadratic(from: Point, control: Point, to: Point) -> Command {
    Command::Cubic(
        from + (control - from) * (2.0 / 3.0),
        to + (control - to) * (2.0 / 3.0),
        to,
    )
}
