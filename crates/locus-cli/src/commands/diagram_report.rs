//! Public accepted layout metadata without positions or control coordinates.

use locus_core::{AnnotationSpec, Evaluation, ResolvedObject, Scene};
use locus_core::{PathGeometry, ResolvedPath};
use locus_layout::LayoutScene;
use locus_syntax::{ExpansionOrigin, Statement, to_interchange};
use serde_json::{Value, json};

pub(super) fn origins(scene: &Scene) -> Vec<ExpansionOrigin> {
    if scene.collections.is_empty() {
        Vec::new()
    } else {
        scene.origins.clone()
    }
}

pub(super) fn describe(scene: &Scene, placed: &LayoutScene) -> Vec<Value> {
    let interchange = to_interchange(&scene.file);
    let resolved = match &scene.evaluation {
        Evaluation::Resolved(scene) | Evaluation::Solved { scene, .. } => scene,
    };
    interchange
        .ops
        .iter()
        .filter_map(|op| {
            let object = resolved.objects.iter().find(|(id, _)| id.name == op.id)?.1;
            let details = metadata(scene, placed, &op.id, object)?;
            Some(
                json!({"id": op.id, "definition": op.def, "properties": op.props,
            "accepted": details}),
            )
        })
        .collect()
}

fn metadata(
    scene: &Scene,
    placed: &LayoutScene,
    id: &str,
    object: &ResolvedObject,
) -> Option<Value> {
    match object {
        ResolvedObject::Annotation(AnnotationSpec::Label { path, .. }) => {
            label_metadata(scene, id, path.as_ref())
        }
        ResolvedObject::Path(ResolvedPath {
            geometry:
                PathGeometry::Routed {
                    route,
                    from,
                    to,
                    bend,
                    ..
                },
            ..
        }) => {
            let logical = route
                .clone()
                .or_else(|| direct_route(placed, from, to, *bend))?;
            Some(route_metadata(scene, placed, id, &logical))
        }
        ResolvedObject::Rect(_) | ResolvedObject::Diamond(_) | ResolvedObject::RoundedRect(_) => {
            dimensions(scene, id, object)
        }
        _ => None,
    }
}

fn dimensions(scene: &Scene, id: &str, object: &ResolvedObject) -> Option<Value> {
    let op = scene
        .file
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Op(op) if op.id.name == id => Some(op),
            _ => None,
        })?;
    let automatic =
        matches!(&op.ctor, locus_syntax::Constructor::Call { args, .. } if args.is_empty());
    let bounds = object.bbox()?;
    let (width, height) = match object {
        ResolvedObject::RoundedRect(rect) => (rect.width, rect.height),
        _ => (bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y),
    };
    Some(json!({"automatic_dimensions": automatic, "width": width, "height": height}))
}

fn label_metadata(
    scene: &Scene,
    id: &str,
    path: Option<&locus_core::scene::PathLabel>,
) -> Option<Value> {
    let measured = scene.inputs.measured.get(id)?;
    let mut details = json!({"width": measured.width, "height": measured.height,
        "role": format!("{:?}", measured.role).to_lowercase()});
    if let Some(path) = path {
        details["attachment"] = json!({"path": path.path,
            "side": format!("{:?}", path.side).to_lowercase(), "region": path.region});
    }
    Some(details)
}

fn route_metadata(
    scene: &Scene,
    placed: &LayoutScene,
    id: &str,
    route: &locus_core::geom::route::Route,
) -> Value {
    let mode = if !route.via.is_empty() {
        "orthogonal"
    } else if route.parameter.is_some() {
        "curved"
    } else {
        "direct"
    };
    let mut details = json!({"route": mode, "logical_length": route.length(),
        "clipped": placed.visible_paths.contains_key(id)});
    if let Some(choice) = scene.inputs.choices.routes.get(id) {
        details["side"] = json!(format!("{:?}", choice.side).to_lowercase());
    }
    details
}

fn direct_route(
    placed: &LayoutScene,
    from: &locus_core::AnchorSpec,
    to: &locus_core::AnchorSpec,
    bend: f64,
) -> Option<locus_core::geom::route::Route> {
    locus_core::geom::route::Route::direct(anchor(placed, from)?, anchor(placed, to)?, bend).ok()
}

fn anchor(placed: &LayoutScene, anchor: &locus_core::AnchorSpec) -> Option<locus_core::Point> {
    match anchor {
        locus_core::AnchorSpec::Point(point) => Some(*point),
        locus_core::AnchorSpec::AnnotationCenter { id, .. } => {
            Some(placed.annotations.get(id)?.text.as_ref()?.bounds.center())
        }
    }
}
