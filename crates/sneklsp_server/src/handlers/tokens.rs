use lsp_types::Uri;
use rustc_hash::FxHashMap;

use crate::server::DocumentState;

pub fn handle_semantic_tokens(
    params: lsp_types::SemanticTokensParams,
    documents: &FxHashMap<Uri, DocumentState>,
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
    documents: &FxHashMap<Uri, DocumentState>,
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
