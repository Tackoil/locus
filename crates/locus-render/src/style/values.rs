//! Validation and concrete values for source class properties.
use super::Style;
use crate::RenderError;
use locus_syntax::Prop;
mod lexical;
use lexical::{choice, color, dash, invalid, number, positive, token};

pub(super) fn resolve(props: &[Prop]) -> Result<Style, RenderError> {
    let mut result = Style::default();
    let mut fill = None;
    for prop in props {
        if prop.name.name == "fill" {
            fill = Some(prop);
        } else {
            apply(&mut result, prop)?;
        }
    }
    if let Some(prop) = fill {
        result.fill = match token(&prop.value) {
            Some("none") => None,
            Some("tint") => Some(result.accent.tint()),
            _ => Some(color(prop)?),
        };
    }
    Ok(result)
}

fn apply(style: &mut Style, prop: &Prop) -> Result<(), RenderError> {
    match prop.name.name.as_str() {
        "accent" => style.accent = color(prop)?,
        "weight" => style.weight = weight(prop)?,
        "opacity" => style.opacity = opacity(prop)?,
        _ => apply_stroke(style, prop)?,
    }
    Ok(())
}

fn weight(prop: &Prop) -> Result<f64, RenderError> {
    match token(&prop.value) {
        Some("thin") => Ok(0.75),
        Some("normal") => Ok(1.25),
        Some("bold") => Ok(2.0),
        _ => positive(prop),
    }
}

fn opacity(prop: &Prop) -> Result<f64, RenderError> {
    number(&prop.value)
        .filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
        .ok_or_else(|| invalid(prop))
}

fn apply_stroke(style: &mut Style, prop: &Prop) -> Result<(), RenderError> {
    match prop.name.name.as_str() {
        "stroke" => style.dash = dash(prop)?,
        "cap" => style.cap = choice(prop, &["butt", "round", "square"])?,
        "join" => style.join = choice(prop, &["miter", "round", "bevel"])?,
        _ => return Err(invalid(prop)),
    }
    Ok(())
}
