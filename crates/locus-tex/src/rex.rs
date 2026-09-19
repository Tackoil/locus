//! `ReX` math layout with bundled STIX Two Math glyph outlines.

use std::fmt::Write as _;
use std::sync::OnceLock;

use rex_backend::fp::F24P8;
use rex_backend::layout::{Alignment, engine::layout};
use rex_backend::parser::{color::RGBA, parse};
use rex_backend::{Cursor, RenderSettings, Renderer};
use ttf_parser::Face;

use crate::engine::{escape_xml, format_number};
use crate::plaintext::SvgPath;
use crate::{BoxMetrics, FontSize, TexBox, TexEngine, TexError};

// This exact font ships with the pinned ReX revision. Its private-use character
// mappings must match ReX's generated STIX2 variant tables.
const FONT_BYTES: &[u8] = include_bytes!("../assets/rex-stix2.otf");
const UNITS_PER_EM: f64 = 1000.0;
static FONT: OnceLock<Result<Face<'static>, TexError>> = OnceLock::new();

/// A deterministic, pure Rust TeX engine with self-contained SVG outlines.
///
/// `ReX` supplies math parsing and display-style layout; the bundled font supplies
/// paths. No installed fonts, external processes, or network access are needed.
#[derive(Clone, Copy, Debug, Default)]
pub struct RexEngine;

impl TexEngine for RexEngine {
    fn render(&self, tex: &str, size: FontSize) -> Result<TexBox, TexError> {
        let size = size.validate()?;
        if tex.contains(['\n', '\r']) {
            return Err(TexError::RawTexNewline);
        }
        let source = escape_xml(tex)?;
        let face = bundled_font()?;
        // Upstream still panics for some unsupported inputs. Keep those failures
        // inside the fallible engine boundary.
        std::panic::catch_unwind(|| render_math(tex, &source, size, face)).map_err(|_| {
            TexError::InvalidTex {
                message: String::from("ReX could not lay out this expression"),
            }
        })?
    }
}

fn bundled_font() -> Result<&'static Face<'static>, TexError> {
    FONT.get_or_init(|| {
        Face::parse(FONT_BYTES, 0).map_err(|_| TexError::InvalidFont {
            message: "could not parse the bundled ReX STIX Two Math font",
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

fn render_math(tex: &str, source: &str, size: f64, face: &Face<'_>) -> Result<TexBox, TexError> {
    let parsed = parse(tex).map_err(|error| TexError::InvalidTex {
        message: error.to_string(),
    })?;
    let backend = OutlineRenderer {
        face,
        settings: RenderSettings::default(),
    };
    let math = layout(&parsed, backend.settings.layout_settings());
    let mut drawing = Drawing::default();
    drawing.include(
        0.0,
        -f64::from(math.height),
        f64::from(math.width),
        -f64::from(math.depth),
    );
    backend.render_hbox(
        &mut drawing,
        Cursor::default(),
        &math.contents,
        math.height,
        math.width,
        Alignment::Default,
    );
    if let Some(error) = drawing.error {
        return Err(error);
    }
    let scale = size / UNITS_PER_EM;
    let mut rendered = TexBox::with_horizontal_metrics(
        String::new(),
        BoxMetrics::new(
            (drawing.right - drawing.left) * scale,
            -drawing.top * scale,
            drawing.bottom * scale,
        ),
        f64::from(math.width) * scale,
        -drawing.left * scale,
    );
    rendered.validate()?;
    rendered.svg_body = format!(
        "<g data-engine=\"rex\" data-font=\"STIX Two Math\" data-source=\"{source}\" transform=\"scale({})\"><g transform=\"translate({} 0)\">{}</g></g>",
        format_number(scale),
        format_number(-drawing.left),
        drawing.svg,
    );
    Ok(rendered)
}

#[derive(Default)]
struct Drawing {
    svg: String,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
    error: Option<TexError>,
}

impl Drawing {
    fn include(&mut self, left: f64, top: f64, right: f64, bottom: f64) {
        self.left = self.left.min(left);
        self.top = self.top.min(top);
        self.right = self.right.max(right);
        self.bottom = self.bottom.max(bottom);
    }

    fn symbol(&mut self, face: &Face<'_>, pos: Cursor, symbol: u32, scale: f64) {
        let glyph = char::from_u32(symbol).and_then(|character| face.glyph_index(character));
        let Some(glyph) = glyph else {
            self.error = Some(TexError::InvalidTex {
                message: format!("the bundled ReX font has no glyph for U+{symbol:04X}"),
            });
            return;
        };
        let mut path = SvgPath::default();
        let Some(bounds) = face.outline_glyph(glyph, &mut path) else {
            return;
        };
        let x = f64::from(pos.x);
        let y = f64::from(pos.y);
        self.include(
            x + f64::from(bounds.x_min) * scale,
            y - f64::from(bounds.y_max) * scale,
            x + f64::from(bounds.x_max) * scale,
            y - f64::from(bounds.y_min) * scale,
        );
        let _ = write!(
            self.svg,
            "<path transform=\"translate({} {}) scale({} {})\" d=\"{}\"/>",
            format_number(x),
            format_number(y),
            format_number(scale),
            format_number(-scale),
            path.data,
        );
    }
}

struct OutlineRenderer<'a> {
    face: &'a Face<'a>,
    settings: RenderSettings,
}

impl Renderer for OutlineRenderer<'_> {
    type Out = Drawing;

    fn settings(&self) -> &RenderSettings {
        &self.settings
    }

    fn symbol(&self, out: &mut Drawing, pos: Cursor, symbol: u32, scale: f64) {
        out.symbol(self.face, pos, symbol, scale);
    }

    fn rule(&self, out: &mut Drawing, pos: Cursor, width: F24P8, height: F24P8) {
        let (x, y) = (f64::from(pos.x), f64::from(pos.y));
        let (width, height) = (f64::from(width), f64::from(height));
        if width < 0.0 || height < 0.0 {
            out.error = Some(TexError::InvalidTex {
                message: String::from("rule dimensions must be non-negative"),
            });
            return;
        }
        out.include(x, y, x + width, y + height);
        let _ = write!(
            out.svg,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
            format_number(x),
            format_number(y),
            format_number(width),
            format_number(height),
        );
    }

    fn color<F>(&self, out: &mut Drawing, _color: RGBA, mut contents: F)
    where
        F: FnMut(&Self, &mut Drawing),
    {
        // Locus applies the annotation's semantic color to all text outlines.
        contents(self, out);
    }
}

#[cfg(test)]
mod tests;
