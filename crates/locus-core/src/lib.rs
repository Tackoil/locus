//! Deterministic, parser-independent geometry for Locus.
//!
//! All transcendental operations in this crate go through the workspace-pinned
//! `libm` dependency. Tolerance decisions belong exclusively in [`tol`].

// Every public fallible geometry operation forwards the typed E2xx-ready enums;
// repeating the same `# Errors` catalog on each small constructor obscures the math.
#![allow(clippy::missing_errors_doc)]

pub mod geom;
pub mod tol;

pub use geom::{Arc, BBox, Circle, ConstructError, Line, Point, Polygon, Ray, Segment, Vec2};
pub use tol::Tolerance;
