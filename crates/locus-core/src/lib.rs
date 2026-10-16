//! Deterministic, parser-independent geometry for Locus.
//!
//! All transcendental operations in this crate go through the workspace-pinned
//! `libm` dependency. Tolerance decisions belong exclusively in [`tol`].

// Every public fallible geometry operation forwards the typed E2xx-ready enums;
// repeating the same `# Errors` catalog on each small constructor obscures the math.
#![allow(clippy::missing_errors_doc)]

pub mod assert;
pub mod eval;
pub mod geom;
pub mod inputs;
pub mod inspect;
pub mod metrics;
pub mod preset;
mod program;
pub mod scene;
pub mod solve;
pub mod tol;
pub mod trace;

pub use r#assert::{
    ActualValue, AssertionReport, AssertionResult, ReportDimension, SubtermActual,
    evaluate_assertions, evaluate_assertions_with_inputs,
};
pub use eval::{
    DependencyError, DependencyGraph, EvalError, RemovalImpact, StatementDependency,
    StatementDependencyKind, Value, evaluate, evaluate_with_inputs,
};
pub use geom::{
    Arc, BBox, Circle, ConstructError, Line, Point, Polygon, Ray, RoundedRect, Segment, Vec2,
};
pub use inputs::{
    EvaluationChoices, EvaluationInputs, LabelChoice, MeasuredContent, MeasuredRole, MeasuredText,
};
pub use inspect::{
    DistanceMeasurement, GeometryBounds, Incidence, InspectDiagnostic, InspectionReport,
    IntegerRatio, LayoutCollision, LineRatioMeasurement, MeasurementQuery, MeasurementTables,
    TriangleAngleMeasurement, inspect, inspect_scene, inspect_scene_with_options,
    query_measurements,
};
pub use preset::LayoutMetrics;
pub use scene::{
    AnchorSpec, AnnotationSpec, Dimension, Evaluation, NumericValue, PathGeometry, PointMark,
    ResolvedArc, ResolvedObject, ResolvedPath, ResolvedPoint, ResolvedScene, Scene,
};
pub use solve::{
    BranchResolution, DofReport, HintDisplacement, ObjectDof, RelationReport, SettlingSummary,
    SolveSummary, Solved, UnknownClass, solve, solve_with_inputs,
};
pub use tol::Tolerance;
