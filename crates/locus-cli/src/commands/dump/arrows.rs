//! Arrow previews use each snapshot's frozen route and resolved anchors.
use locus_core::{AnchorSpec, Point, ResolvedObject, ResolvedScene, geom::route::Route};
use locus_core::{PathGeometry, ResolvedPath};
use locus_layout::{LayoutScene, Leader, PlacedAnnotation};

pub(super) fn append(scene: &ResolvedScene, layout: &mut LayoutScene) {
    for (id, object) in &scene.objects {
        let ResolvedObject::Path(path) = object else {
            continue;
        };
        if matches!(path.geometry, PathGeometry::Segment(_)) {
            continue;
        }
        let head = path.head;
        let Some(route) = route(path, layout) else {
            continue;
        };
        let bounds = route.bbox();
        let leader = if route.via.is_empty() {
            Leader::Arrow {
                from: route.from.into(),
                to: route.to.into(),
                control: route.control.map(Into::into),
                head,
            }
        } else {
            Leader::Routed { route, head }
        };
        layout.annotations.insert(
            id.name.clone(),
            PlacedAnnotation {
                id: id.clone(),
                spec: object.clone(),
                bounds,
                text: None,
                leader,
                strokes: Vec::new(),
                candidate_index: 0,
            },
        );
        layout.bbox = layout.bbox.union(bounds);
    }
}

fn route(path: &ResolvedPath, layout: &LayoutScene) -> Option<Route> {
    if let Some(route) = path.route().ok().flatten() {
        return Some(route);
    }
    let PathGeometry::Routed { from, to, bend, .. } = &path.geometry else {
        return None;
    };
    Route::direct(anchor(from, layout)?, anchor(to, layout)?, *bend).ok()
}

fn anchor(anchor: &AnchorSpec, layout: &LayoutScene) -> Option<Point> {
    match anchor {
        AnchorSpec::Point(point) => Some(*point),
        AnchorSpec::AnnotationCenter { id, .. } => {
            Some(layout.annotations.get(id)?.bounds.center())
        }
    }
}
