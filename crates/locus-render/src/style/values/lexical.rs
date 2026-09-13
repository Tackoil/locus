//! Scalar, token, dash, and color decoding for class declarations.
use crate::{RenderError, style::Color};
use locus_syntax::{Expr, Prop, PropValue, Span};
pub(super) fn invalid(prop: &Prop) -> RenderError {
    RenderError::invalid(
        prop.value.span(),
        format!("invalid {} style value", prop.name.name),
    )
}

pub(super) fn token(value: &PropValue) -> Option<&str> {
    if let PropValue::Expr(Expr::Ref(id)) = value {
        Some(&id.name)
    } else {
        None
    }
}

pub(super) fn number(value: &PropValue) -> Option<f64> {
    if let PropValue::Expr(Expr::Number(n)) = value {
        n.unit.is_none().then_some(n.value)
    } else {
        None
    }
}

pub(super) fn positive(prop: &Prop) -> Result<f64, RenderError> {
    number(&prop.value)
        .filter(|n| n.is_finite() && *n > 0.0)
        .ok_or_else(|| invalid(prop))
}

pub(super) fn choice(prop: &Prop, choices: &[&'static str]) -> Result<&'static str, RenderError> {
    choices
        .iter()
        .copied()
        .find(|v| Some(*v) == token(&prop.value))
        .ok_or_else(|| invalid(prop))
}

pub(super) fn dash(prop: &Prop) -> Result<Vec<f64>, RenderError> {
    match token(&prop.value) {
        Some("solid") => return Ok(Vec::new()),
        Some("dashed") => return Ok(vec![5.0, 3.0]),
        Some("dotted") => return Ok(vec![1.0, 3.0]),
        _ => {}
    }
    let PropValue::Expr(Expr::Call { name, args, .. }) = &prop.value else {
        return Err(invalid(prop));
    };
    if name.name != "dash" || args.len() < 2 {
        return Err(invalid(prop));
    }
    let mut values = args
        .iter()
        .map(|expr| {
            if let Expr::Number(n) = expr {
                if n.unit.is_none() && n.value.is_finite() && n.value > 0.0 {
                    return Ok(n.value);
                }
            }
            Err(RenderError::invalid(
                expr.span(),
                "dash lengths must be positive finite CSS pixels",
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() % 2 == 1 {
        values.extend_from_within(..);
    }
    Ok(values)
}

pub(super) fn color(prop: &Prop) -> Result<Color, RenderError> {
    const PALETTE: [(&str, Color); 8] = [
        ("ink", Color(17, 24, 39)),
        ("gray", Color(107, 114, 128)),
        ("blue", Color(37, 99, 235)),
        ("orange", Color(234, 88, 12)),
        ("teal", Color(15, 118, 110)),
        ("green", Color(22, 163, 74)),
        ("red", Color(220, 38, 38)),
        ("purple", Color(147, 51, 234)),
    ];
    if let PropValue::HexColor(hex) = &prop.value {
        return parse_hex(&hex.value, hex.span);
    }
    PALETTE
        .iter()
        .find(|(name, _)| Some(*name) == token(&prop.value))
        .map(|(_, color)| *color)
        .ok_or_else(|| invalid(prop))
}

fn parse_hex(value: &str, span: Span) -> Result<Color, RenderError> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(RenderError::invalid(
            span,
            "expected a six-digit sRGB color",
        ));
    }
    let number =
        u32::from_str_radix(digits, 16).map_err(|_| RenderError::invalid(span, "invalid color"))?;
    let bytes = number.to_be_bytes();
    Ok(Color(bytes[1], bytes[2], bytes[3]))
}
