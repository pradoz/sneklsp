use std::collections::HashMap;

use lsp_types::Uri;

use crate::server::DocumentState;

pub fn handle_semantic_tokens(
    params: lsp_types::SemanticTokensParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<lsp_types::SemanticTokensResult> {
    let uri = params.text_document.uri;
    let state = documents.get(&uri)?;
    let index = state.document.index.as_ref()?;
    Some(crate::semantic_tokens::compute_semantic_tokens(
        index,
        &state.document.line_index,
    ))
}

pub fn handle_semantic_tokens_range(
    params: lsp_types::SemanticTokensRangeParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<lsp_types::SemanticTokensResult> {
    let uri = params.text_document.uri;
    let state = documents.get(&uri)?;
    let index = state.document.index.as_ref()?;
    Some(crate::semantic_tokens::compute_semantic_tokens_range(
        index,
        &state.document.line_index,
        params.range,
    ))
}
