//! Relation-aware deterministic layout seeding. Unrelated geometry retains ring seeds.
use super::{EvalError, UnknownClass, UnknownSystem};
use crate::eval::PreparedEvaluation;
use locus_syntax::Expr;
use std::collections::{BTreeMap, BTreeSet};
mod captions;
mod incidence;
mod movement;
mod numeric;
pub(super) mod repair;

pub(super) fn initialize(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
) -> Result<(), EvalError> {
    let sources = super::relations::collect(prepared.file());
    if !sources
        .iter()
        .any(|s| super::layout::is_layout(s.expression))
    {
        return Ok(());
    }
    system.seed_owners = movement::index(prepared, system);
    let seed = prepared.evaluate_trial(Some(&system.hints))?;
    let mut boxes = BTreeMap::new();
    for (id, object) in seed.objects() {
        if let Some(bounds) = object.bbox() {
            boxes.insert(id.name.clone(), bounds);
        }
    }
    for source in &sources {
        seed_anchor(prepared, system, source, &mut boxes);
    }
    numeric::seed(prepared, system, &sources, &mut boxes)?;
    for horizontal in [true, false] {
        seed_order_axis(prepared, system, &sources, &mut boxes, horizontal);
    }
    seed_containers(prepared, system, &sources, &mut boxes);
    seed_shared_coordinates(prepared, system, &sources, &mut boxes);
    incidence::seed(prepared, system, &sources, &mut boxes)?;
    system.normalized_hints = system
        .hints
        .iter()
        .zip(&system.scales)
        .map(|(v, s)| v / s)
        .collect();
    Ok(())
}

fn seed_order_axis(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[super::SourceRelation<'_>],
    boxes: &mut BTreeMap<String, crate::BBox>,
    horizontal: bool,
) {
    // First settle owned member sizes/orders, then recompute combined collection extents.
    for _ in 0..2 {
        let mut graph_boxes = boxes.clone();
        let mut movable = movable_names(system);
        append_collection_bounds(prepared, boxes, &mut graph_boxes, &mut movable);
        let graph = order_graph(prepared, system, sources, &graph_boxes, horizontal);
        let positions = longest_paths(&graph, &graph_boxes, &movable, horizontal);
        apply_positions(prepared, system, boxes, &positions, horizontal, !horizontal);
    }
}

fn apply_positions(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    boxes: &mut BTreeMap<String, crate::BBox>,
    positions: &BTreeMap<String, f64>,
    horizontal: bool,
    downward: bool,
) {
    for collections in [false, true] {
        for (id, coordinate) in positions {
            if prepared.collections().contains_key(id) != collections {
                continue;
            }
            let Some(bounds) = aggregate(prepared, id, boxes) else {
                continue;
            };
            let coordinate = if downward { -*coordinate } else { *coordinate };
            let mut center = bounds.center();
            if horizontal {
                center.x = coordinate;
            } else {
                center.y = coordinate;
            }
            move_members(prepared, system, id, center - bounds.center(), boxes);
        }
    }
}

fn seed_relation(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    expression: &Expr,
    boxes: &mut BTreeMap<String, crate::BBox>,
) -> bool {
    let Expr::Call { name, args, .. } = expression else {
        return false;
    };
    match name.name.as_str() {
        "center" => seed_argument_alignment(prepared, system, args, boxes, "center"),
        name if name.starts_with("align_") => {
            seed_argument_alignment(prepared, system, args, boxes, name)
        }
        _ => false,
    }
}

fn movable_names(system: &UnknownSystem) -> BTreeSet<String> {
    system
        .objects
        .iter()
        .filter(|object| {
            object.count == 2
                && matches!(
                    object.class,
                    UnknownClass::Geometric | UnknownClass::LabelPlacement
                )
        })
        .map(|object| object.id.clone())
        .chain(
            system
                .seed_owners
                .iter()
                .filter(|(_, owners)| !owners.is_empty())
                .map(|(name, _)| name.clone()),
        )
        .collect()
}

fn seed_argument_alignment(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    args: &[Expr],
    boxes: &mut BTreeMap<String, crate::BBox>,
    name: &str,
) -> bool {
    let Some(Expr::Ref(first)) = args.first() else {
        return false;
    };
    let Some(reference) = aggregate(prepared, &first.name, boxes) else {
        return false;
    };
    for expression in &args[1..] {
        let Expr::Ref(id) = expression else {
            continue;
        };
        let Some(bounds) = aggregate(prepared, &id.name, boxes) else {
            continue;
        };
        let center = if name == "center" {
            reference.center()
        } else {
            aligned_center(reference, bounds, name, 0.0)
        };
        move_members(prepared, system, &id.name, center - bounds.center(), boxes);
    }
    true
}

fn operand_names(args: &[Expr]) -> Vec<String> {
    args.iter()
        .filter_map(|arg| match arg {
            Expr::Ref(id) => Some(id.name.clone()),
            _ => None,
        })
        .collect()
}

fn chain_members(prepared: &PreparedEvaluation<'_>, args: &[Expr], name: &str) -> Vec<String> {
    if !matches!(name, "row" | "column") {
        return operand_names(args);
    }
    let aliases = crate::program::alias_identities(prepared.file());
    operand_names(&crate::program::chain_operands(
        prepared.collections(),
        &aliases,
        args,
    ))
}

fn aligned_center(
    reference: crate::BBox,
    bounds: crate::BBox,
    name: &str,
    padding: f64,
) -> crate::Point {
    let mut center = bounds.center();
    match name {
        "align_left" | "inset_left" => center.x = reference.min.x + bounds.width() / 2.0 + padding,
        "align_right" | "inset_right" => {
            center.x = reference.max.x - bounds.width() / 2.0 - padding;
        }
        "align_top" | "inset_top" => center.y = reference.max.y - bounds.height() / 2.0 - padding,
        "align_bottom" | "inset_bottom" => {
            center.y = reference.min.y + bounds.height() / 2.0 + padding;
        }
        "align_center_x" => center.x = reference.center().x,
        _ => center.y = reference.center().y,
    }
    center
}

fn seed_anchor(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    source: &super::SourceRelation<'_>,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    let Expr::Call { name, args, .. } = source.expression else {
        return;
    };
    if name.name.starts_with("inset_") {
        seed_inset(
            prepared,
            system,
            args,
            &name.name,
            source.padding_none,
            boxes,
        );
    } else if matches!(name.name.as_str(), "contain" | "cover_x" | "cover_y") {
        seed_container(
            prepared,
            system,
            args,
            &name.name,
            source.padding_none,
            boxes,
        );
    } else if name.name == "center" || name.name.starts_with("align_") {
        seed_relation(prepared, system, source.expression, boxes);
    }
}

fn seed_inset(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    args: &[Expr],
    name: &str,
    padding_none: bool,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    let [Expr::Ref(content), Expr::Ref(container)] = args else {
        return;
    };
    let Some(reference) = aggregate(prepared, &container.name, boxes) else {
        return;
    };
    let Some(bounds) = aggregate(prepared, &content.name, boxes) else {
        return;
    };
    let padding = if padding_none {
        0.0
    } else {
        prepared.inputs().metrics.padding
    };
    let center = aligned_center(reference, bounds, name, padding);
    move_members(
        prepared,
        system,
        &content.name,
        center - bounds.center(),
        boxes,
    );
}

fn aggregate(
    prepared: &PreparedEvaluation<'_>,
    name: &str,
    boxes: &BTreeMap<String, crate::BBox>,
) -> Option<crate::BBox> {
    let Some(collection) = prepared.collections().get(name) else {
        return boxes.get(name).copied();
    };
    let bounds = collection
        .members
        .iter()
        .filter_map(|id| aggregate(prepared, id, boxes));
    bounds.reduce(|a, b| crate::BBox {
        min: crate::Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
        max: crate::Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
    })
}

fn move_members(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    name: &str,
    offset: crate::Vec2,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    let mut names = BTreeSet::new();
    movable_members(prepared, system, name, &mut names);
    for member in names {
        if let Some(bounds) = boxes.get(&member).copied() {
            move_center(system, &member, bounds.center() + offset, boxes);
        }
    }
    refresh_derived_boxes(prepared, system, boxes);
}

fn refresh_derived_boxes(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    if let Ok(trial) = prepared.evaluate_trial(Some(&system.hints)) {
        for (id, object) in trial.objects() {
            if let Some(bounds) = object.bbox() {
                boxes.insert(id.name.clone(), bounds);
            }
        }
    }
}

fn movable_members(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    name: &str,
    names: &mut BTreeSet<String>,
) {
    if let Some(owners) = system.seed_owners.get(name) {
        names.extend(owners.iter().cloned());
        return;
    }
    let Some(collection) = prepared.collections().get(name) else {
        names.insert(
            crate::program::alias_identities(prepared.file())
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_owned()),
        );
        return;
    };
    if collection.kind == locus_syntax::CollectionKind::Instance {
        append_owned_unknowns(system, name, names);
    } else {
        for member in &collection.members {
            movable_members(prepared, system, member, names);
        }
    }
}

fn append_owned_unknowns(system: &UnknownSystem, name: &str, names: &mut BTreeSet<String>) {
    let prefix = format!("{name}.");
    names.extend(
        system
            .objects
            .iter()
            .filter(|object| object.id.starts_with(&prefix))
            .map(|object| object.id.clone()),
    );
}

fn seed_container(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    args: &[Expr],
    name: &str,
    padding_none: bool,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    let Some(Expr::Ref(container)) = args.first() else {
        return;
    };
    let content = operand_names(&args[1..]);
    let Some(bounds) = content
        .iter()
        .filter_map(|id| aggregate(prepared, id, boxes))
        .reduce(|a, b| crate::BBox {
            min: crate::Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
            max: crate::Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
        })
    else {
        return;
    };
    let padding = if padding_none {
        0.0
    } else {
        prepared.inputs().metrics.padding
    };
    grow_container(system, &container.name, bounds, name, padding, boxes);
    let Some(before) = boxes.get(&container.name).copied() else {
        return;
    };
    let mut center = before.center();
    let axes = ordered_container_axes(prepared, &container.name);
    if name != "cover_y" && !axes.0 {
        center.x = bounds.center().x;
    }
    if name != "cover_x" && !axes.1 {
        center.y = bounds.center().y;
    }
    if movable_names(system).contains(&container.name) {
        move_center(system, &container.name, center, boxes);
        let offset = crate::Vec2::new(
            if axes.0 {
                center.x - bounds.center().x
            } else {
                0.0
            },
            if axes.1 {
                center.y - bounds.center().y
            } else {
                0.0
            },
        );
        move_contents(prepared, system, &args[1..], offset, boxes);
    } else {
        let offset = crate::Vec2::new(
            if name == "cover_y" {
                0.0
            } else {
                before.center().x - bounds.center().x
            },
            if name == "cover_x" {
                0.0
            } else {
                before.center().y - bounds.center().y
            },
        );
        move_contents(prepared, system, &args[1..], offset, boxes);
    }
}

fn move_contents(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    args: &[Expr],
    offset: crate::Vec2,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    for expression in args {
        if let Expr::Ref(id) = expression {
            move_members(prepared, system, &id.name, offset, boxes);
        }
    }
}

fn ordered_container_axes(prepared: &PreparedEvaluation<'_>, container: &str) -> (bool, bool) {
    let mut axes = (false, false);
    for source in super::relations::collect(prepared.file()) {
        let Expr::Call { name, args, .. } = source.expression else {
            continue;
        };
        if !args
            .iter()
            .any(|arg| expression_contains(prepared, arg, container))
        {
            continue;
        }
        axes.0 |= matches!(
            name.name.as_str(),
            "row" | "gap_x" | "inset_left" | "inset_right"
        );
        axes.1 |= matches!(
            name.name.as_str(),
            "column" | "gap_y" | "inset_top" | "inset_bottom"
        );
    }
    axes
}

fn expression_contains(
    prepared: &PreparedEvaluation<'_>,
    expression: &Expr,
    container: &str,
) -> bool {
    let Expr::Ref(id) = expression else {
        return false;
    };
    id.name == container
        || prepared
            .collections()
            .get(&id.name)
            .is_some_and(|collection| collection.members.iter().any(|member| member == container))
}

fn seed_containers(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[super::SourceRelation<'_>],
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    for source in sources {
        if let Expr::Call { name, args, .. } = source.expression {
            if matches!(name.name.as_str(), "contain" | "cover_x" | "cover_y") {
                seed_container(
                    prepared,
                    system,
                    args,
                    &name.name,
                    source.padding_none,
                    boxes,
                );
            }
        }
    }
}

fn grow_container(
    system: &mut UnknownSystem,
    id: &str,
    content: crate::BBox,
    name: &str,
    padding: f64,
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    let Some(object) = system
        .objects
        .iter()
        .find(|o| o.id == id && o.class == UnknownClass::AutomaticDimension)
    else {
        return;
    };
    let Some(bounds) = boxes.get_mut(id) else {
        return;
    };
    let center = bounds.center();
    let width = if name == "cover_y" {
        bounds.width()
    } else {
        bounds.width().max(content.width() + 2.0 * padding)
    };
    let height = if name == "cover_x" {
        bounds.height()
    } else {
        bounds.height().max(content.height() + 2.0 * padding)
    };
    system.hints[object.start] = width;
    system.hints[object.start + 1] = height;
    *bounds = crate::BBox {
        min: crate::Point::new(center.x - width / 2.0, center.y - height / 2.0),
        max: crate::Point::new(center.x + width / 2.0, center.y + height / 2.0),
    };
}

type OrderGraph = BTreeMap<String, Vec<(String, f64)>>;

fn order_graph(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[super::SourceRelation<'_>],
    boxes: &BTreeMap<String, crate::BBox>,
    horizontal: bool,
) -> OrderGraph {
    let mut graph = OrderGraph::new();
    for source in sources {
        let Expr::Call { name, args, .. } = source.expression else {
            continue;
        };
        let names = chain_members(prepared, args, &name.name);
        let chain = if horizontal {
            matches!(name.name.as_str(), "row" | "gap_x")
        } else {
            matches!(name.name.as_str(), "column" | "gap_y")
        };
        if chain {
            append_chain(
                &mut graph,
                &names,
                boxes,
                horizontal,
                prepared.inputs().metrics.gap,
            );
        } else {
            append_container(&mut graph, prepared, source, boxes, horizontal);
        }
    }
    for (first, second, offset) in &system.seed_lengths {
        let distance = if horizontal { offset.x } else { -offset.y };
        if distance > 0.0 {
            append_edge(&mut graph, first, second, distance);
        } else if distance < 0.0 {
            append_edge(&mut graph, second, first, -distance);
        }
    }
    graph
}

fn append_chain(
    graph: &mut OrderGraph,
    members: &[String],
    boxes: &BTreeMap<String, crate::BBox>,
    horizontal: bool,
    gap: f64,
) {
    for pair in members.windows(2) {
        let (Some(a), Some(b)) = (boxes.get(&pair[0]), boxes.get(&pair[1])) else {
            continue;
        };
        let distance = extent(*a, horizontal).midpoint(extent(*b, horizontal)) + gap;
        append_edge(graph, &pair[0], &pair[1], distance);
    }
}

fn append_container(
    graph: &mut OrderGraph,
    prepared: &PreparedEvaluation<'_>,
    source: &super::SourceRelation<'_>,
    boxes: &BTreeMap<String, crate::BBox>,
    horizontal: bool,
) {
    let Expr::Call { name, args, .. } = source.expression else {
        return;
    };
    let supported = if horizontal {
        matches!(
            name.name.as_str(),
            "contain" | "cover_x" | "inset_left" | "inset_right"
        )
    } else {
        matches!(
            name.name.as_str(),
            "contain" | "cover_y" | "inset_top" | "inset_bottom"
        )
    };
    if !supported {
        return;
    }
    let (container, content) = container_members(args, &name.name);
    let Some(container) = container else {
        return;
    };
    let padding = if source.padding_none {
        0.0
    } else {
        prepared.inputs().metrics.padding
    };
    for item in &content {
        let (Some(a), Some(b)) = (boxes.get(&container), boxes.get(item)) else {
            continue;
        };
        let distance = (extent(*b, horizontal) - extent(*a, horizontal)) / 2.0 + padding;
        if matches!(name.name.as_str(), "inset_right" | "inset_bottom") {
            append_edge(graph, item, &container, distance);
        } else {
            append_edge(graph, &container, item, distance);
        }
    }
}

fn container_members(args: &[Expr], name: &str) -> (Option<String>, Vec<String>) {
    if name.starts_with("inset_") {
        let [content, Expr::Ref(container)] = args else {
            return (None, Vec::new());
        };
        return (
            Some(container.name.clone()),
            operand_names(std::slice::from_ref(content)),
        );
    }
    let Some(Expr::Ref(container)) = args.first() else {
        return (None, Vec::new());
    };
    (Some(container.name.clone()), operand_names(&args[1..]))
}

fn extent(bounds: crate::BBox, horizontal: bool) -> f64 {
    if horizontal {
        bounds.width()
    } else {
        bounds.height()
    }
}

fn append_edge(graph: &mut OrderGraph, source: &str, target: &str, distance: f64) {
    graph
        .entry(source.to_owned())
        .or_default()
        .push((target.to_owned(), distance));
    graph.entry(target.to_owned()).or_default();
}

fn longest_paths(
    graph: &OrderGraph,
    boxes: &BTreeMap<String, crate::BBox>,
    movable: &BTreeSet<String>,
    horizontal: bool,
) -> BTreeMap<String, f64> {
    let mut degree = incoming_counts(graph);
    let mut ready = degree
        .iter()
        .filter_map(|(name, count)| (*count == 0).then_some(name.clone()))
        .collect::<BTreeSet<_>>();
    let mut positions = BTreeMap::new();
    while let Some(name) = ready.pop_first() {
        let Some(bounds) = boxes.get(&name) else {
            continue;
        };
        let seeded = if horizontal {
            bounds.center().x
        } else {
            -bounds.center().y
        };
        let coordinate = if movable.contains(&name) {
            *positions.entry(name.clone()).or_insert(seeded)
        } else {
            seeded
        };
        positions.insert(name.clone(), coordinate);
        propagate(
            &graph[&name],
            coordinate,
            &mut positions,
            &mut degree,
            &mut ready,
        );
    }
    // Cyclic components and their descendants retain seeds for the feasibility solver.
    positions.retain(|name, _| degree.get(name) == Some(&0));
    positions
}

fn incoming_counts(graph: &OrderGraph) -> BTreeMap<String, usize> {
    let mut degree = graph
        .keys()
        .map(|name| (name.clone(), 0))
        .collect::<BTreeMap<_, _>>();
    for edges in graph.values() {
        for (target, _) in edges {
            *degree.entry(target.clone()).or_default() += 1;
        }
    }
    degree
}

fn propagate(
    edges: &[(String, f64)],
    coordinate: f64,
    positions: &mut BTreeMap<String, f64>,
    degree: &mut BTreeMap<String, usize>,
    ready: &mut BTreeSet<String>,
) {
    for (target, distance) in edges {
        let value = positions.entry(target.clone()).or_insert(f64::NEG_INFINITY);
        *value = value.max(coordinate + distance);
        if let Some(count) = degree.get_mut(target) {
            *count -= 1;
            if *count == 0 {
                ready.insert(target.clone());
            }
        }
    }
}

fn seed_shared_coordinates(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[super::SourceRelation<'_>],
    boxes: &mut BTreeMap<String, crate::BBox>,
) {
    for horizontal in [true, false] {
        let mut bounds = boxes.clone();
        let mut movable = movable_names(system);
        append_collection_bounds(prepared, boxes, &mut bounds, &mut movable);
        let graph = alignment_graph(prepared, sources, &bounds, horizontal);
        let preferred = inset_containers(sources);
        let positions = shared_positions(&graph, &bounds, &movable, &preferred, horizontal);
        apply_positions(prepared, system, boxes, &positions, horizontal, false);
    }
}

fn append_collection_bounds(
    prepared: &PreparedEvaluation<'_>,
    boxes: &BTreeMap<String, crate::BBox>,
    bounds: &mut BTreeMap<String, crate::BBox>,
    movable: &mut BTreeSet<String>,
) {
    for id in prepared.collections().keys() {
        if let Some(aggregate) = aggregate(prepared, id, boxes) {
            bounds.insert(id.clone(), aggregate);
        }
        if item_movable(prepared, id, movable) {
            movable.insert(id.clone());
        }
    }
}

fn item_movable(prepared: &PreparedEvaluation<'_>, name: &str, movable: &BTreeSet<String>) -> bool {
    if movable.contains(name) {
        return true;
    }
    if let Some(identity) = crate::program::alias_identities(prepared.file()).get(name) {
        return item_movable(prepared, identity, movable);
    }
    let Some(collection) = prepared.collections().get(name) else {
        return false;
    };
    if collection.kind == locus_syntax::CollectionKind::Instance {
        let prefix = format!("{name}.");
        return movable.iter().any(|id| id.starts_with(&prefix));
    }
    collection
        .members
        .iter()
        .any(|member| item_movable(prepared, member, movable))
}

fn alignment_graph(
    prepared: &PreparedEvaluation<'_>,
    sources: &[super::SourceRelation<'_>],
    boxes: &BTreeMap<String, crate::BBox>,
    horizontal: bool,
) -> OrderGraph {
    let mut graph = OrderGraph::new();
    for source in sources {
        let Expr::Call { name, args, .. } = source.expression else {
            continue;
        };
        let Some(alignment) = axis_alignment(&name.name, horizontal) else {
            continue;
        };
        let mut ids = chain_members(prepared, args, &name.name);
        let padding = alignment_padding(prepared, source, &name.name);
        if name.name.starts_with("inset_") {
            ids.reverse();
        }
        let Some(first) = ids.first() else {
            continue;
        };
        let Some(reference) = boxes.get(first) else {
            continue;
        };
        for id in ids.iter().skip(1) {
            let Some(bounds) = boxes.get(id) else {
                continue;
            };
            let center = aligned_center(*reference, *bounds, alignment, padding);
            let distance = if horizontal {
                center.x - reference.center().x
            } else {
                center.y - reference.center().y
            };
            append_edge(&mut graph, first, id, distance);
            append_edge(&mut graph, id, first, -distance);
        }
    }
    graph
}

fn axis_alignment(name: &str, horizontal: bool) -> Option<&str> {
    if matches!(
        (name, horizontal),
        ("inset_left" | "inset_right", true) | ("inset_top" | "inset_bottom", false)
    ) {
        return Some(name);
    }
    if name == "center" {
        return Some(if horizontal {
            "align_center_x"
        } else {
            "align_center_y"
        });
    }
    if name == "row" && !horizontal {
        return Some("align_center_y");
    }
    if name == "column" && horizontal {
        return Some("align_center_x");
    }
    let names = if horizontal {
        ["align_center_x", "align_left", "align_right"]
    } else {
        ["align_center_y", "align_top", "align_bottom"]
    };
    names.contains(&name).then_some(name)
}

fn alignment_padding(
    prepared: &PreparedEvaluation<'_>,
    source: &super::SourceRelation<'_>,
    name: &str,
) -> f64 {
    if name.starts_with("inset_") && !source.padding_none {
        prepared.inputs().metrics.padding
    } else {
        0.0
    }
}

fn inset_containers(sources: &[super::SourceRelation<'_>]) -> BTreeSet<String> {
    sources
        .iter()
        .filter_map(|source| {
            let Expr::Call { name, args, .. } = source.expression else {
                return None;
            };
            let [_, Expr::Ref(container)] = args.as_slice() else {
                return None;
            };
            name.name
                .starts_with("inset_")
                .then(|| container.name.clone())
        })
        .collect()
}

fn shared_positions(
    graph: &OrderGraph,
    boxes: &BTreeMap<String, crate::BBox>,
    movable: &BTreeSet<String>,
    preferred: &BTreeSet<String>,
    horizontal: bool,
) -> BTreeMap<String, f64> {
    let mut positions = BTreeMap::new();
    for start in graph.keys() {
        if positions.contains_key(start) {
            continue;
        }
        let offsets = component_offsets(graph, start);
        let anchor = offsets
            .keys()
            .find(|id| !movable.contains(*id))
            .or_else(|| offsets.keys().find(|id| preferred.contains(*id)))
            .unwrap_or(start);
        let Some(bounds) = boxes.get(anchor) else {
            continue;
        };
        let center = bounds.center();
        let coordinate = if horizontal { center.x } else { center.y };
        let origin = coordinate - offsets[anchor];
        positions.extend(offsets.into_iter().map(|(id, delta)| (id, origin + delta)));
    }
    positions
}

fn component_offsets(graph: &OrderGraph, start: &str) -> BTreeMap<String, f64> {
    let mut offsets = BTreeMap::from([(start.to_owned(), 0.0)]);
    let mut ready = BTreeSet::from([start.to_owned()]);
    while let Some(id) = ready.pop_first() {
        for (target, distance) in &graph[&id] {
            if !offsets.contains_key(target) {
                offsets.insert(target.clone(), offsets[&id] + distance);
                ready.insert(target.clone());
            }
        }
    }
    offsets
}

fn move_center(
    system: &mut UnknownSystem,
    name: &str,
    center: crate::Point,
    boxes: &mut BTreeMap<String, crate::BBox>,
) -> bool {
    let Some(object) = system.objects.iter().find(|o| {
        o.id == name
            && o.count == 2
            && matches!(
                o.class,
                UnknownClass::Geometric | UnknownClass::LabelPlacement
            )
    }) else {
        return false;
    };
    let Some(bounds) = boxes.get_mut(name) else {
        return false;
    };
    let before = bounds.center();
    if before == center {
        return false;
    }
    let offset = center - before;
    bounds.min = bounds.min + offset;
    bounds.max = bounds.max + offset;
    system.hints[object.start] = center.x;
    system.hints[object.start + 1] = center.y;
    true
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    fn bounds(x: f64, y: f64, width: f64, height: f64) -> crate::BBox {
        crate::BBox {
            min: crate::Point::new(x - width / 2.0, y - height / 2.0),
            max: crate::Point::new(x + width / 2.0, y + height / 2.0),
        }
    }

    pub(super) fn seeded(
        source: &str,
    ) -> (BTreeMap<String, crate::BBox>, BTreeMap<String, crate::BBox>) {
        let parsed = locus_syntax::parse(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let expanded = crate::program::expand_program(&parsed.file).expect("expand seed source");
        let prepared = PreparedEvaluation::with_inputs(
            &expanded.file,
            &crate::inputs::EvaluationInputs::default(),
            &expanded.collections,
        )
        .expect("prepare seed source");
        let trial = prepared.evaluate_seed(1.0).expect("ring seeds");
        let before = trial
            .objects()
            .iter()
            .filter_map(|(id, object)| object.bbox().map(|b| (id.name.clone(), b)))
            .collect();
        let mut system = UnknownSystem::from_seed(&prepared, &trial).expect("seed registry");
        initialize(&prepared, &mut system).expect("relation-aware seeds");
        let trial = prepared
            .evaluate_trial(Some(&system.hints))
            .expect("initialized trial");
        let after = trial
            .objects()
            .iter()
            .filter_map(|(id, object)| object.bbox().map(|b| (id.name.clone(), b)))
            .collect();
        (before, after)
    }

    #[test]
    fn longest_path_uses_all_predecessors_independent_of_edge_order() {
        let boxes = BTreeMap::from([
            ("a".to_owned(), bounds(0.0, 0.0, 1.0, 1.0)),
            ("b".to_owned(), bounds(0.0, 0.0, 1.0, 1.0)),
            ("c".to_owned(), bounds(0.0, 0.0, 1.0, 1.0)),
        ]);
        let mut graph = OrderGraph::new();
        append_edge(&mut graph, "a", "c", 2.0);
        append_edge(&mut graph, "b", "c", 5.0);
        append_edge(&mut graph, "a", "b", 3.0);
        let positions = longest_paths(&graph, &boxes, &boxes.keys().cloned().collect(), true);
        assert_eq!(positions["b"], 3.0);
        assert_eq!(positions["c"], 8.0);
    }

    #[test]
    fn contradictory_cycle_and_descendants_do_not_accumulate_coordinates() {
        let boxes = BTreeMap::from([
            ("a".to_owned(), bounds(0.0, 0.0, 1.0, 1.0)),
            ("b".to_owned(), bounds(1.0, 0.0, 1.0, 1.0)),
            ("c".to_owned(), bounds(2.0, 0.0, 1.0, 1.0)),
        ]);
        let mut graph = OrderGraph::new();
        append_edge(&mut graph, "a", "b", 2.0);
        append_edge(&mut graph, "b", "a", 2.0);
        append_edge(&mut graph, "b", "c", 2.0);
        assert!(longest_paths(&graph, &boxes, &boxes.keys().cloned().collect(), true).is_empty());
    }

    #[test]
    fn fixed_anchor_keeps_its_coordinate_when_it_has_an_incoming_order() {
        let boxes = BTreeMap::from([
            ("a".to_owned(), bounds(10.0, 0.0, 1.0, 1.0)),
            ("fixed".to_owned(), bounds(1.0, 0.0, 1.0, 1.0)),
            ("c".to_owned(), bounds(0.0, 0.0, 1.0, 1.0)),
        ]);
        let mut graph = OrderGraph::new();
        append_edge(&mut graph, "a", "fixed", 2.0);
        append_edge(&mut graph, "fixed", "c", 3.0);
        let movable = BTreeSet::from(["a".to_owned(), "c".to_owned()]);
        let positions = longest_paths(&graph, &boxes, &movable, true);
        assert_eq!(positions["fixed"], 1.0);
        assert_eq!(positions["c"], 4.0);
    }

    #[test]
    fn reversed_source_order_chains_share_coordinates_and_full_bound_gaps() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect a(2,1)\nrect b(3,2)\nrect c(1,1)\nrel row(b,c)\nrel row(a,b)\n",
        );
        assert!(boxes["b"].min.x - boxes["a"].max.x >= 0.25 - 1.0e-12);
        assert!(boxes["c"].min.x - boxes["b"].max.x >= 0.25 - 1.0e-12);
        assert_eq!(boxes["a"].center().y, boxes["b"].center().y);
        assert_eq!(boxes["b"].center().y, boxes["c"].center().y);
    }

    #[test]
    fn expanded_column_members_run_downward_with_full_bound_gaps() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect a(2,1)\nrect b(3,2)\nrect c(1,1)\ngroup stack(a,b,c)\nrel column(stack)\n",
        );
        assert!(boxes["a"].min.y - boxes["b"].max.y >= 0.25 - 1.0e-12);
        assert!(boxes["b"].min.y - boxes["c"].max.y >= 0.25 - 1.0e-12);
        assert_eq!(boxes["a"].center().x, boxes["c"].center().x);
    }

    #[test]
    fn contain_grows_after_row_layering_of_a_collection() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect a(2,1)\nrect b(3,2)\ngroup contents(a,b)\nrect panel()\nrel row(contents)\nrel contain(panel,contents)\n",
        );
        assert!(boxes["panel"].min.x <= boxes["a"].min.x - 0.25 + 1.0e-12);
        assert!(boxes["panel"].max.x >= boxes["b"].max.x + 0.25 - 1.0e-12);
        assert!(boxes["panel"].height() >= 2.5 - 1.0e-12);
    }

    #[test]
    fn inset_seeding_honors_none_padding_and_member_sizes() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect panel(6,4)\nrect content(2,1)\nrect extra(1,1)\nrel gap_x(panel,extra)\nrel inset_left(content,panel) { padding: none }\nrel inset_top(content,panel)\n",
        );
        assert!((boxes["content"].min.x - boxes["panel"].min.x).abs() < 1.0e-12);
        assert!((boxes["panel"].max.y - boxes["content"].max.y - 0.25).abs() < 1.0e-12);
    }

    #[test]
    fn exact_insets_survive_automatic_container_growth_on_every_side() {
        for (horizontal, vertical) in [("left", "top"), ("right", "bottom")] {
            let source = format!(
                "locus 0 beta 6\nrect panel()\nrect tab(2,1)\nrect content(8,3)\nrel inset_{horizontal}(tab,panel) {{ padding: none }}\nrel inset_{vertical}(tab,panel) {{ padding: none }}\nrel contain(panel,content)\n"
            );
            let (_, b) = seeded(&source);
            let (tab, panel) = (b["tab"], b["panel"]);
            let x = if horizontal == "left" {
                tab.min.x - panel.min.x
            } else {
                tab.max.x - panel.max.x
            };
            let y = if vertical == "top" {
                tab.max.y - panel.max.y
            } else {
                tab.min.y - panel.min.y
            };
            assert!(
                x.abs() < 1e-12 && y.abs() < 1e-12,
                "{horizontal}/{vertical}: {x},{y}"
            );
        }
    }

    #[test]
    fn pure_geometry_keeps_ring_seeds_exactly() {
        let (before, after) =
            seeded("locus 0 beta 6\npoint a()\npoint b()\nrect c(2,1)\nrel dist(a,b) == 2\n");
        assert_eq!(before, after);
    }

    #[test]
    fn composed_instances_remain_whole_column_items() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nfn pair() {\nrect left(2,1)\nrect right(3,1)\nrel row(left,right)\n}\npair upper()\npair lower()\nrel column(upper,lower)\n",
        );
        assert!(boxes["upper.right"].min.x - boxes["upper.left"].max.x >= 0.25 - 1.0e-12);
        assert!(boxes["lower.right"].min.x - boxes["lower.left"].max.x >= 0.25 - 1.0e-12);
        let upper_bottom = boxes["upper.left"].min.y.min(boxes["upper.right"].min.y);
        let lower_top = boxes["lower.left"].max.y.max(boxes["lower.right"].max.y);
        assert!(upper_bottom - lower_top >= 0.25 - 1.0e-12);
        assert_eq!(
            boxes["upper.left"].center().y,
            boxes["upper.right"].center().y
        );
        assert_eq!(
            boxes["lower.left"].center().y,
            boxes["lower.right"].center().y
        );
    }

    #[test]
    fn gap_collection_uses_union_bounds_without_arranging_its_members() {
        let (before, boxes) = seeded(
            "locus 0 beta 6\nrect a(2,1)\nrect b(3,2)\nrect c(1,1)\ngroup combined(a,b)\nrel gap_y(combined,c)\n",
        );
        let group_bottom = boxes["a"].min.y.min(boxes["b"].min.y);
        assert!(group_bottom - boxes["c"].max.y >= 0.25 - 1.0e-12);
        let initial_relative = before["a"].center() - before["b"].center();
        let final_relative = boxes["a"].center() - boxes["b"].center();
        assert!((initial_relative.x - final_relative.x).abs() < 1.0e-12);
        assert!((initial_relative.y - final_relative.y).abs() < 1.0e-12);
    }

    #[test]
    fn row_deduplicates_members_across_group_and_direct_arguments() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect a(2,1)\nrect b(3,2)\ngroup combined(a,b)\nrel row(combined,a,b)\n",
        );
        assert!(boxes["b"].min.x - boxes["a"].max.x >= 0.25 - 1.0e-12);
        assert_eq!(boxes["a"].center().y, boxes["b"].center().y);
    }

    #[test]
    fn ordered_points_seed_on_the_interior_of_an_anchor_aligned_guide() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect header(2,1)\npoint finish()\nrel align_center_x(header.s,finish)\nsegment guide(header.s,finish)\npoint first()\npoint second()\nrel on(first,guide)\nrel on(second,guide)\nrel gap_y(header,first,second,finish)\n",
        );
        assert!((boxes["first"].center().x - boxes["header"].center().x).abs() < 1e-12);
        assert!((boxes["second"].center().x - boxes["header"].center().x).abs() < 1e-12);
        assert!(boxes["first"].center().y < boxes["header"].min.y);
        assert!(boxes["second"].center().y > boxes["finish"].center().y);
        assert!(boxes["first"].center().y - boxes["second"].center().y >= 0.25 - 1e-12);
    }

    #[test]
    fn finite_edge_incidence_seeds_use_the_current_container_bounds() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect panel(6,4)\nrect extra(1,1)\nrel gap_x(panel,extra)\npoint left()\npoint right()\nrel on(left,edge(panel,W))\nrel on(right,edge(panel,E))\nrel align_center_y(left,right)\n",
        );
        assert!((boxes["left"].center().x - boxes["panel"].min.x).abs() < 1e-12);
        assert!((boxes["right"].center().x - boxes["panel"].max.x).abs() < 1e-12);
        assert!(boxes["left"].center().y < boxes["panel"].max.y);
        assert!(boxes["left"].center().y > boxes["panel"].min.y);
    }

    #[test]
    fn property_anchor_gap_seeds_a_guide_in_the_required_direction() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect header(2,1)\npoint finish()\nrel align_center_x(header.s,finish)\nrel gap_y(header.s,finish)\nsegment guide(header.s,finish)\npoint event()\nrel on(event,guide)\n",
        );
        assert!(boxes["finish"].center().y <= boxes["header"].min.y - 0.25);
        assert!(boxes["event"].center().y < boxes["header"].min.y);
        assert!(boxes["event"].center().y > boxes["finish"].center().y);
    }

    #[test]
    fn hidden_endpoint_instances_translate_with_fresh_visible_route_bounds() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nfn wire() {\npoint tail() {mark:none}\npoint tip() {mark:none}\nrel align_center_y(tail,tip)\nrel gap_x(tail,tip)\narrow path(tail,tip)\n}\nwire first()\nwire second()\ngroup wires(first,second)\nrect panel(2,1)\nrel column(first,second)\nrel gap_y(panel,wires)\n",
        );
        let top = boxes["first.path"].max.y.max(boxes["second.path"].max.y);
        assert!(boxes["panel"].min.y - top >= 0.25 - 1e-12);
        assert!(boxes["first.path"].min.y - boxes["second.path"].max.y >= 0.25 - 1e-12);
        assert!((boxes["first.path"].center().y - boxes["first.tail"].center().y).abs() < 1e-12);
    }

    #[test]
    fn containment_preserves_ordered_container_axes_and_translates_content() {
        let (_, boxes) = seeded(
            "locus 0 beta 6\nrect header(2,1)\nrect a(1,1)\nrect b(1,1)\ngroup content(a,b)\nrect frame()\nrel column(a,b)\nrel contain(frame,content)\nrel gap_y(header,frame)\n",
        );
        assert!(boxes["header"].min.y - boxes["frame"].max.y >= 0.25 - 1e-12);
        assert!(boxes["frame"].max.y - boxes["a"].max.y >= 0.25 - 1e-12);
        assert!(boxes["b"].min.y - boxes["frame"].min.y >= 0.25 - 1e-12);
    }

    #[test]
    fn ordered_alias_operands_move_their_geometric_owner() {
        let (_, boxes) =
            seeded("locus 0 beta 6\nrect a(2,1)\nrect b(3,2)\nrect alias = a\nrel row(b,alias)\n");
        assert!(boxes["a"].min.x - boxes["b"].max.x >= 0.25 - 1e-12);
        assert_eq!(boxes["alias"], boxes["a"]);
    }
}
