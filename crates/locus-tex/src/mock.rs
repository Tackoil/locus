//! Stable renderer for cross-crate golden tests.

use crate::engine::{BoxMetrics, FontSize, TexBox, TexEngine, TexError, escape_xml, format_number};

/// A deterministic renderer with a fixed character-advance table.
#[derive(Clone, Copy, Debug, Default)]
pub struct MockEngine;

impl TexEngine for MockEngine {
    fn render(&self, tex: &str, size: FontSize) -> Result<TexBox, TexError> {
        let size = size.validate()?;
        let width = tex.chars().map(character_advance).sum::<f64>() * size;
        let metrics = BoxMetrics::new(width, 0.8 * size, 0.2 * size);
        let mut rendered = TexBox::new(String::new(), metrics);
        rendered.validate()?;
        rendered.svg_body = format!(
            "<g data-engine=\"mock\" data-source=\"{}\" data-width=\"{}\"/>",
            escape_xml(tex)?,
            format_number(width),
        );
        Ok(rendered)
    }
}

fn character_advance(character: char) -> f64 {
    match character {
        ' ' => 0.25,
        'i' | 'j' | 'l' | 'I' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' => 0.30,
        'm' | 'w' | 'M' | 'W' | '@' => 0.90,
        'A'..='Z' => 0.70,
        'a'..='z' => 0.55,
        '0'..='9' => 0.60,
        _ => 0.75,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_table_is_stable() -> Result<(), TexError> {
        let first = MockEngine.render("Mi 2", FontSize::DEFAULT);
        let second = MockEngine.render("Mi 2", FontSize::DEFAULT);

        assert_eq!(first, second);
        assert_eq!(first?.metrics, BoxMetrics::new(2.05, 0.8, 0.2));
        Ok(())
    }

    #[test]
    fn overflowing_dimensions_are_rejected() {
        let result = MockEngine.render("WW", FontSize::new(f64::MAX));

        assert!(matches!(result, Err(TexError::InvalidMetrics { .. })));
    }
}
