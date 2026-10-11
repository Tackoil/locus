//! Seed freely placed guide spacing from actual attached-caption measurements.

use super::{BTreeMap, BTreeSet, EvalError, Expr, PreparedEvaluation, UnknownClass, UnknownSystem};
use crate::{AnnotationSpec, BBox, ResolvedObject, Segment, Vec2};
use locus_syntax::{Constructor, Statement};

type Boxes = BTreeMap<String, BBox>;

pub(super) fn seed(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    boxes: &mut Boxes,
) -> Result<(), EvalError> {
    let aliases = crate::program::alias_identities(prepared.file());
    for source in sources {
        let Expr::Call { name, args, .. } = source.expression else {
            continue;
        };
        if name.name != "avoid" || args.len() < 3 {
            continue;
        }
        let subjects = crate::program::avoidance_operands(
            prepared.collections(),
            &aliases,
            std::slice::from_ref(&args[0]),
        );
        let obstacles =
            crate::program::avoidance_operands(prepared.collections(), &aliases, &args[1..]);
        seed_avoidance(prepared, system, sources, boxes, &subjects, &obstacles)?;
    }
    Ok(())
}

fn seed_avoidance(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    boxes: &mut Boxes,
    subjects: &[Expr],
    obstacles: &[Expr],
) -> Result<(), EvalError> {
    for expression in subjects {
        let trial = prepared.evaluate_trial(Some(&system.hints))?;
        let Expr::Ref(id) = expression else {
            continue;
        };
        let Some(ResolvedObject::Annotation(AnnotationSpec::Label {
            path: Some(label),
            bounds: Some(bounds),
            ..
        })) = find_object(&trial, &id.name)
        else {
            continue;
        };
        let Some(carriers) = endpoint_carriers(prepared, sources, &label.path) else {
            continue;
        };
        if !carriers.iter().all(|name| {
            obstacles
                .iter()
                .any(|expr| matches!(expr, Expr::Ref(id) if id.name == *name))
        }) {
            continue;
        }
        let Some((normal, gap)) = carrier_spacing(&trial, &carriers) else {
            continue;
        };
        let required = required_spacing(prepared, label, *bounds, normal);
        widen(
            prepared,
            system,
            sources,
            boxes,
            &carriers,
            normal,
            required - gap,
        );
    }
    Ok(())
}

fn find_object<'a>(
    trial: &'a crate::eval::TrialEvaluation,
    name: &str,
) -> Option<&'a ResolvedObject> {
    trial
        .objects()
        .iter()
        .find(|(id, _)| id.name == name)
        .map(|(_, object)| object)
}

fn endpoint_carriers(
    prepared: &PreparedEvaluation<'_>,
    sources: &[crate::solve::SourceRelation<'_>],
    path: &str,
) -> Option<[String; 2]> {
    let Constructor::Call { args, .. } = &declaration(prepared, path)?.ctor else {
        return None;
    };
    let [Expr::Ref(from), Expr::Ref(to)] = args.as_slice() else {
        return None;
    };
    let aliases = crate::program::alias_identities(prepared.file());
    let first = on_carrier(sources, &aliases, &from.name)?;
    let second = on_carrier(sources, &aliases, &to.name)?;
    (first != second).then_some([first, second])
}

fn on_carrier(
    sources: &[crate::solve::SourceRelation<'_>],
    aliases: &BTreeMap<String, String>,
    point: &str,
) -> Option<String> {
    let expected = aliases.get(point).map_or(point, String::as_str);
    sources.iter().find_map(|source| {
        let Expr::Call { name, args, .. } = source.expression else {
            return None;
        };
        let [Expr::Ref(subject), Expr::Ref(target)] = args.as_slice() else {
            return None;
        };
        let identity = aliases.get(&subject.name).unwrap_or(&subject.name);
        (name.name == "on" && identity == expected)
            .then(|| aliases.get(&target.name).unwrap_or(&target.name).clone())
    })
}

fn carrier_spacing(
    trial: &crate::eval::TrialEvaluation,
    carriers: &[String; 2],
) -> Option<(Vec2, f64)> {
    let ResolvedObject::Segment(first) = find_object(trial, &carriers[0])? else {
        return None;
    };
    let ResolvedObject::Segment(second) = find_object(trial, &carriers[1])? else {
        return None;
    };
    let normal = guide_normal(*first, *second)?;
    let gap = (second.midpoint() - first.midpoint()).dot(normal);
    Some((normal * if gap < 0.0 { -1.0 } else { 1.0 }, gap.abs()))
}

fn guide_normal(first: Segment, second: Segment) -> Option<Vec2> {
    let a = first.direction();
    let b = second.direction();
    let lengths = a.norm() * b.norm();
    if lengths == 0.0 || a.cross(b).abs() > lengths * 1e-10 {
        return None;
    }
    Some(a.perpendicular() / a.norm())
}

fn required_spacing(
    prepared: &PreparedEvaluation<'_>,
    label: &crate::scene::PathLabel,
    bounds: BBox,
    normal: Vec2,
) -> f64 {
    let half_extent = (normal.x.abs() * bounds.width()).midpoint(normal.y.abs() * bounds.height());
    let metrics = prepared.inputs().metrics;
    let reach = half_extent + metrics.clearance + metrics.stroke_width / 2.0;
    let offset = page_offset(label.side, bounds, label.offset).dot(normal);
    ((reach - offset) / label.parameter.max(1e-6))
        .max((reach + offset) / (1.0 - label.parameter).max(1e-6))
}

fn page_offset(side: crate::geom::route::RouteSide, bounds: BBox, gap: f64) -> Vec2 {
    use crate::geom::route::RouteSide;
    match side {
        RouteSide::Right => Vec2::new(bounds.width() / 2.0 + gap, 0.0),
        RouteSide::Left => Vec2::new(-bounds.width() / 2.0 - gap, 0.0),
        RouteSide::Above => Vec2::new(0.0, bounds.height() / 2.0 + gap),
        RouteSide::Below => Vec2::new(0.0, -bounds.height() / 2.0 - gap),
    }
}

fn widen(
    prepared: &PreparedEvaluation<'_>,
    system: &mut UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    boxes: &mut Boxes,
    carriers: &[String; 2],
    normal: Vec2,
    extra: f64,
) {
    if extra <= 0.0 || !extra.is_finite() {
        return;
    }
    let first = carrier_owners(prepared, system, sources, &carriers[0], normal);
    let second = carrier_owners(prepared, system, sources, &carriers[1], normal);
    let (preferred, other, direction) = if normal.x > 0.0 || (normal.x == 0.0 && normal.y > 0.0) {
        (second, first, normal)
    } else {
        (first, second, normal * -1.0)
    };
    let (owners, direction) = if preferred.is_disjoint(&other) && !preferred.is_empty() {
        (preferred, direction)
    } else if other.is_disjoint(&preferred) && !other.is_empty() {
        (other, direction * -1.0)
    } else {
        return;
    };
    for name in owners {
        let Some(bounds) = boxes.get(&name).copied() else {
            continue;
        };
        super::move_center(system, &name, bounds.center() + direction * extra, boxes);
    }
}

fn carrier_owners(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    sources: &[crate::solve::SourceRelation<'_>],
    carrier: &str,
    normal: Vec2,
) -> BTreeSet<String> {
    let mut owners = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_owners(prepared, system, carrier, &mut visited, &mut owners);
    for _ in 0..sources.len() {
        let before = owners.len();
        for source in sources {
            add_partners(prepared, system, source.expression, normal, &mut owners);
        }
        if owners.len() == before {
            break;
        }
    }
    owners
}

fn add_partners(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    expression: &Expr,
    normal: Vec2,
    owners: &mut BTreeSet<String>,
) {
    let Expr::Call { name, args, .. } = expression else {
        return;
    };
    if !partner_relation(&name.name, normal)
        || args.iter().any(
            |arg| matches!(arg, Expr::Ref(id) if prepared.collections().contains_key(&id.name)),
        )
    {
        return;
    }
    let mut partners = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for arg in args {
        expression_owners(prepared, system, arg, &mut visited, &mut partners);
    }
    if !partners.is_disjoint(owners) {
        owners.extend(partners);
    }
}

fn partner_relation(name: &str, normal: Vec2) -> bool {
    name == "center"
        || (normal.x != 0.0 && matches!(name, "align_center_x" | "align_left" | "align_right"))
        || (normal.y != 0.0 && matches!(name, "align_center_y" | "align_top" | "align_bottom"))
}

fn collect_owners(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    name: &str,
    visited: &mut BTreeSet<String>,
    owners: &mut BTreeSet<String>,
) {
    if !visited.insert(name.to_owned()) {
        return;
    }
    if system.objects.iter().any(|object| {
        object.id == name
            && object.count == 2
            && matches!(
                object.class,
                UnknownClass::Geometric | UnknownClass::LabelPlacement
            )
    }) {
        owners.insert(name.to_owned());
        return;
    }
    let Some(op) = declaration(prepared, name) else {
        return;
    };
    match &op.ctor {
        Constructor::Call { args, .. } => {
            for arg in args {
                expression_owners(prepared, system, arg, visited, owners);
            }
        }
        Constructor::Expr { expr, .. } => {
            expression_owners(prepared, system, expr, visited, owners);
        }
        Constructor::FreeNum { .. } => {}
    }
}

fn expression_owners(
    prepared: &PreparedEvaluation<'_>,
    system: &UnknownSystem,
    expression: &Expr,
    visited: &mut BTreeSet<String>,
    owners: &mut BTreeSet<String>,
) {
    match expression {
        Expr::Ref(id) => collect_owners(prepared, system, &id.name, visited, owners),
        Expr::Property { base, .. } | Expr::Group { expr: base, .. } => {
            expression_owners(prepared, system, base, visited, owners);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                expression_owners(prepared, system, arg, visited, owners);
            }
        }
        _ => {}
    }
}

fn declaration<'a>(
    prepared: &'a PreparedEvaluation<'_>,
    name: &str,
) -> Option<&'a locus_syntax::OpStmt> {
    prepared
        .file()
        .statements
        .iter()
        .find_map(|statement| match statement {
            Statement::Op(op) if op.id.name == name => Some(op),
            _ => None,
        })
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "controlled measured-layout seed fixtures"
)]
mod tests {
    use super::*;

    const GUIDES: &str = "locus 0 beta 6\nrect first(2,1)\nrect second(2,1)\nlabel first_title(\"Left\")\nlabel second_title(\"Right\")\nrel center(first_title,first)\nrel center(second_title,second)\nrel row(first,second)\npoint left_finish() { mark: none }\npoint right_finish() { mark: none }\nrel align_center_x(first.s,left_finish)\nrel align_center_x(second.s,right_finish)\nrel gap_y(first.s,left_finish)\nrel gap_y(second.s,right_finish)\nsegment left_guide(first.s,left_finish)\nsegment right_guide(second.s,right_finish)\npoint tail() { mark: none }\npoint tip() { mark: none }\nrel on(tail,left_guide)\nrel on(tip,right_guide)\nrel align_center_y(tail,tip)\narrow path(tail,tip)\nlabel caption(path,\"Long measured guide caption\") { near: middle, side: above }\nrel avoid(caption,left_guide,right_guide)\nrel align_center_y(left_finish,right_finish)\nrel gap_y(first,caption,left_finish)\n";

    fn inputs(width: f64) -> crate::EvaluationInputs {
        let mut inputs = crate::EvaluationInputs::default();
        for (name, measured_width) in [
            ("first_title", 1.0),
            ("second_title", 1.0),
            ("caption", width),
        ] {
            inputs.measured.insert(
                name.to_owned(),
                crate::MeasuredText {
                    width: measured_width,
                    height: 0.3,
                    role: crate::MeasuredRole::Normal,
                },
            );
        }
        inputs
    }

    fn seeded(source: &str, width: f64) -> BTreeMap<String, BBox> {
        seeded_with_inputs(source, &inputs(width))
    }

    fn seeded_with_inputs(
        source: &str,
        inputs: &crate::EvaluationInputs,
    ) -> BTreeMap<String, BBox> {
        let parsed = locus_syntax::parse(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let expanded = crate::program::expand_program(&parsed.file).expect("expanded guides");
        let prepared =
            PreparedEvaluation::with_inputs(&expanded.file, inputs, &expanded.collections)
                .expect("prepared guides");
        let trial = prepared.evaluate_seed(1.0).expect("guide seed");
        let mut system = UnknownSystem::from_seed(&prepared, &trial).expect("guide registry");
        super::super::initialize(&prepared, &mut system).expect("measured guide initialization");
        prepared
            .evaluate_trial(Some(&system.hints))
            .expect("initialized guide geometry")
            .objects()
            .iter()
            .filter_map(|(id, object)| object.bbox().map(|bounds| (id.name.clone(), bounds)))
            .collect()
    }

    #[test]
    fn long_attached_caption_widens_guides_and_keeps_centered_partners_together() {
        let bounds = seeded(GUIDES, 7.0);
        let gap = bounds["right_guide"].center().x - bounds["left_guide"].center().x;
        assert!(gap >= 7.34 - 1e-12, "seeded guide distance {gap}");
        for name in ["first", "second"] {
            assert!(
                (bounds[name].center() - bounds[&format!("{name}_title")].center()).norm() < 1e-12
            );
        }
        assert!((bounds["second"].center().x - bounds["right_finish"].center().x).abs() < 1e-12);
    }

    #[test]
    fn wide_caption_between_finite_guides_is_feasible_after_initialization() {
        let parsed = locus_syntax::parse(GUIDES);
        let scene = crate::Scene::with_inputs(parsed.file, inputs(7.0))
            .expect("wide measured caption solves");
        let resolved = scene.resolved().expect("resolved guides");
        let object = |name: &str| {
            resolved
                .objects
                .iter()
                .find(|(id, _)| id.name == name)
                .map(|(_, object)| object)
                .expect("fixture object")
        };
        for guide in ["left_guide", "right_guide"] {
            let separation = crate::geom::sdf::object_separation(
                object("caption"),
                object(guide),
                scene.inputs.metrics.stroke_width / 2.0,
                None,
            )
            .expect("label-guide separation");
            assert!(separation >= scene.inputs.metrics.clearance - 1e-10);
        }
    }
    #[test]
    fn tall_caption_widens_horizontal_guides_on_the_vertical_axis() {
        let source = GUIDES
            .replace("(2,1)", "(1,2)")
            .replace("row(first,second)", "column(first,second)")
            .replace("first.s", "first.e")
            .replace("second.s", "second.e")
            .replace("align_center_x", "TEMP_AXIS")
            .replace("align_center_y", "align_center_x")
            .replace("TEMP_AXIS", "align_center_y")
            .replace("gap_y", "gap_x")
            .replace("side: above", "side: right");
        let mut measurements = inputs(0.3);
        measurements
            .measured
            .get_mut("caption")
            .expect("caption measurement")
            .height = 7.0;
        let bounds = seeded_with_inputs(&source, &measurements);
        let gap = bounds["left_guide"].center().y - bounds["right_guide"].center().y;
        assert!(gap >= 7.34 - 1e-12, "seeded vertical guide distance {gap}");
        assert!((bounds["first"].center() - bounds["first_title"].center()).norm() < 1e-12);
    }

    #[test]
    fn attached_caption_without_guide_avoidance_does_not_widen_its_partners() {
        let source = GUIDES.replace("rel avoid(caption,left_guide,right_guide)\n", "");
        let bounds = seeded(&source, 7.0);
        let gap = bounds["right_guide"].center().x - bounds["left_guide"].center().x;
        let baseline = seeded(&source, 0.3);
        let original_gap = baseline["right_guide"].center().x - baseline["left_guide"].center().x;
        assert!(
            (gap - original_gap).abs() < 1e-12,
            "unrequested guide spacing changed to {gap}"
        );
    }

    #[test]
    fn rigidly_related_guides_keep_their_authored_spacing() {
        let source = GUIDES
            .replace("rect first(2,1)", "point origin()\nrect first(origin,2,1)")
            .replace(
                "rect second(2,1)",
                "point other = on_circle(circle(origin,2.25),0deg)\nrect second(other,2,1)",
            )
            .replace(
                "point left_finish() { mark: none }",
                "point left_finish = on_circle(circle(first.s,10),270deg) { mark: none }",
            )
            .replace(
                "point right_finish() { mark: none }",
                "point right_finish = on_circle(circle(second.s,10),270deg) { mark: none }",
            );
        let bounds = seeded(&source, 7.0);
        let gap = bounds["right_guide"].center().x - bounds["left_guide"].center().x;
        assert!(
            (gap - 2.25).abs() < 1e-12,
            "rigid guide spacing changed to {gap}"
        );
    }
    #[test]
    fn aliased_guide_operands_use_the_same_translation_owners() {
        let source = GUIDES.replace(
            "segment right_guide(second.s,right_finish)",
            "segment right_original(second.s,right_finish)\nsegment right_guide = right_original",
        );
        let bounds = seeded(&source, 7.0);
        let gap = bounds["right_guide"].center().x - bounds["left_guide"].center().x;
        assert!(gap >= 7.34 - 1e-12, "aliased guide distance {gap}");
    }
}
