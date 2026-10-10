//! Composition of ordered plain-text and TeX segments.

use crate::engine::{BoxMetrics, FontSize, TexBox, TexEngine, TexError, format_number};

const LINE_GAP_EM: f64 = 0.2;

pub(crate) fn compose_inline(pieces: Vec<TexBox>) -> Result<TexBox, TexError> {
    let mut line = Line::default();
    for piece in pieces {
        line.push(piece)?;
    }
    compose_lines(&[line], 1.0)
}

/// A borrowed segment from a parsed label value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSegment<'a> {
    /// Text shaped with the deterministic plain-text engine.
    Plain(&'a str),
    /// Math source rendered with the selected TeX engine.
    Tex(&'a str),
}

/// Renders ordered text segments into one measured, baseline-aligned box.
///
/// Adjacent segments are concatenated without whitespace. Newlines in plain
/// segments start centered lines; raw newlines in TeX segments are rejected.
/// The returned box's baseline is the first line's baseline. A trailing plain
/// newline starts an empty final line, and that line contributes the plain
/// engine's normal line height and depth.
///
/// # Errors
///
/// Returns [`TexError`] when either engine fails, a TeX segment contains a raw
/// newline, the size is invalid, or an engine returns invalid metrics.
pub fn render_segments(
    segments: &[TextSegment<'_>],
    plaintext_engine: &dyn TexEngine,
    tex_engine: &dyn TexEngine,
    size: FontSize,
) -> Result<TexBox, TexError> {
    let size_em = size.validate()?;
    if segments.is_empty() {
        return Ok(TexBox::empty());
    }

    let mut lines = Vec::new();
    let mut line = Line::default();
    for segment in segments {
        match *segment {
            TextSegment::Plain(source) => {
                render_plain_segment(source, plaintext_engine, size, &mut line, &mut lines)?;
            }
            TextSegment::Tex(source) => {
                if source.contains(['\n', '\r']) {
                    return Err(TexError::RawTexNewline);
                }
                line.push(tex_engine.render(source, size)?)?;
            }
        }
    }
    lines.push(line);

    compose_lines(&lines, size_em)
}

fn render_plain_segment(
    source: &str,
    engine: &dyn TexEngine,
    size: FontSize,
    line: &mut Line,
    lines: &mut Vec<Line>,
) -> Result<(), TexError> {
    let mut parts = source.split('\n').peekable();
    while let Some(part) = parts.next() {
        line.push(engine.render(part, size)?)?;
        if parts.peek().is_some() {
            lines.push(std::mem::take(line));
        }
    }
    Ok(())
}

#[derive(Default)]
struct Line {
    pieces: Vec<Piece>,
    metrics: BoxMetrics,
    cursor: f64,
    x_min: f64,
    x_max: f64,
}

impl Line {
    fn push(&mut self, rendered: TexBox) -> Result<(), TexError> {
        rendered.validate()?;
        let x = self.cursor - rendered.baseline_x;
        let right = x + rendered.metrics.width;
        let cursor = self.cursor + rendered.advance;
        let x_min = self.x_min.min(x);
        let x_max = self.x_max.max(right);
        let metrics = BoxMetrics::new(
            x_max - x_min,
            self.metrics.height.max(rendered.metrics.height),
            self.metrics.depth.max(rendered.metrics.depth),
        );
        TexBox::with_horizontal_metrics(String::new(), metrics, cursor, -x_min).validate()?;

        self.metrics = metrics;
        self.cursor = cursor;
        self.x_min = x_min;
        self.x_max = x_max;
        self.pieces.push(Piece { x, rendered });
        Ok(())
    }
}

struct Piece {
    x: f64,
    rendered: TexBox,
}

fn compose_lines(lines: &[Line], size_em: f64) -> Result<TexBox, TexError> {
    let width = lines
        .iter()
        .map(|line| line.metrics.width)
        .fold(0.0_f64, f64::max);
    let baselines = line_baselines(lines, size_em);
    let height = lines.first().map_or(0.0, |line| line.metrics.height);
    let depth = lines
        .last()
        .zip(baselines.last())
        .map_or(0.0, |(line, baseline)| baseline + line.metrics.depth);
    let metrics = BoxMetrics::new(width, height, depth);
    let (advance, baseline_x) = lines.first().map_or((0.0, 0.0), |line| {
        let centered_x = (width - line.metrics.width) / 2.0;
        (line.cursor, centered_x - line.x_min)
    });
    let mut rendered = TexBox::with_horizontal_metrics(String::new(), metrics, advance, baseline_x);
    rendered.validate()?;

    for (line, baseline) in lines.iter().zip(&baselines) {
        append_line(&mut rendered.svg_body, line, width, *baseline);
    }

    Ok(rendered)
}

fn line_baselines(lines: &[Line], size_em: f64) -> Vec<f64> {
    let mut baselines = Vec::with_capacity(lines.len());
    let mut baseline = 0.0;
    for (index, line) in lines.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1).and_then(|value| lines.get(value)) {
            baseline += previous.metrics.depth + LINE_GAP_EM * size_em + line.metrics.height;
        }
        baselines.push(baseline);
    }
    baselines
}

fn append_line(output: &mut String, line: &Line, width: f64, baseline: f64) {
    let centered_x = (width - line.metrics.width) / 2.0;
    for piece in &line.pieces {
        output.push_str("<g transform=\"translate(");
        output.push_str(&format_number(centered_x + piece.x - line.x_min));
        output.push(' ');
        output.push_str(&format_number(baseline));
        output.push_str(")\">");
        output.push_str(&piece.rendered.svg_body);
        output.push_str("</g>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MockEngine, PlaintextEngine};

    #[test]
    fn mixed_segments_share_a_baseline_without_added_space() -> Result<(), TexError> {
        let rendered = render_segments(
            &[
                TextSegment::Plain("A"),
                TextSegment::Tex("x"),
                TextSegment::Plain(""),
            ],
            &MockEngine,
            &MockEngine,
            FontSize::DEFAULT,
        )?;

        assert_eq!(rendered.metrics, BoxMetrics::new(1.25, 0.8, 0.2));
        assert_eq!(rendered.svg_body.matches("translate(").count(), 3);
        Ok(())
    }

    #[test]
    fn multiline_segments_are_centered_and_measured() -> Result<(), TexError> {
        let rendered = render_segments(
            &[TextSegment::Plain("A\n"), TextSegment::Tex("WW")],
            &MockEngine,
            &MockEngine,
            FontSize::DEFAULT,
        )?;

        assert!((rendered.metrics.width - 1.8).abs() <= f64::EPSILON);
        assert!((rendered.metrics.height - 0.8).abs() <= f64::EPSILON);
        assert!((rendered.metrics.depth - 1.4).abs() <= f64::EPSILON);
        assert!(rendered.svg_body.contains("translate(0.55 0)"));
        assert!(rendered.svg_body.contains("translate(0 1.2"));
        Ok(())
    }

    #[test]
    fn tex_newlines_are_rejected_before_engine_dispatch() {
        let result = render_segments(
            &[TextSegment::Tex("x\ny")],
            &MockEngine,
            &MockEngine,
            FontSize::DEFAULT,
        );

        assert_eq!(result, Err(TexError::RawTexNewline));
    }

    #[test]
    fn trailing_plain_newline_keeps_an_empty_final_line() -> Result<(), TexError> {
        let rendered = render_segments(
            &[TextSegment::Plain("A\n")],
            &MockEngine,
            &MockEngine,
            FontSize::DEFAULT,
        )?;

        assert!((rendered.metrics.depth - 1.4).abs() <= f64::EPSILON);
        assert_eq!(rendered.svg_body.matches("data-engine=\"mock\"").count(), 2);
        Ok(())
    }

    #[test]
    fn segment_overhangs_do_not_change_logical_spacing() -> Result<(), TexError> {
        let plaintext = PlaintextEngine::default();
        let whole = render_segments(
            &[TextSegment::Plain("fH")],
            &plaintext,
            &MockEngine,
            FontSize::DEFAULT,
        )?;
        let split = render_segments(
            &[TextSegment::Plain("f"), TextSegment::Plain("H")],
            &plaintext,
            &MockEngine,
            FontSize::DEFAULT,
        )?;

        assert!((whole.metrics.width - split.metrics.width).abs() <= f64::EPSILON);
        assert!((whole.advance - split.advance).abs() <= f64::EPSILON);
        Ok(())
    }
}
