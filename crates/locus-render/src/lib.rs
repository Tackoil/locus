//! Deterministic backends over borrowed geometry and already positioned layout.
//!
//! Build a [`style::StyleSheet`] from the parsed file, then pass it together with
//! the corresponding `ResolvedScene` and `LayoutScene` to [`svg::render`] or
//! [`tikz::render`]. Neither backend solves geometry or places text again.
//! Text is emitted as the measured outlines in both backends, requiring no
//! fonts or TeX installation at render time. `TikZ` needs only the `tikz` package.

#[cfg(feature = "png")]
pub mod png;
mod primitives;
pub mod style;
pub mod svg;
mod text;
pub mod tikz;
pub mod viewport;

use locus_syntax::Span;
use thiserror::Error;

pub use svg::render as render_svg;
pub use tikz::render as render_tikz;

/// Backend-wide physical scale and viewport policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderOptions {
    /// CSS pixels per scene unit. `TikZ` uses 0.75pt per pixel.
    pub pixels_per_unit: f64,
    /// Padding on each side in scene units, after fitting visible content.
    pub margin: f64,
    /// Optional opaque viewport fill, painted behind geometry and annotations.
    pub background: Option<style::Color>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            pixels_per_unit: 32.0,
            margin: 0.5,
            background: None,
        }
    }
}

impl RenderOptions {
    pub(crate) fn validate(self) -> Result<(), RenderError> {
        if self.pixels_per_unit.is_finite()
            && self.pixels_per_unit > 0.0
            && self.margin.is_finite()
            && self.margin >= 0.0
        {
            Ok(())
        } else {
            Err(RenderError::Options)
        }
    }
}

/// Source-aware rendering failures; existing E4xx codes remain unchanged.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum RenderError {
    /// Invalid style, geometry, or measured outline at the rendering boundary.
    #[error("E415: {message}")]
    Invalid {
        /// Source of the rejected value.
        span: Span,
        /// Description of the invalid input.
        message: String,
    },
    /// Global viewport or physical scale is invalid or unrepresentable.
    #[error("E416: invalid render scale, margin, or viewport dimensions")]
    Options,
    /// Optional raster backend failed to parse, allocate, or encode.
    #[error("E417: PNG rendering failed: {message}")]
    Raster {
        /// Underlying backend failure.
        message: String,
    },
}

impl RenderError {
    /// Stable machine-readable code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Invalid { .. } => "E415",
            Self::Options => "E416",
            Self::Raster { .. } => "E417",
        }
    }

    pub(crate) fn invalid(span: Span, message: impl Into<String>) -> Self {
        Self::Invalid {
            span,
            message: message.into(),
        }
    }
}
