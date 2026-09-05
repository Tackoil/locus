//! File-local go-to-definition.

use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location};

use super::target::{Target, at, class_definition, object_definition};
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
        Target::Object(id) => object_definition(&analysis.file, &id.name)?.id.span,
        Target::Class(id) => class_definition(&analysis.file, &id.name)?.name.span,
        Target::Callable(_) => return None,
    };
    Some(GotoDefinitionResponse::Scalar(Location::new(
        uri.clone(),
        document.span_to_range(span),
    )))
}
