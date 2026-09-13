//! The textbook palette and source-ordered semantic class resolution.

use crate::RenderError;
use indexmap::IndexMap;
use locus_syntax::{File, Statement};
mod values;
use values::resolve;

/// Concrete sRGB color, independent of backend color names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub u8, pub u8, pub u8);

impl Color {
    /// Uppercase six-digit CSS color.
    #[must_use]
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.0, self.1, self.2)
    }

    fn tint(self) -> Self {
        fn channel(value: u8) -> u8 {
            u8::try_from((u16::from(value) + 9 * 255) / 10).unwrap_or(255)
        }
        Self(channel(self.0), channel(self.1), channel(self.2))
    }
}

/// Fully resolved op style. Lengths are CSS pixels, not scene units.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// Stroke, point disk, and text color.
    pub accent: Color,
    /// Closed-geometry fill; ignored by open paths.
    pub fill: Option<Color>,
    /// Stroke width in CSS pixels.
    pub weight: f64,
    /// Even-length dash array in CSS pixels; empty means solid.
    pub dash: Vec<f64>,
    /// Whole-op opacity.
    pub opacity: f64,
    /// SVG/TikZ line-cap token.
    pub cap: &'static str,
    /// SVG/TikZ line-join token.
    pub join: &'static str,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            accent: Color(17, 24, 39),
            fill: None,
            weight: 1.25,
            dash: Vec::new(),
            opacity: 1.0,
            cap: "butt",
            join: "miter",
        }
    }
}

/// Textbook classes and op assignments in source declaration order.
#[derive(Clone, Debug)]
pub struct StyleSheet {
    classes: IndexMap<String, Style>,
    objects: IndexMap<String, Style>,
}

impl Default for StyleSheet {
    fn default() -> Self {
        let main = Style::default();
        let classes = IndexMap::from([
            ("main".into(), main.clone()),
            (
                "auxiliary".into(),
                Style {
                    weight: 0.75,
                    dash: vec![5.0, 3.0],
                    ..main.clone()
                },
            ),
            (
                "construction".into(),
                Style {
                    accent: Color(107, 114, 128),
                    weight: 0.75,
                    dash: vec![1.0, 3.0],
                    ..main.clone()
                },
            ),
            (
                "highlight".into(),
                Style {
                    accent: Color(37, 99, 235),
                    fill: Some(Color(37, 99, 235).tint()),
                    ..main
                },
            ),
        ]);
        Self {
            classes,
            objects: IndexMap::new(),
        }
    }
}

impl StyleSheet {
    /// Resolves declarations before use, validating exact class values.
    ///
    /// # Errors
    /// Returns E415 with the offending source span for invalid styles or classes.
    pub fn from_file(file: &File) -> Result<Self, RenderError> {
        let mut sheet = Self::default();
        for statement in &file.statements {
            sheet.statement(statement)?;
        }
        Ok(sheet)
    }

    fn statement(&mut self, statement: &Statement) -> Result<(), RenderError> {
        match statement {
            Statement::Style(style) if style.name.name != "textbook" => {
                return Err(RenderError::invalid(
                    style.name.span,
                    "unknown preset; expected textbook",
                ));
            }
            Statement::Class(class) => {
                if self.classes.contains_key(&class.name.name) {
                    return Err(RenderError::invalid(
                        class.name.span,
                        "duplicate or reserved class name",
                    ));
                }
                self.classes
                    .insert(class.name.name.clone(), resolve(&class.props.items)?);
            }
            Statement::Op(op) => {
                let style = if let Some(attr) = &op.attr {
                    self.classes
                        .get(&attr.name.name)
                        .ok_or_else(|| RenderError::invalid(attr.span, "unknown style class"))?
                        .clone()
                } else {
                    Style::default()
                };
                self.objects.insert(op.id.name.clone(), style);
            }
            _ => {}
        }
        Ok(())
    }

    /// Resolves a class by name, including the four built-ins.
    #[must_use]
    pub fn class(&self, name: &str) -> Option<&Style> {
        self.classes.get(name)
    }

    /// Concrete style for an object; absent assignments use `main`.
    #[must_use]
    pub fn for_object(&self, name: &str) -> Style {
        self.objects.get(name).cloned().unwrap_or_default()
    }
}
