//! Registry-first preparation and solve-time dependency evaluation.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use locus_syntax::{Constructor, File, OpStmt, Statement};

use super::expr::DimensionMap;
use super::{
    DependencyGraph, EvalError, FileEvaluator, TrialEvaluation, UnknownCursor,
    dependency_eval_error, infer_dimensions, is_placed_op,
};

#[derive(Clone)]
pub(crate) struct PreparedEvaluation<'a> {
    file: &'a File,
    route_choices: BTreeMap<String, crate::geom::route::RouteChoice>,
    dimensions: DimensionMap,
    registry: Vec<&'a OpStmt>,
    unknown_ranges: BTreeMap<String, Range<usize>>,
    closure: BTreeSet<String>,
    inputs: crate::inputs::EvaluationInputs,
    collections: BTreeMap<String, locus_syntax::Collection>,
}

impl<'a> PreparedEvaluation<'a> {
    #[cfg(test)]
    pub(crate) fn new(file: &'a File) -> Result<Self, EvalError> {
        Self::with_inputs(
            file,
            &crate::inputs::EvaluationInputs::default(),
            &BTreeMap::new(),
        )
    }

    pub(crate) fn with_inputs(
        file: &'a File,
        inputs: &crate::inputs::EvaluationInputs,
        collections: &BTreeMap<String, locus_syntax::Collection>,
    ) -> Result<Self, EvalError> {
        let _prepare = crate::metrics::span("prepare_evaluation");
        inputs.validate()?;
        let graph = crate::metrics::measure("dependencies", || {
            DependencyGraph::extract_with_collections(file, collections)
        })
        .map_err(dependency_eval_error)?;
        let dimensions = crate::metrics::measure("infer_dimensions", || infer_dimensions(file))?;
        let registry = file
            .statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::Op(op) if owns_unknowns(op, &dimensions, file) => Some(op),
                _ => None,
            })
            .collect::<Vec<_>>();
        let unknown_ranges = unknown_ranges(&registry);
        let closure = graph.solver_closure(registry.iter().map(|op| op.id.name.as_str()));
        Ok(Self {
            route_choices: inputs.choices.routes.clone(),
            file,
            dimensions,
            registry,
            unknown_ranges,
            closure,
            inputs: inputs.clone(),
            collections: collections.clone(),
        })
    }

    pub(crate) fn with_route_choices(
        &self,
        choices: BTreeMap<String, crate::geom::route::RouteChoice>,
    ) -> Self {
        let mut prepared = self.clone();
        prepared.route_choices = choices;
        prepared
    }

    pub(crate) fn with_layout_choices(&self, choices: crate::inputs::EvaluationChoices) -> Self {
        let mut prepared = self.with_route_choices(choices.routes.clone());
        prepared.inputs.choices = choices;
        prepared
    }

    pub(crate) fn route_choices(&self) -> &BTreeMap<String, crate::geom::route::RouteChoice> {
        &self.route_choices
    }

    pub(crate) const fn file(&self) -> &'a File {
        self.file
    }

    pub(crate) const fn dimensions(&self) -> &DimensionMap {
        &self.dimensions
    }

    pub(crate) const fn inputs(&self) -> &crate::inputs::EvaluationInputs {
        &self.inputs
    }

    pub(crate) fn collections(&self) -> &BTreeMap<String, locus_syntax::Collection> {
        &self.collections
    }

    pub(crate) fn registry(&self) -> &[&'a OpStmt] {
        &self.registry
    }

    pub(crate) fn solver_ops(&self) -> impl Iterator<Item = &'a OpStmt> + '_ {
        self.file
            .statements
            .iter()
            .filter_map(|statement| match statement {
                Statement::Op(op) if self.closure.contains(&op.id.name) => Some(op),
                _ => None,
            })
    }

    pub(crate) fn unknown_range(&self, id: &str) -> Option<&Range<usize>> {
        self.unknown_ranges.get(id)
    }

    pub(crate) fn evaluate_seed(&self, placement_scale: f64) -> Result<TrialEvaluation, EvalError> {
        self.evaluate_ops(None, false, placement_scale)
    }

    pub(crate) fn evaluate_trial(
        &self,
        unknowns: Option<&[f64]>,
    ) -> Result<TrialEvaluation, EvalError> {
        self.evaluate_ops(unknowns, false, 1.0)
    }

    pub(crate) fn evaluate_final(&self, unknowns: &[f64]) -> Result<TrialEvaluation, EvalError> {
        self.evaluate_ops(Some(unknowns), true, 1.0)
    }

    fn evaluate_ops(
        &self,
        unknowns: Option<&[f64]>,
        include_deferred: bool,
        placement_scale: f64,
    ) -> Result<TrialEvaluation, EvalError> {
        let mut evaluator = FileEvaluator::with_inputs(
            self.dimensions.clone(),
            self.inputs.clone(),
            self.collections.clone(),
        );
        evaluator.route_choices = self.route_choices.clone();
        evaluator.aliases = crate::program::alias_identities(self.file);
        evaluator.env.set_aliases(&evaluator.aliases);
        evaluator.center_partners = super::automatic::center_partners(self.file);
        evaluator.content_extents =
            super::automatic::content_extents(self.file, &self.inputs, &self.collections);
        evaluator.placement_seeds = self
            .registry
            .iter()
            .filter(|op| is_placed_op(op, &self.dimensions))
            .enumerate()
            .map(|(index, op)| (op.id.name.clone(), super::deterministic_placement(index)))
            .collect();
        evaluator.placement_scale = placement_scale;
        let mut cursor = UnknownCursor::new(unknowns);
        for statement in &self.file.statements {
            let Statement::Op(op) = statement else {
                continue;
            };
            if !include_deferred && !self.closure.contains(&op.id.name) {
                continue;
            }
            if let Some(range) = self.unknown_range(&op.id.name) {
                cursor.index = range.start;
            }
            evaluator.evaluate_op_with_unknowns(op, &mut cursor)?;
        }
        cursor.finish()?;
        Ok(evaluator.finish_trial())
    }
}

fn owns_unknowns(op: &OpStmt, dimensions: &DimensionMap, file: &File) -> bool {
    matches!(op.ctor, Constructor::FreeNum { .. })
        || is_placed_op(op, dimensions)
        || super::routing::route_kind(op).is_some()
        || super::path_label::owns_parameters(op, file)
}

fn unknown_ranges(registry: &[&OpStmt]) -> BTreeMap<String, Range<usize>> {
    let mut start = 0;
    registry
        .iter()
        .map(|op| {
            let count = if super::automatic::is_automatic_box(op) {
                4
            } else if matches!(op.ctor, Constructor::FreeNum { .. })
                || super::routing::route_kind(op).is_some()
            {
                1
            } else {
                2
            };
            let range = start..start + count;
            start += count;
            (op.id.name.clone(), range)
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::float_cmp)]
mod tests {
    use locus_syntax::parse;

    use super::*;
    use crate::scene::{Dimension, NumericValue, ResolvedObject};

    fn file(source: &str) -> File {
        let parsed = parse(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        parsed.file
    }

    #[test]
    fn measured_collection_content_initializes_box_dimensions_before_solving() {
        let source = file(
            "locus 0 beta 6\nrect panel()\nlabel first(\"Long\")\nlabel second(\"Short\")\ngroup captions(first, second)\nrel contain(panel, captions)\n",
        );
        let expanded =
            crate::program::expand_program(&source).expect("expanded content collection");
        let mut inputs = crate::EvaluationInputs::default();
        for (id, width) in [("first", 9.0), ("second", 2.0)] {
            inputs.measured.insert(
                id.to_owned(),
                crate::MeasuredText {
                    width,
                    height: 0.4,
                    role: crate::MeasuredRole::Normal,
                },
            );
        }
        let prepared =
            PreparedEvaluation::with_inputs(&expanded.file, &inputs, &expanded.collections)
                .expect("measured preparation");
        let seed = prepared.evaluate_seed(1.0).expect("content seeds");
        let panel = seed
            .objects()
            .values()
            .next()
            .expect("panel")
            .bbox()
            .expect("box bounds");
        assert!((panel.width() - 9.5).abs() < 1.0e-12);
        assert!((panel.height() - 1.0).abs() < 1.0e-12);
        assert_eq!(prepared.unknown_range("panel"), Some(&(0..4)));
        assert_eq!(prepared.unknown_range("second"), Some(&(6..8)));
    }

    #[test]
    fn deferred_intersection_does_not_change_the_solve() {
        let source =
            "locus 0 beta 6\ncircle a(1)\ncircle b(1)\nrel dist(a.center, b.center) == 1\n";
        let baseline = crate::solve::solve(&file(source)).expect("circle solve");
        let decorated_file = file(&format!(
            "{source}point P = intersect(a, b, 0)\nassert on(P, a)\n"
        ));
        let decorated = crate::solve::solve(&decorated_file).expect("deferred intersection solve");
        assert_eq!(baseline.summary, decorated.summary);
        assert_eq!(
            baseline.scene.objects.values().collect::<Vec<_>>(),
            decorated.scene.objects.values().take(2).collect::<Vec<_>>()
        );
        assert!(
            crate::assert::evaluate_assertions(&decorated_file, &decorated.scene)
                .expect("final assertions")
                .passed
        );
    }

    #[test]
    fn unreferenced_free_number_keeps_its_hint_and_dof_reports() {
        let solved = crate::solve::solve(&file("locus 0 beta 6\nnum unused = free 7\n"))
            .expect("unreferenced unknown solve");
        assert_eq!(solved.summary.unknowns, 1);
        assert_eq!(solved.summary.dof.global_remaining, 1);
        assert_eq!(solved.summary.dof.objects[0].id, "unused");
        assert_eq!(solved.summary.dof.objects[0].hint_settled_dofs, 1);
        assert_eq!(solved.summary.hint_displacements[0].id, "unused");
        assert_eq!(
            solved.summary.hint_displacements[0].source_components,
            [0.0]
        );
        assert_eq!(
            solved.scene.objects.values().next(),
            Some(&ResolvedObject::Num(NumericValue {
                value: 7.0,
                dimension: Dimension::Scalar
            }))
        );
    }

    #[test]
    fn registry_ranges_survive_interleaved_deferred_declarations() {
        let file = file(
            "locus 0 beta 6\npoint A()\npoint far = on_circle(circle(A, 1000), 0deg)\nnum spare = free 7\npoint B()\nrel dist(A, B) == 2\n",
        );
        let prepared = PreparedEvaluation::new(&file).expect("prepared program");
        assert_eq!(prepared.unknown_range("A"), Some(&(0..2)));
        assert_eq!(prepared.unknown_range("spare"), Some(&(2..3)));
        assert_eq!(prepared.unknown_range("B"), Some(&(3..5)));
        let values = [1.0, 2.0, 9.0, 3.0, 4.0];
        let trial = prepared
            .evaluate_trial(Some(&values))
            .expect("active trial");
        assert_eq!(trial.objects().len(), 3);
        let final_evaluation = prepared.evaluate_final(&values).expect("all declarations");
        assert_eq!(final_evaluation.objects().len(), 4);
        assert!(final_evaluation.tolerance().diameter() > 100.0);
        assert!(trial.tolerance().diameter() < 3.0);
    }

    #[test]
    fn unknown_dimensions_and_numeric_hints_keep_transitive_dependencies() {
        let file = file(
            "locus 0 beta 6\nnum base = 2\nnum radius = base + 1\nnum spare = free radius\ncircle c(radius)\n",
        );
        let prepared = PreparedEvaluation::new(&file).expect("prepared program");
        assert_eq!(
            prepared
                .solver_ops()
                .map(|op| op.id.name.as_str())
                .collect::<Vec<_>>(),
            ["base", "radius", "spare", "c"]
        );
        assert_eq!(
            prepared
                .evaluate_trial(None)
                .expect("seed dependencies")
                .objects()
                .len(),
            4
        );
    }

    #[test]
    fn construction_failure_in_the_relation_closure_retries_initialization() {
        let file = file(
            "locus 0 beta 6\ncircle a(1)\ncircle b(1)\npoint P = intersect(a, b, 0) where { on(P, a) }\n",
        );
        let prepared = PreparedEvaluation::new(&file).expect("prepared program");
        assert!(prepared.evaluate_trial(None).is_err());
        let solved = crate::solve::solve(&file).expect("bounded initializer finds an intersection");
        assert_eq!(solved.summary.unknowns, 4);
        assert!(solved.summary.max_normalized_residual < crate::tol::SOLVE_EPSILON);
    }

    #[test]
    fn failure_in_a_deferred_construction_is_a_final_construction_error() {
        let file = file(
            "locus 0 beta 6\ncircle a(1)\ncircle b(1)\nrel dist(a.center, b.center) == 3\npoint P = intersect(a, b, 0)\n",
        );
        let error = crate::solve::solve(&file).expect_err("accepted circles do not intersect");
        assert_eq!(error.code, "E201");
        assert!(!error.message.contains("solver seed"));
    }

    #[test]
    fn deferred_distant_geometry_cannot_inflate_solve_tolerance() {
        let file = file(
            "locus 0 beta 6\npoint O()\npoint Far = on_circle(circle(O, 1000000), 0deg)\ncircle c(O, 6)\nrel radius(c) / 2 == 3.000001\n",
        );
        let error = crate::solve::solve(&file).expect_err("constant length relation conflicts");
        assert_eq!(error.code, "E403");
    }

    #[test]
    fn repeated_function_bodies_infer_dimensions_per_instance() {
        let file = file(
            "locus 0 beta 6\nfn measure() {\n num amount = free 1\n num twice = amount * 2\n}\nmeasure scalar()\nmeasure length()\ncircle disk(length.amount)\n",
        );
        let solved = crate::solve::solve(&file).expect("instance-local numeric dimensions");
        let dimensions = solved
            .scene
            .objects
            .iter()
            .filter_map(|(id, object)| match object {
                ResolvedObject::Num(value) => Some((id.name.as_str(), value.dimension)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            dimensions,
            [
                ("scalar.amount", Dimension::Scalar),
                ("scalar.twice", Dimension::Scalar),
                ("length.amount", Dimension::Length),
                ("length.twice", Dimension::Length),
            ]
        );
    }
}
