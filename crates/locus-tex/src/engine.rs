//! Renderer-neutral measurement types.

use thiserror::Error;

/// A font size expressed as a multiple of the label's base em.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontSize(f64);

impl FontSize {
    /// The default one-em font size.
    pub const DEFAULT: Self = Self(1.0);

    /// Creates a font size.
    ///
    /// Engines reject non-finite or non-positive values with
    /// [`TexError::InvalidFontSize`].
    #[must_use]
    pub const fn new(em: f64) -> Self {
        Self(em)
    }

    /// Returns the size as a multiple of the base em.
    #[must_use]
    pub const fn em(self) -> f64 {
        self.0
    }

    pub(crate) fn validate(self) -> Result<f64, TexError> {
        if self.0.is_finite() && self.0 > 0.0 {
            Ok(self.0)
        } else {
            Err(TexError::InvalidFontSize { size: self.0 })
        }
    }
}

impl Default for FontSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Em-relative dimensions around a horizontal baseline.
///
/// `height` is the distance above the baseline and `depth` is the distance
/// below it. All three values are non-negative.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoxMetrics {
    /// Inline extent in ems.
    pub width: f64,
    /// Extent above the baseline in ems.
    pub height: f64,
    /// Extent below the baseline in ems.
    pub depth: f64,
}

impl BoxMetrics {
    /// A box with no extent.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
        depth: 0.0,
    };

    /// Creates a measured box.
    #[must_use]
    pub const fn new(width: f64, height: f64, depth: f64) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    pub(crate) fn validate(self) -> Result<(), TexError> {
        validate_metric("width", self.width)?;
        validate_metric("height", self.height)?;
        validate_metric("depth", self.depth)
    }
}

/// SVG output and its measured em-relative bounds.
///
/// The SVG body places the measured visual bounds at `x = 0`, with the
/// baseline at `y = 0` and positive y pointing down, matching SVG
/// coordinates. For a box composed by [`crate::render_segments`], this is the
/// first line's baseline; every later line is positioned at a positive y
/// offset.
///
/// `baseline_x` locates the inline origin within the measured visual bounds,
/// while `advance` is the logical distance to the next fragment. Keeping
/// those values separate prevents glyph overhangs from adding inline space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexBox {
    /// SVG elements representing the rendered fragment, without an outer SVG.
    pub svg_body: String,
    /// Measured dimensions used by label layout.
    pub metrics: BoxMetrics,
    /// Logical inline advance to the next fragment, in ems.
    pub advance: f64,
    /// Horizontal distance from the visual box's left edge to its inline
    /// origin, in ems.
    pub baseline_x: f64,
}

impl TexBox {
    /// Creates rendered output from an SVG body and measured dimensions.
    #[must_use]
    pub fn new(svg_body: impl Into<String>, metrics: BoxMetrics) -> Self {
        Self {
            svg_body: svg_body.into(),
            metrics,
            advance: metrics.width,
            baseline_x: 0.0,
        }
    }

    /// Creates rendered output with distinct visual and logical horizontal
    /// measurements.
    #[must_use]
    pub fn with_horizontal_metrics(
        svg_body: impl Into<String>,
        metrics: BoxMetrics,
        advance: f64,
        baseline_x: f64,
    ) -> Self {
        Self {
            svg_body: svg_body.into(),
            metrics,
            advance,
            baseline_x,
        }
    }

    /// Creates an empty rendered box.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn validate(&self) -> Result<(), TexError> {
        self.metrics.validate()?;
        validate_finite_metric("advance", self.advance)?;
        validate_metric("baseline_x", self.baseline_x)
    }
}

/// A renderer hidden behind the Locus text/TeX boundary.
pub trait TexEngine {
    /// Render a TeX fragment; returns SVG body + measured box (em-relative).
    ///
    /// # Errors
    ///
    /// Returns [`TexError`] when the size, input, font, renderer, or measured
    /// output is invalid or unavailable.
    fn render(&self, tex: &str, size: FontSize) -> Result<TexBox, TexError>;
}

/// An error produced while measuring or rendering a text fragment.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TexError {
    /// A font size was not finite and positive.
    #[error("font size must be finite and positive, got {size}")]
    InvalidFontSize {
        /// Rejected em-relative size.
        size: f64,
    },
    /// An engine produced a dimension that cannot participate in layout.
    #[error("rendered {field} must be finite and non-negative, got {value}")]
    InvalidMetrics {
        /// Name of the rejected metric.
        field: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// Bundled or caller-provided font data was invalid.
    #[error("invalid font data: {message}")]
    InvalidFont {
        /// Parser failure description.
        message: &'static str,
    },
    /// A shaped glyph identifier was outside the TrueType range.
    #[error("shaper returned invalid glyph id {glyph_id}")]
    InvalidGlyphId {
        /// Rejected glyph identifier.
        glyph_id: u32,
    },
    /// Positioned font-unit arithmetic exceeded the supported range.
    #[error("text measurement exceeded the supported font-unit range")]
    MeasurementOverflow,
    /// Text contained a character forbidden by XML 1.0.
    #[error("text contains XML-invalid character U+{codepoint:04X}")]
    InvalidXmlCharacter {
        /// Rejected Unicode code point.
        codepoint: u32,
    },
    /// A TeX segment contained a raw line break.
    #[error("TeX segments cannot contain raw line breaks")]
    RawTexNewline,
    /// A selected feature exposes an engine seam without a working backend.
    #[error("{engine} engine is unavailable: {reason}")]
    EngineUnavailable {
        /// Stable engine name.
        engine: &'static str,
        /// Why the engine cannot run in this build.
        reason: &'static str,
    },
}

fn validate_metric(field: &'static str, value: f64) -> Result<(), TexError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(TexError::InvalidMetrics { field, value })
    }
}

fn validate_finite_metric(field: &'static str, value: f64) -> Result<(), TexError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(TexError::InvalidMetrics { field, value })
    }
}

pub(crate) fn format_number(value: f64) -> String {
    if value == 0.0 {
        return String::from("0");
    }

    let mut buffer = ryu::Buffer::new();
    buffer.format_finite(value).to_owned()
}

pub(crate) fn escape_xml(value: &str) -> Result<String, TexError> {
    const ESCAPES: &[(char, &str)] = &[
        ('&', "&amp;"),
        ('<', "&lt;"),
        ('>', "&gt;"),
        ('"', "&quot;"),
        ('\'', "&apos;"),
    ];

    if let Some(character) = value
        .chars()
        .find(|character| !is_xml_character(*character))
    {
        return Err(TexError::InvalidXmlCharacter {
            codepoint: u32::from(character),
        });
    }

    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if let Some((_, entity)) = ESCAPES.iter().find(|(escaped, _)| *escaped == character) {
            escaped.push_str(entity);
        } else {
            escaped.push(character);
        }
    }
    Ok(escaped)
}

const fn is_xml_character(character: char) -> bool {
    matches!(
        character,
        '\u{0009}' | '\u{000A}' | '\u{000D}' | '\u{0020}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_forbidden_control_character_is_rejected() {
        let result = escape_xml("before\0after");

        assert_eq!(result, Err(TexError::InvalidXmlCharacter { codepoint: 0 }));
    }
}
