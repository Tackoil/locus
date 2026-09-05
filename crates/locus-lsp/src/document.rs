//! Versioned document text and the sole LSP UTF-16/byte conversion boundary.

use locus_syntax::Span;
use lsp_types::{Position, Range, TextDocumentContentChangeEvent, Uri};

#[derive(Clone, Debug)]
pub(crate) struct Document {
    uri: Uri,
    version: i32,
    text: String,
    line_starts: Vec<usize>,
}

impl Document {
    pub(crate) fn new(uri: Uri, version: i32, text: String) -> Self {
        let line_starts = line_starts(&text);
        Self {
            uri,
            version,
            text,
            line_starts,
        }
    }

    pub(crate) const fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(crate) const fn version(&self) -> i32 {
        self.version
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn apply_changes(
        &mut self,
        version: i32,
        changes: &[TextDocumentContentChangeEvent],
    ) -> bool {
        if version <= self.version {
            return false;
        }
        for change in changes {
            if let Some(range) = change.range {
                let Some(start) = self.position_to_byte(range.start) else {
                    return false;
                };
                let Some(end) = self.position_to_byte(range.end) else {
                    return false;
                };
                if start > end {
                    return false;
                }
                self.text.replace_range(start..end, &change.text);
                self.line_starts = line_starts(&self.text);
            } else {
                self.text.clone_from(&change.text);
                self.line_starts = line_starts(&self.text);
            }
        }
        self.version = version;
        true
    }

    pub(crate) fn position_to_byte(&self, position: Position) -> Option<usize> {
        let line = usize::try_from(position.line).ok()?;
        let start = *self.line_starts.get(line)?;
        let end = self.line_content_end(line);
        let target = position.character;
        let mut utf16 = 0_u32;
        for (offset, character) in self.text[start..end].char_indices() {
            if utf16 == target {
                return Some(start + offset);
            }
            utf16 = utf16.saturating_add(utf16_len(character));
            if utf16 > target {
                return None;
            }
        }
        (utf16 == target).then_some(end)
    }

    pub(crate) fn byte_to_position(&self, byte: usize) -> Position {
        let byte = floor_char_boundary(&self.text, byte.min(self.text.len()));
        let line = self.line_for_byte(byte);
        let start = self.line_starts[line];
        let character = self.text[start..byte]
            .chars()
            .fold(0_u32, |total, value| total.saturating_add(utf16_len(value)));
        let line = u32::try_from(line).map_or(u32::MAX, |value| value);
        Position::new(line, character)
    }

    pub(crate) fn span_to_range(&self, span: Span) -> Range {
        Range::new(
            self.byte_to_position(span.byte_start),
            self.byte_to_position(span.byte_end),
        )
    }

    pub(crate) fn full_range(&self) -> Range {
        Range::new(Position::new(0, 0), self.byte_to_position(self.text.len()))
    }

    pub(crate) fn line_start_at(&self, byte: usize) -> usize {
        self.line_starts[self.line_for_byte(byte.min(self.text.len()))]
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub(crate) fn line_text(&self, line: usize) -> Option<&str> {
        let start = *self.line_starts.get(line)?;
        Some(&self.text[start..self.line_content_end(line)])
    }

    pub(crate) fn source_for_span(&self, span: Span) -> &str {
        let start = floor_char_boundary(&self.text, span.byte_start.min(self.text.len()));
        let end = floor_char_boundary(&self.text, span.byte_end.min(self.text.len()));
        self.text.get(start..end).unwrap_or_default()
    }

    fn line_content_end(&self, line: usize) -> usize {
        let start = self.line_starts[line];
        let mut end = self
            .line_starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());
        if end > start && self.text.as_bytes()[end - 1] == b'\n' {
            end -= 1;
        }
        if end > start && self.text.as_bytes()[end - 1] == b'\r' {
            end -= 1;
        }
        end
    }

    fn line_for_byte(&self, byte: usize) -> usize {
        self.line_starts
            .partition_point(|start| *start <= byte)
            .saturating_sub(1)
    }
}

fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let newline_width = match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => 2,
            b'\r' | b'\n' => 1,
            _ => 0,
        };
        index += newline_width.max(1);
        if newline_width > 0 {
            starts.push(index);
        }
    }
    starts
}

fn floor_char_boundary(text: &str, mut byte: usize) -> usize {
    while !text.is_char_boundary(byte) {
        byte = byte.saturating_sub(1);
    }
    byte
}

const fn utf16_len(character: char) -> u32 {
    if character.len_utf16() == 1 { 1 } else { 2 }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use super::*;

    #[test]
    fn converts_utf16_around_non_bmp_characters() {
        let Ok(uri) = Uri::from_str("file:///utf16.locus") else {
            return;
        };
        let document = Document::new(uri, 1, "a😀b\n".to_owned());
        assert_eq!(document.position_to_byte(Position::new(0, 3)), Some(5));
        assert_eq!(document.byte_to_position(5), Position::new(0, 3));
        assert_eq!(document.position_to_byte(Position::new(0, 2)), None);
    }

    #[test]
    fn indexes_all_supported_line_endings() {
        let Ok(uri) = Uri::from_str("file:///line-endings.locus") else {
            return;
        };
        let document = Document::new(uri, 1, "a\rb\r\nc\n".to_owned());
        assert_eq!(document.position_to_byte(Position::new(1, 0)), Some(2));
        assert_eq!(document.position_to_byte(Position::new(1, 1)), Some(3));
        assert_eq!(document.position_to_byte(Position::new(2, 0)), Some(5));
        assert_eq!(document.byte_to_position(5), Position::new(2, 0));
        assert_eq!(document.line_text(1), Some("b"));
    }
}
