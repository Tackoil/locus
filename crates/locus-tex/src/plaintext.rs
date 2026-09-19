//! Deterministic OpenType shaping for non-TeX label segments.

use std::fmt::{self, Write as _};
use std::sync::OnceLock;

use rustybuzz::{Face as ShapingFace, UnicodeBuffer};
use ttf_parser::{Face as OutlineFace, GlyphId, OutlineBuilder, name_id};

use crate::engine::{BoxMetrics, FontSize, TexBox, TexEngine, TexError, escape_xml, format_number};

/// Family name of the bundled deterministic default font.
pub const DEFAULT_FONT_FAMILY: &str = "Noto Sans";

/// The bundled Noto Sans Regular font bytes.
///
/// The font is distributed under the SIL Open Font License 1.1; its license
/// and provenance are in `assets/OFL.txt` and `assets/README.md`.
pub const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");

/// A deterministic plain-text engine backed by `rustybuzz` and `ttf-parser`.
///
/// Measurements contain both the logical line box and every positioned glyph
/// outline. Empty strings therefore retain the font's normal line height,
/// while overhangs and stacked marks can expand non-empty boxes. Parsed font
/// tables are retained by the engine and reused across render calls.
#[derive(Clone)]
pub struct PlaintextEngine {
    source: FontSource,
}

impl PlaintextEngine {
    /// Uses the bundled Noto Sans Regular face.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            source: FontSource::Bundled,
        }
    }

    /// Creates an engine from a caller-bundled static OpenType font.
    ///
    /// # Errors
    ///
    /// Returns [`TexError::InvalidFont`] when `ttf-parser` rejects the selected
    /// face. The SVG `data-font` metadata is read from that face's OpenType
    /// naming table.
    pub fn from_static_font(font_data: &'static [u8], face_index: u32) -> Result<Self, TexError> {
        Ok(Self {
            source: FontSource::Custom(Box::new(parse_font(font_data, face_index)?)),
        })
    }
}

impl fmt::Debug for PlaintextEngine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlaintextEngine")
            .field("source", &self.source.description())
            .finish_non_exhaustive()
    }
}

impl Default for PlaintextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TexEngine for PlaintextEngine {
    fn render(&self, text: &str, size: FontSize) -> Result<TexBox, TexError> {
        let size = size.validate()?;
        let font = self.parsed_font()?;
        let shaping_face = &font.shaping_face;
        let outline_face = shaping_face.as_ref();
        let units_per_em = f64::from(outline_face.units_per_em());
        let scale = size / units_per_em;

        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(shaping_face, &[], buffer);
        let glyphs = glyph_layout(outline_face, &shaped)?;
        let measured = glyphs.measured_box(outline_face, scale)?;
        let mut rendered = TexBox::with_horizontal_metrics(
            String::new(),
            measured.metrics,
            measured.advance,
            measured.baseline_x,
        );
        rendered.validate()?;
        rendered.svg_body = render_svg_group(
            text,
            &font.family_name,
            scale,
            measured.shift_x,
            &glyphs.paths,
        )?;
        Ok(rendered)
    }
}

impl PlaintextEngine {
    fn parsed_font(&self) -> Result<&ParsedFont, TexError> {
        match &self.source {
            FontSource::Bundled => bundled_font(),
            FontSource::Custom(font) => Ok(font),
        }
    }
}

#[derive(Clone)]
enum FontSource {
    Bundled,
    Custom(Box<ParsedFont>),
}

impl FontSource {
    const fn description(&self) -> &'static str {
        match self {
            Self::Bundled => "bundled Noto Sans",
            Self::Custom(_) => "custom static font",
        }
    }
}

#[derive(Clone)]
struct ParsedFont {
    shaping_face: ShapingFace<'static>,
    family_name: String,
}

static BUNDLED_FONT: OnceLock<Result<ParsedFont, TexError>> = OnceLock::new();

fn bundled_font() -> Result<&'static ParsedFont, TexError> {
    match BUNDLED_FONT.get_or_init(|| parse_font(DEFAULT_FONT_BYTES, 0)) {
        Ok(font) => Ok(font),
        Err(error) => Err(error.clone()),
    }
}

fn parse_font(font_data: &'static [u8], face_index: u32) -> Result<ParsedFont, TexError> {
    let outline_face =
        OutlineFace::parse(font_data, face_index).map_err(|_| TexError::InvalidFont {
            message: "ttf-parser could not parse the selected face",
        })?;
    let family_name = font_family_name(&outline_face);
    Ok(ParsedFont {
        shaping_face: ShapingFace::from_face(outline_face),
        family_name,
    })
}

fn font_family_name(face: &OutlineFace<'_>) -> String {
    font_name(face, name_id::TYPOGRAPHIC_FAMILY)
        .or_else(|| font_name(face, name_id::FAMILY))
        .unwrap_or_else(|| String::from("unnamed OpenType font"))
}

fn font_name(face: &OutlineFace<'_>, name_id: u16) -> Option<String> {
    face.names()
        .into_iter()
        .filter(|name| name.name_id == name_id)
        .find_map(|name| name.to_string())
}

fn glyph_layout(
    face: &OutlineFace<'_>,
    shaped: &rustybuzz::GlyphBuffer,
) -> Result<GlyphLayout, TexError> {
    let mut layout = GlyphLayout::default();
    let mut x_cursor = 0_i32;
    let mut y_cursor = 0_i32;

    for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        let glyph_id = u16::try_from(info.glyph_id).map_err(|_| TexError::InvalidGlyphId {
            glyph_id: info.glyph_id,
        })?;
        let x = checked_font_unit_add(x_cursor, position.x_offset)?;
        let y = checked_font_unit_add(y_cursor, position.y_offset)?;
        let bounds = append_glyph_path(&mut layout.paths, face, GlyphId(glyph_id), x, y)?;
        layout.include(bounds);
        x_cursor = checked_font_unit_add(x_cursor, position.x_advance)?;
        y_cursor = checked_font_unit_add(y_cursor, position.y_advance)?;
    }

    layout.advance = x_cursor;
    Ok(layout)
}

fn append_glyph_path(
    output: &mut String,
    face: &OutlineFace<'_>,
    glyph_id: GlyphId,
    x: i32,
    y: i32,
) -> Result<Option<FontBounds>, TexError> {
    let mut path = SvgPath::default();
    let Some(bounds) = face.outline_glyph(glyph_id, &mut path) else {
        return Ok(None);
    };
    if !path.data.is_empty() {
        let _ = write!(
            output,
            "<path transform=\"translate({x} {y})\" d=\"{}\"/>",
            path.data
        );
    }
    Ok(Some(FontBounds::from_rect(bounds, x, y)?))
}

fn render_svg_group(
    text: &str,
    family_name: &str,
    scale: f64,
    shift_x: i32,
    paths: &str,
) -> Result<String, TexError> {
    Ok(format!(
        "<g data-font=\"{}\" aria-label=\"{}\" transform=\"scale({} -{})\"><g transform=\"translate({shift_x} 0)\">{paths}</g></g>",
        escape_xml(family_name)?,
        escape_xml(text)?,
        format_number(scale),
        format_number(scale),
    ))
}

#[derive(Default)]
struct GlyphLayout {
    paths: String,
    advance: i32,
    ink_bounds: Option<FontBounds>,
}

impl GlyphLayout {
    fn include(&mut self, bounds: Option<FontBounds>) {
        if let Some(bounds) = bounds {
            self.ink_bounds = Some(
                self.ink_bounds
                    .map_or(bounds, |current| current.union(bounds)),
            );
        }
    }

    fn measured_box(
        &self,
        face: &OutlineFace<'_>,
        scale: f64,
    ) -> Result<MeasuredGlyphBox, TexError> {
        let mut bounds = FontBounds {
            x_min: self.advance.min(0),
            y_min: i32::from(face.descender()).min(0),
            x_max: self.advance.max(0),
            y_max: i32::from(face.ascender()).max(0),
        };
        if let Some(ink_bounds) = self.ink_bounds {
            bounds = bounds.union(ink_bounds);
        }

        let metrics = BoxMetrics::new(
            (f64::from(bounds.x_max) - f64::from(bounds.x_min)) * scale,
            f64::from(bounds.y_max) * scale,
            -f64::from(bounds.y_min) * scale,
        );
        let shift_x = bounds
            .x_min
            .checked_neg()
            .ok_or(TexError::MeasurementOverflow)?;
        Ok(MeasuredGlyphBox {
            metrics,
            advance: f64::from(self.advance) * scale,
            baseline_x: f64::from(shift_x) * scale,
            shift_x,
        })
    }
}

#[derive(Clone, Copy)]
struct MeasuredGlyphBox {
    metrics: BoxMetrics,
    advance: f64,
    baseline_x: f64,
    shift_x: i32,
}

#[derive(Clone, Copy)]
struct FontBounds {
    x_min: i32,
    y_min: i32,
    x_max: i32,
    y_max: i32,
}

impl FontBounds {
    fn from_rect(rect: ttf_parser::Rect, x: i32, y: i32) -> Result<Self, TexError> {
        let first_x = checked_font_unit_add(x, i32::from(rect.x_min))?;
        let second_x = checked_font_unit_add(x, i32::from(rect.x_max))?;
        let first_y = checked_font_unit_add(y, i32::from(rect.y_min))?;
        let second_y = checked_font_unit_add(y, i32::from(rect.y_max))?;
        Ok(Self {
            x_min: first_x.min(second_x),
            y_min: first_y.min(second_y),
            x_max: first_x.max(second_x),
            y_max: first_y.max(second_y),
        })
    }

    fn union(self, other: Self) -> Self {
        Self {
            x_min: self.x_min.min(other.x_min),
            y_min: self.y_min.min(other.y_min),
            x_max: self.x_max.max(other.x_max),
            y_max: self.y_max.max(other.y_max),
        }
    }
}

fn checked_font_unit_add(left: i32, right: i32) -> Result<i32, TexError> {
    left.checked_add(right).ok_or(TexError::MeasurementOverflow)
}

#[derive(Default)]
pub(crate) struct SvgPath {
    pub(crate) data: String,
}

impl SvgPath {
    fn command(&mut self, command: char, values: &[f32]) {
        self.data.push(command);
        for value in values {
            self.data.push(' ');
            self.data.push_str(&format_number(f64::from(*value)));
        }
    }
}

impl OutlineBuilder for SvgPath {
    fn move_to(&mut self, x: f32, y: f32) {
        self.command('M', &[x, y]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.command('L', &[x, y]);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.command('Q', &[x1, y1, x, y]);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.command('C', &[x1, y1, x2, y2, x, y]);
    }

    fn close(&mut self) {
        self.data.push('Z');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_font_measurement_is_deterministic() -> Result<(), TexError> {
        let engine = PlaintextEngine::default();
        let first = engine.render("filter offset", FontSize::new(1.25));
        let second = engine.render("filter offset", FontSize::new(1.25));

        assert_eq!(first, second);
        let rendered = first?;
        assert!(rendered.metrics.width > 0.0);
        assert!(rendered.svg_body.contains("<path"));
        Ok(())
    }

    #[test]
    fn invalid_size_is_typed() {
        let result = PlaintextEngine::default().render("A", FontSize::new(0.0));

        assert!(matches!(
            result,
            Err(TexError::InvalidFontSize { size: 0.0 })
        ));
    }

    #[test]
    fn xml_invalid_text_is_rejected() {
        let result = PlaintextEngine::default().render("bad\0label", FontSize::DEFAULT);

        assert!(matches!(
            result,
            Err(TexError::InvalidXmlCharacter { codepoint: 0 })
        ));
    }

    #[test]
    fn overflowing_dimensions_are_rejected() {
        let result = PlaintextEngine::default().render("WW", FontSize::new(f64::MAX));

        assert!(matches!(result, Err(TexError::InvalidMetrics { .. })));
    }

    #[test]
    fn parsed_bundled_font_is_reused() -> Result<(), TexError> {
        let engine = PlaintextEngine::default();
        let first = std::ptr::from_ref(engine.parsed_font()?);
        engine.render("first", FontSize::DEFAULT)?;
        let second = std::ptr::from_ref(engine.parsed_font()?);

        assert_eq!(first, second);
        Ok(())
    }

    #[test]
    fn custom_font_metadata_comes_from_its_name_table() -> Result<(), TexError> {
        let engine = PlaintextEngine::from_static_font(DEFAULT_FONT_BYTES, 0)?;
        let rendered = engine.render("A", FontSize::DEFAULT)?;

        assert_eq!(engine.parsed_font()?.family_name, DEFAULT_FONT_FAMILY);
        assert!(rendered.svg_body.contains("data-font=\"Noto Sans\""));
        Ok(())
    }

    #[test]
    fn logical_height_is_independent_of_ink_bounds() -> Result<(), TexError> {
        let engine = PlaintextEngine::default();
        let empty = engine.render("", FontSize::DEFAULT)?;
        let lowercase = engine.render("x", FontSize::DEFAULT)?;
        let uppercase = engine.render("X", FontSize::DEFAULT)?;

        assert_eq!(
            empty.metrics.height.to_bits(),
            lowercase.metrics.height.to_bits()
        );
        assert_eq!(
            lowercase.metrics.height.to_bits(),
            uppercase.metrics.height.to_bits()
        );
        assert_eq!(
            empty.metrics.depth.to_bits(),
            lowercase.metrics.depth.to_bits()
        );
        assert_eq!(
            lowercase.metrics.depth.to_bits(),
            uppercase.metrics.depth.to_bits()
        );
        Ok(())
    }

    #[test]
    fn measured_boxes_contain_positioned_glyph_outlines() -> Result<(), TexError> {
        let engine = PlaintextEngine::default();
        let font = engine.parsed_font()?;
        let face = font.shaping_face.as_ref();

        for text in ["f", "j", "A\u{030d}\u{030d}"] {
            assert_glyphs_fit(&engine, face, text)?;
        }
        Ok(())
    }

    fn assert_glyphs_fit(
        engine: &PlaintextEngine,
        face: &OutlineFace<'_>,
        text: &str,
    ) -> Result<(), TexError> {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&engine.parsed_font()?.shaping_face, &[], buffer);
        let glyphs = glyph_layout(face, &shaped)?;
        let scale = 1.0 / f64::from(face.units_per_em());
        let measured = glyphs.measured_box(face, scale)?;
        let ink = glyphs.ink_bounds.ok_or(TexError::InvalidFont {
            message: "test text unexpectedly had no outlines",
        })?;

        assert!(ink.x_min + measured.shift_x >= 0);
        assert!(f64::from(ink.x_max + measured.shift_x) * scale <= measured.metrics.width);
        assert!(f64::from(ink.y_max) * scale <= measured.metrics.height);
        assert!(f64::from(-ink.y_min) * scale <= measured.metrics.depth);
        let rendered = engine.render(text, FontSize::DEFAULT)?;
        assert_eq!(rendered.metrics, measured.metrics);
        assert_eq!(rendered.advance.to_bits(), measured.advance.to_bits());
        assert_eq!(rendered.baseline_x.to_bits(), measured.baseline_x.to_bits());
        assert!(
            rendered
                .svg_body
                .contains(&format!("translate({} 0)", measured.shift_x))
        );
        Ok(())
    }
}
