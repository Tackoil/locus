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
pub mod inspect;
pub mod scene;
pub mod solve;
pub mod tol;

pub use r#assert::{
    ActualValue, AssertionReport, AssertionResult, ReportDimension, SubtermActual,
    evaluate_assertions,
};
pub use eval::{DependencyError, DependencyGraph, EvalError, Value, evaluate};
pub use geom::{Arc, BBox, Circle, ConstructError, Line, Point, Polygon, Ray, Segment, Vec2};
pub use inspect::{
    DistanceMeasurement, GeometryBounds, Incidence, InspectDiagnostic, InspectionReport,
    IntegerRatio, LineRatioMeasurement, MeasurementTables, TriangleAngleMeasurement, inspect,
    inspect_scene,
};
pub use scene::{
    AnchorSpec, AnnotationSpec, Dimension, Evaluation, NumericValue, PointMark, ResolvedArc,
    ResolvedObject, ResolvedPoint, ResolvedScene, Scene,
};
pub use solve::{
    BranchResolution, DofReport, HintDisplacement, ObjectDof, RelationReport, SolveSummary, Solved,
    solve,
};
pub use tol::Tolerance;
