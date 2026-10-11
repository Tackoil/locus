//! File-local go-to-definition.

use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location};

use super::scope;
use super::target::{Target, at, class_definition, named_definition};
use crate::state::State;

pub(crate) fn handle(
    state: &State,
    params: &GotoDefinitionParams,
) -> Option<GotoDefinitionResponse> {
    let uri = &params.text_document_position_params.text_document.uri;
    let document = state.document(uri)?;
    let analysis = state.analysis(uri)?;
    let byte = document.position_to_byte(params.text_document_position_params.position)?;
    let span = match at(&analysis.file, byte)? {
        Target::Object(id) => scope::resolve(&analysis.file, &id.name, byte)?.id().span,
        Target::Member(expr, _) => {
            scope::resolve(&analysis.file, &scope::qualified_name(expr)?, byte)?
                .id()
                .span
        }
        Target::Class(id) => scope::resolve(&analysis.file, &id.name, byte)
            .filter(|value| matches!(value, scope::Binding::Parameter(_)))
            .map(|value| value.id().span)
            .or_else(|| class_definition(&analysis.file, &id.name).map(|class| class.name.span))?,
        Target::Callable(id) => named_definition(&analysis.file, &id.name)?.span,
    };
    Some(GotoDefinitionResponse::Scalar(Location::new(
        uri.clone(),
        document.span_to_range(span),
    )))
}
