//! Deterministic plain-text and TeX measurement for Locus labels.
//!
//! All renderers implement [`TexEngine`], so consumers only handle an SVG body
//! and em-relative [`BoxMetrics`]. Plain and TeX fragments can be combined with
//! [`render_segments`] without exposing the selected engines to downstream
//! layout or rendering crates.

mod engine;
mod mock;
mod segments;

pub mod plaintext;

#[cfg(feature = "rex")]
pub mod rex;

pub use engine::{BoxMetrics, FontSize, TexBox, TexEngine, TexError};
pub use mock::MockEngine;
pub use plaintext::PlaintextEngine;
#[cfg(feature = "rex")]
pub use rex::RexEngine;
pub use segments::{TextSegment, render_segments};
