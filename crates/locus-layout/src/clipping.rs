//! Visible route intervals; logical route length and bounds remain untouched.

use crate::discrete::{LayoutFeedback, Requirement, conflict, members, object, route};
use crate::{LayoutError, Leader};
use locus_core::geom::{route::Route, sdf::object_signed_distance};
use locus_core::{Point, ResolvedObject, Scene};

struct ClipRequest {
    boundaries: Vec<String>,
    span: locus_syntax::Span,
}

pub(crate) fn apply(
    scene: &Scene,
    requirements: &[Requirement<'_>],
    feedback: &mut LayoutFeedback,
) -> Result<(), LayoutError> {
    let mut requests = indexmap::IndexMap::<String, ClipRequest>::new();
    for requirement in requirements.iter().filter(|r| r.name == "trim_endpoints") {
        collect_clips(scene, requirement, &mut requests)?;
    }
    for (name, request) in requests {
        clip_path(scene, &name, &request, feedback)?;
    }
    Ok(())
}

fn clip_path(
    scene: &Scene,
    name: &str,
    request: &ClipRequest,
    feedback: &mut LayoutFeedback,
) -> Result<(), LayoutError> {
    let logical = crate::routing::planned_route(scene, name, feedback).ok_or_else(|| {
        conflict(
            request.span,
            &[name.to_owned()],
            "clipping subject is not a finite path",
        )
    })?;
    let range = range_for_boundaries(scene, &logical, request);
    let (accepted, provisional) = if range.is_ok() {
        (logical.clone(), false)
    } else {
        let candidate = crate::routing::clipping_candidate(scene, name, feedback, |candidate| {
            range_for_boundaries(scene, candidate, request).is_ok()
        });
        let Some(candidate) = candidate else {
            let message = format!(
                "path {name}, boundaries {}: {}",
                request.boundaries.join(", "),
                range
                    .err()
                    .unwrap_or("clipping search exhausted its candidate cap")
            );
            return Err(conflict(request.span, &[name.to_owned()], &message));
        };
        (candidate, true)
    };
    let range = range_for_boundaries(scene, &accepted, request)
        .map_err(|message| conflict(request.span, &[name.to_owned()], message))?;
    feedback.retained_routes.insert(name.to_owned(), range);
    feedback.retained_distances.insert(
        name.to_owned(),
        (
            accepted.distance_at_parameter(&range.0),
            accepted.length() - accepted.distance_at_parameter(&range.1),
        ),
    );
    // A changed route is frozen by feedback before it can become an accepted drawing.
    if !provisional {
        apply_visible_aliases(scene, name, &retain(&logical, range), feedback);
    }
    Ok(())
}

fn range_for_boundaries(
    scene: &Scene,
    route: &Route,
    request: &ClipRequest,
) -> Result<(f64, f64), &'static str> {
    let mut result = (0.0_f64, 1.0_f64);
    for name in &request.boundaries {
        let boundary = object(scene, name).ok_or("unresolved clipping boundary")?;
        let range = retained_interval(route, boundary)?;
        result.0 = result.0.max(range.0);
        result.1 = result.1.min(range.1);
    }
    if result.0 < result.1 {
        Ok(result)
    } else {
        Err("incompatible endpoint clipping boundaries")
    }
}

fn apply_visible_aliases(
    scene: &Scene,
    identity: &str,
    visible: &Route,
    feedback: &mut LayoutFeedback,
) {
    let Some(resolved) = scene.resolved() else {
        return;
    };
    for (id, object) in &resolved.objects {
        if crate::discrete::alias_identity(scene, &id.name) != identity {
            continue;
        }
        if let Some(annotation) = feedback.layout.annotations.get_mut(&id.name) {
            annotation.leader = visible_leader(visible, &annotation.leader);
        } else if route(object).is_some() {
            feedback
                .layout
                .visible_paths
                .insert(id.name.clone(), visible.clone());
        }
    }
}

fn collect_clips(
    scene: &Scene,
    requirement: &Requirement<'_>,
    requests: &mut indexmap::IndexMap<String, ClipRequest>,
) -> Result<(), LayoutError> {
    let [subject, boundary] = requirement.args else {
        return Err(conflict(
            requirement.span,
            &[],
            "trim_endpoints requires paths and one closed boundary",
        ));
    };
    let boundary = boundary_name(scene, boundary, requirement.span)?;
    let paths = members(scene, subject);
    let mut eligible = false;
    for name in &paths {
        if object(scene, name).and_then(route).is_some() {
            eligible = true;
            let request = requests.entry(name.clone()).or_insert(ClipRequest {
                boundaries: Vec::new(),
                span: requirement.span,
            });
            if !request.boundaries.contains(&boundary) {
                request.boundaries.push(boundary.clone());
            }
        }
    }
    if !eligible {
        return Err(conflict(
            requirement.span,
            &paths,
            "clipping collection has no finite paths",
        ));
    }
    Ok(())
}

fn boundary_name(
    scene: &Scene,
    expression: &locus_syntax::Expr,
    span: locus_syntax::Span,
) -> Result<String, LayoutError> {
    if is_collection(scene, expression) {
        return Err(conflict(
            span,
            &[],
            "clipping boundary must be a supported closed shape, not a collection",
        ));
    }
    let names = members(scene, expression);
    let [name] = names.as_slice() else {
        return Err(conflict(
            span,
            &names,
            "clipping boundary must be one supported closed shape",
        ));
    };
    if object(scene, name)
        .is_none_or(|boundary| object_signed_distance(boundary, Point::new(0.0, 0.0)).is_none())
    {
        return Err(conflict(
            span,
            &names,
            "unresolved or unsupported closed clipping boundary",
        ));
    }
    Ok(name.clone())
}

fn is_collection(scene: &Scene, expression: &locus_syntax::Expr) -> bool {
    match expression {
        locus_syntax::Expr::Ref(id) => scene.collections.contains_key(&id.name),
        locus_syntax::Expr::Group { expr, .. } => is_collection(scene, expr),
        _ => false,
    }
}

fn retained_interval(route: &Route, boundary: &ResolvedObject) -> Result<(f64, f64), &'static str> {
    let start = exit_parameter(route, boundary, false)?;
    let end = exit_parameter(route, boundary, true)?;
    if start < end {
        Ok((start, end))
    } else {
        Err("incompatible endpoint clipping boundaries")
    }
}

fn exit_parameter(
    route: &Route,
    boundary: &ResolvedObject,
    reverse: bool,
) -> Result<f64, &'static str> {
    let parameter = |t: f64| if reverse { 1.0 - t } else { t };
    let distance = |t| {
        object_signed_distance(boundary, route.sample(&parameter(t)).into())
            .unwrap_or(f64::INFINITY)
    };
    if distance(0.0) >= 0.0 {
        return Ok(parameter(0.0));
    }
    let speed = route.parameter_speed();
    let mut remaining = locus_core::preset::CLIPPING_INTERVAL_CAP;
    let bracket = first_exit(
        &distance,
        speed,
        (0.0, 1.0),
        (distance(0.0), distance(1.0)),
        0,
        &mut remaining,
    );
    let bracket = bracket.ok_or(if remaining == 0 {
        "endpoint clipping exhausted its deterministic interval cap"
    } else {
        "path is fully enclosed by clipping boundary"
    })?;
    let (mut lower, mut upper) = bracket;
    for _ in 0..locus_core::preset::CLIPPING_BISECTIONS {
        let middle = lower.midpoint(upper);
        if distance(middle) < 0.0 {
            lower = middle;
        } else {
            upper = middle;
        }
    }
    Ok(parameter(lower.midpoint(upper)))
}

// Signed distance is 1-Lipschitz. A speed bound rejects intervals that
// provably stay inside, including tiny exit/reentry intervals missed by sampling.
fn first_exit(
    distance: &impl Fn(f64) -> f64,
    speed: f64,
    interval: (f64, f64),
    values: (f64, f64),
    depth: usize,
    remaining: &mut usize,
) -> Option<(f64, f64)> {
    if *remaining == 0 {
        return None;
    }
    *remaining -= 1;
    if values.0 >= 0.0 {
        return Some((interval.0, interval.0));
    }
    if values.0.max(values.1) + speed * (interval.1 - interval.0) / 2.0 < 0.0 {
        return None;
    }
    if depth >= locus_core::preset::CLIPPING_SEARCH_DEPTH {
        return (values.1 >= 0.0).then_some(interval);
    }
    let middle = interval.0.midpoint(interval.1);
    let value = distance(middle);
    first_exit(
        distance,
        speed,
        (interval.0, middle),
        (values.0, value),
        depth + 1,
        remaining,
    )
    .or_else(|| {
        first_exit(
            distance,
            speed,
            (middle, interval.1),
            (value, values.1),
            depth + 1,
            remaining,
        )
    })
}

pub(crate) fn retain(route: &Route, range: (f64, f64)) -> Route {
    route.retained(range)
}

pub(crate) fn visible_leader(route: &Route, old: &Leader) -> Leader {
    let head = match old {
        Leader::Arrow { head, .. } | Leader::Routed { head, .. } => *head,
        _ => locus_syntax::HeadKind::None,
    };
    if route.via.is_empty() {
        Leader::Arrow {
            from: route.from.clone().into(),
            to: route.to.clone().into(),
            control: route.control.clone().map(Into::into),
            head,
        }
    } else {
        Leader::Routed {
            route: route.clone(),
            head,
        }
    }
}
