//! Open-document snapshots and version-gated analysis results.

use std::collections::BTreeMap;

use locus_core::{Evaluation, evaluate};
use locus_syntax::{File, Statement, parse};
use lsp_types::{ClientCapabilities, Diagnostic, MarkupKind, TextDocumentContentChangeEvent, Uri};

use crate::diagnostics;
use crate::document::Document;

#[derive(Clone, Debug)]
pub(crate) struct Analysis {
    uri: Uri,
    version: i32,
    pub(crate) file: File,
    pub(crate) evaluation: Option<Evaluation>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl Analysis {
    pub(crate) fn new(document: &Document) -> Self {
        let parsed = parse(document.text());
        let mut lsp_diagnostics = parsed
            .diagnostics
            .iter()
            .map(|value| diagnostics::syntax(document, value))
            .collect::<Vec<_>>();
        let prefix = evaluable_prefix(&parsed.file, &parsed.diagnostics);
        let evaluation = if prefix.version == 2 {
            match evaluate(&prefix) {
                Ok(value) => Some(value),
                Err(error) => {
                    lsp_diagnostics.push(diagnostics::evaluation(document, &error));
                    None
                }
            }
        } else {
            None
        };
        lsp_diagnostics.sort_by(|left, right| {
            (left.range.start.line, left.range.start.character)
                .cmp(&(right.range.start.line, right.range.start.character))
                .then_with(|| diagnostic_code(left).cmp(diagnostic_code(right)))
        });
        Self {
            uri: document.uri().clone(),
            version: document.version(),
            file: parsed.file,
            evaluation,
            diagnostics: lsp_diagnostics,
        }
    }

    pub(crate) const fn uri(&self) -> &Uri {
        &self.uri
    }

    pub(crate) const fn version(&self) -> i32 {
        self.version
    }
}

#[derive(Clone, Debug)]
struct OpenDocument {
    document: Document,
    analysis: Analysis,
}

#[derive(Default)]
pub(crate) struct State {
    documents: BTreeMap<Uri, OpenDocument>,
    client: ClientPreferences,
}

#[derive(Clone, Debug)]
struct ClientPreferences {
    hierarchical_symbols: bool,
    hover_markup: MarkupKind,
}

impl Default for ClientPreferences {
    fn default() -> Self {
        Self {
            hierarchical_symbols: false,
            hover_markup: MarkupKind::PlainText,
        }
    }
}

impl State {
    pub(crate) fn with_client_capabilities(capabilities: &ClientCapabilities) -> Self {
        let text = capabilities.text_document.as_ref();
        let hierarchical_symbols = text
            .and_then(|value| value.document_symbol.as_ref())
            .and_then(|value| value.hierarchical_document_symbol_support)
            .unwrap_or(false);
        let hover_markup = text
            .and_then(|value| value.hover.as_ref())
            .and_then(|value| value.content_format.as_ref())
            .and_then(|formats| formats.first())
            .cloned()
            .unwrap_or(MarkupKind::PlainText);
        Self {
            documents: BTreeMap::new(),
            client: ClientPreferences {
                hierarchical_symbols,
                hover_markup,
            },
        }
    }

    pub(crate) fn open(&mut self, uri: Uri, version: i32, text: String) -> Analysis {
        let document = Document::new(uri.clone(), version, text);
        let analysis = Analysis::new(&document);
        self.documents.insert(
            uri,
            OpenDocument {
                document,
                analysis: analysis.clone(),
            },
        );
        analysis
    }

    pub(crate) fn change(
        &mut self,
        uri: &Uri,
        version: i32,
        changes: &[TextDocumentContentChangeEvent],
    ) -> bool {
        let Some(open) = self.documents.get_mut(uri) else {
            return false;
        };
        if !open.document.apply_changes(version, changes) {
            return false;
        }
        true
    }

    pub(crate) fn analyze_current(&mut self, uri: &Uri) -> Option<Analysis> {
        let open = self.documents.get_mut(uri)?;
        let analysis = Analysis::new(&open.document);
        open.analysis = analysis.clone();
        Some(analysis)
    }

    pub(crate) fn close(&mut self, uri: &Uri) -> Option<Document> {
        self.documents.remove(uri).map(|open| open.document)
    }

    pub(crate) fn document(&self, uri: &Uri) -> Option<&Document> {
        self.documents.get(uri).map(|open| &open.document)
    }

    pub(crate) fn analysis(&self, uri: &Uri) -> Option<&Analysis> {
        self.documents.get(uri).map(|open| &open.analysis)
    }

    pub(crate) fn is_current(&self, analysis: &Analysis) -> bool {
        self.document(analysis.uri())
            .is_some_and(|document| document.version() == analysis.version())
    }

    pub(crate) const fn hierarchical_symbols(&self) -> bool {
        self.client.hierarchical_symbols
    }

    pub(crate) const fn hover_markup(&self) -> &MarkupKind {
        &self.client.hover_markup
    }
}

fn evaluable_prefix(file: &File, syntax_diagnostics: &[locus_syntax::Diagnostic]) -> File {
    let cutoff = syntax_diagnostics
        .iter()
        .map(|diagnostic| diagnostic.span.byte_start)
        .min()
        .unwrap_or(file.span.byte_end);
    let mut prefix = file.clone();
    prefix
        .statements
        .retain(|statement| statement.span().byte_end <= cutoff);
    prefix.span.byte_end = cutoff;
    prefix
}

fn diagnostic_code(value: &Diagnostic) -> &str {
    match value.code.as_ref() {
        Some(lsp_types::NumberOrString::String(code)) => code,
        Some(lsp_types::NumberOrString::Number(_)) | None => "",
    }
}

pub(crate) fn resolved_scene(evaluation: &Evaluation) -> &locus_core::ResolvedScene {
    match evaluation {
        Evaluation::Resolved(scene) | Evaluation::Solved { scene, .. } => scene,
    }
}

pub(crate) fn defined_ops(file: &File) -> impl Iterator<Item = &locus_syntax::OpStmt> {
    file.statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Op(op) => Some(op),
            _ => None,
        })
}
