//! Primary SVG backend and the sole numeric serialization policy.

use crate::{
    RenderError, RenderOptions,
    primitives::{self, Command, Drawing, Shape},
    style::{Color, Style, StyleSheet},
};
use locus_core::{Point, ResolvedScene};
use locus_layout::LayoutScene;
use std::fmt::Write as _;

/// Number of digits after the decimal point in both vector backends.
pub const FLOAT_PRECISION: usize = 6;

/// Locale-free fixed-six-decimal serialization of Ryu's shortest decimal.
///
/// Rounds decimal ties away from zero, expands scientific notation without
/// floating-point scaling/overflow, and normalizes negative zero after rounding.
/// Non-finite values retain Ryu's spelling for diagnostic use; renderers reject
/// them before serialization. No geometry tolerance is applied here.
#[must_use]
pub fn format_float(value: f64) -> String {
    let mut buffer = ryu::Buffer::new();
    let source = buffer.format(value);
    if !value.is_finite() {
        return source.to_owned();
    }
    let unsigned = source.trim_start_matches('-');
    let (mantissa, exponent) = unsigned.split_once('e').unwrap_or((unsigned, "0"));
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    let decimal =
        i32::try_from(mantissa.find('.').unwrap_or(mantissa.len())).unwrap_or(0) + exponent;
    let digits: Vec<_> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let integer_digits = usize::try_from(decimal.max(1)).unwrap_or(1);
    let mut output = Vec::with_capacity(integer_digits + FLOAT_PRECISION + 1);
    for position in (1 - i32::try_from(integer_digits).unwrap_or(1))..=6 {
        let index = decimal + position - 1;
        output.push(
            usize::try_from(index)
                .ok()
                .and_then(|i| digits.get(i))
                .copied()
                .unwrap_or(b'0'),
        );
    }
    let next = usize::try_from(decimal + 6)
        .ok()
        .and_then(|i| digits.get(i))
        .copied()
        .unwrap_or(b'0');
    if next >= b'5' {
        round_up(&mut output);
    }
    let split = output.len() - FLOAT_PRECISION;
    output.insert(split, b'.');
    if value.is_sign_negative() && output.iter().any(|b| matches!(b, b'1'..=b'9')) {
        output.insert(0, b'-');
    }
    output.into_iter().map(char::from).collect()
}

fn round_up(digits: &mut Vec<u8>) {
    for digit in digits.iter_mut().rev() {
        if *digit != b'9' {
            *digit += 1;
            return;
        }
        *digit = b'0';
    }
    digits.insert(0, b'1');
}

/// Renders already-resolved geometry and layout as a complete SVG document.
///
/// # Errors
/// Returns a source-aware [`RenderError`] for invalid styles, coordinates, or
/// measured outlines; never silently drops unsupported text elements.
pub fn render(
    scene: &ResolvedScene,
    layout: &LayoutScene,
    styles: &StyleSheet,
    options: RenderOptions,
) -> Result<String, RenderError> {
    let drawing = primitives::prepare(scene, layout, styles, options)?;
    Ok(document(&drawing))
}

pub(crate) fn document(drawing: &Drawing) -> String {
    let width = format_float(drawing.viewport.width());
    let height = format_float(drawing.viewport.height());
    let mut result = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n"
    );
    if let Some(background) = drawing.background {
        let _ = writeln!(
            result,
            "  <rect width=\"{width}\" height=\"{height}\" fill=\"{}\"{}/>",
            background.hex(),
            alpha_attribute("fill", background)
        );
    }
    for op in &drawing.ops {
        let _ = writeln!(
            result,
            "  <g id=\"{}\" data-source-start=\"{}\" data-source-end=\"{}\" opacity=\"{}\">",
            escape(&op.id.name),
            op.id.span.byte_start,
            op.id.span.byte_end,
            format_float(op.style.opacity)
        );
        for shape in &op.shapes {
            element(&mut result, shape, &op.style);
        }
        result.push_str("  </g>\n");
    }
    result.push_str("</svg>\n");
    result
}

fn element(output: &mut String, shape: &Shape, style: &Style) {
    match shape {
        Shape::Circle {
            center,
            radius,
            disk,
        } => {
            let _ = writeln!(
                output,
                "    <circle cx=\"{}\" cy=\"{}\" r=\"{}\" {}/>",
                format_float(center.x),
                format_float(center.y),
                format_float(*radius),
                attributes(style, true, *disk)
            );
        }
        Shape::Ellipse {
            center,
            radius_x,
            radius_y,
            rotation,
        } => {
            let degrees = format_float(rotation * 180.0 / core::f64::consts::PI);
            let _ = writeln!(
                output,
                "    <ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" transform=\"rotate({degrees} {} {})\" {}/>",
                format_float(center.x),
                format_float(center.y),
                format_float(*radius_x),
                format_float(*radius_y),
                format_float(center.x),
                format_float(center.y),
                attributes(style, true, false)
            );
        }
        Shape::RoundedRect { min, max, radius } => {
            let _ = writeln!(
                output,
                "    <rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" {}/>",
                format_float(min.x),
                format_float(min.y),
                format_float(max.x - min.x),
                format_float(max.y - min.y),
                format_float(*radius),
                attributes(style, true, false)
            );
        }
        Shape::Path {
            commands,
            closed,
            ink,
        } => {
            let _ = writeln!(
                output,
                "    <path d=\"{}\" {}/>",
                path_data(commands),
                attributes(style, *closed, *ink)
            );
        }
        Shape::Arc {
            start,
            end,
            radius,
            sweep,
            ..
        } => {
            let large = u8::from(sweep.abs() > core::f64::consts::PI);
            let forward = u8::from(*sweep > 0.0);
            let r = format_float(*radius);
            let _ = writeln!(
                output,
                "    <path d=\"M {} A {r} {r} 0 {large} {forward} {}\" {}/>",
                pair(*start),
                pair(*end),
                attributes(style, false, false)
            );
        }
    }
}

pub(crate) fn pair(point: Point) -> String {
    format!("{} {}", format_float(point.x), format_float(point.y))
}

fn path_data(commands: &[Command]) -> String {
    commands
        .iter()
        .map(|command| match command {
            Command::Move(p) => format!("M {}", pair(*p)),
            Command::Line(p) => format!("L {}", pair(*p)),
            Command::Cubic(a, b, c) => format!("C {} {} {}", pair(*a), pair(*b), pair(*c)),
            Command::Close => "Z".into(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn attributes(style: &Style, closed: bool, ink: bool) -> String {
    if ink {
        return format!(
            "fill=\"{}\" stroke=\"none\"{}",
            style.accent.hex(),
            alpha_attribute("fill", style.accent)
        );
    }
    let fill = if closed {
        style
            .fill
            .map_or_else(|| "none".into(), crate::style::Color::hex)
    } else {
        "none".into()
    };
    let mut result = format!(
        "fill=\"{fill}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"{}\" stroke-linejoin=\"{}\"",
        style.accent.hex(),
        format_float(style.weight),
        style.cap,
        style.join
    );
    result.push_str(&alpha_attribute("stroke", style.accent));
    if closed {
        if let Some(color) = style.fill {
            result.push_str(&alpha_attribute("fill", color));
        }
    }
    if !style.dash.is_empty() {
        let _ = write!(
            result,
            " stroke-dasharray=\"{}\"",
            style
                .dash
                .iter()
                .map(|v| format_float(*v))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    result
}

fn alpha_attribute(channel: &str, color: Color) -> String {
    if color.3 == 255 {
        String::new()
    } else {
        format!(" {channel}-opacity=\"{}\"", format_float(color.opacity()))
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::format_float;
    #[test]
    fn fixed_precision_and_decimal_carry() {
        for (value, expected) in [
            (1.234_567_4, "1.234567"),
            (1.234_567_5, "1.234568"),
            (-1.234_567_5, "-1.234568"),
            (9.999_999_9, "10.000000"),
            (42.0, "42.000000"),
        ] {
            assert_eq!(format_float(value), expected);
        }
    }
    #[test]
    fn signed_zero_and_subnormal() {
        for value in [
            0.0,
            -0.0,
            -0.000_000_4,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
        ] {
            assert_eq!(format_float(value), "0.000000");
        }
        assert_eq!(format_float(-0.000_000_5), "-0.000001");
    }
    #[test]
    fn large_and_small_magnitudes_without_exponents_or_overflow() {
        assert_eq!(format_float(1e20), "100000000000000000000.000000");
        assert_eq!(format_float(1e-5), "0.000010");
        assert_eq!(format_float(1.2345e-4), "0.000123");
        let large = format_float(f64::MAX);
        assert!(large.starts_with("17976931348623157"));
        assert_eq!(large.len(), 316);
        assert!(large.ends_with(".000000"));
    }
}
