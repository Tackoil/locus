//! Canonical whole-document formatting through `locus-syntax`.

use locus_syntax::format;
use lsp_types::{DocumentFormattingParams, TextEdit};

use crate::state::State;

pub(crate) fn handle(state: &State, params: &DocumentFormattingParams) -> Option<Vec<TextEdit>> {
    let document = state.document(&params.text_document.uri)?;
    let formatted = format(document.text());
    (formatted.source != document.text()).then(|| {
        vec![TextEdit {
            range: document.full_range(),
            new_text: formatted.source,
        }]
    })
}
