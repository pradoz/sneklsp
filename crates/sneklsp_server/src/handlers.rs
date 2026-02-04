use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    GotoDefinitionResponse, Location, Position, Range, ReferenceParams, RenameParams, SymbolKind,
    TextEdit, Uri, WorkspaceEdit,
};
use std::collections::HashMap;

use crate::background::{IndexedModule, IndexedSymbol};
use sneklsp_text::{LineIndex, TextRange};

fn to_document_symbol(
    symbol: &IndexedSymbol,
    index: &IndexedModule,
    line_index: &LineIndex,
) -> DocumentSymbol {
    let children = find_symbol_children(symbol, index, line_index);

    #[allow(deprecated)]
    DocumentSymbol {
        name: symbol.name.clone(),
        detail: None,
        kind: to_lsp_symbol_kind(symbol.kind),
        tags: None,
        deprecated: None,
        range: to_lsp_range(symbol.range, line_index),
        selection_range: to_lsp_range(symbol.selection_range, line_index),
        children: if children.is_empty() {
            None
        } else {
            Some(children)
        },
    }
}

fn find_symbol_children(
    parent: &IndexedSymbol,
    index: &IndexedModule,
    line_index: &LineIndex,
) -> Vec<DocumentSymbol> {
    let mut children = Vec::new();

    match parent.kind {
        sneklsp_index::SymbolKind::Function
        | sneklsp_index::SymbolKind::Class
        | sneklsp_index::SymbolKind::Method => {
            // find child scope that matches this symbol range
            for scope in &index.scopes {
                if scope.parent == Some(parent.scope) && scope.range == parent.range {
                    for &sym_id in &scope.symbols {
                        if let Some(sym) = index.symbols.iter().find(|s| s.id == sym_id) {
                            children.push(to_document_symbol(sym, index, line_index));
                        }
                    }
                }
            }
        }
        _ => {}
    }

    children
}

pub fn handle_document_symbol(
    params: DocumentSymbolParams,
    documents: &HashMap<Uri, crate::server::DocumentState>,
) -> Option<DocumentSymbolResponse> {
    let uri = params.text_document.uri;
    let state = documents.get(&uri)?;
    let index = state.document.index.as_ref()?;

    let mut symbols = Vec::new();

    let root_scope = index.scopes.first()?;
    for &sym_id in &root_scope.symbols {
        if let Some(symbol) = index.symbols.iter().find(|s| s.id == sym_id) {
            symbols.push(to_document_symbol(
                symbol,
                index,
                &state.document.line_index,
            ));
        }
    }

    Some(DocumentSymbolResponse::Nested(symbols))
}

pub fn handle_goto_definition(
    params: GotoDefinitionParams,
    documents: &HashMap<Uri, crate::server::DocumentState>,
) -> Option<GotoDefinitionResponse> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let state = documents.get(&uri)?;
    let index = state.document.index.as_ref()?;
    let offset = state.document.line_index.offset(sneklsp_text::Position {
        line: pos.line,
        column: pos.character,
    })?;

    // might already be on a definition
    for symbol in &index.symbols {
        if symbol.selection_range.contains(offset) {
            let range = to_lsp_range(symbol.selection_range, &state.document.line_index);
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range,
            }));
        }
    }

    // check if its a reference
    for reference in &index.references {
        if reference.range.contains(offset) {
            if let Some(sym_id) = reference.resolved {
                if let Some(symbol) = index.symbols.iter().find(|s| s.id == sym_id) {
                    let range = to_lsp_range(symbol.range, &state.document.line_index);
                    return Some(GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range,
                    }));
                }
            }
            break;
        }
    }

    None
}

pub fn handle_references(
    params: ReferenceParams,
    documents: &HashMap<Uri, crate::server::DocumentState>,
) -> Option<Vec<Location>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let include_decl = params.context.include_declaration;

    let state = documents.get(&uri)?;
    let index = state.document.index.as_ref()?;
    let offset = state.document.line_index.offset(sneklsp_text::Position {
        line: pos.line,
        column: pos.character,
    })?;

    let symbol_id = find_symbol_at_position(offset, index)?;

    let mut locations = Vec::new();

    if include_decl {
        if let Some(symbol) = index.symbols.iter().find(|s| s.id == symbol_id) {
            locations.push(Location {
                uri: uri.clone(),
                range: to_lsp_range(symbol.range, &state.document.line_index),
            });
        }
    }

    // add all references
    for reference in &index.references {
        if reference.resolved == Some(symbol_id) {
            locations.push(Location {
                uri: uri.clone(),
                range: to_lsp_range(reference.range, &state.document.line_index),
            });
        }
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

pub fn handle_rename(
    params: RenameParams,
    documents: &HashMap<Uri, crate::server::DocumentState>,
) -> Option<WorkspaceEdit> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name;

    let state = documents.get(&uri)?;
    let index = state.document.index.as_ref()?;
    let offset = state.document.line_index.offset(sneklsp_text::Position {
        line: pos.line,
        column: pos.character,
    })?;

    let symbol_id = find_symbol_at_position(offset, index)?;
    let mut edits = Vec::new();

    if let Some(symbol) = index.symbols.iter().find(|s| s.id == symbol_id) {
        edits.push(TextEdit {
            range: to_lsp_range(symbol.selection_range, &state.document.line_index),
            new_text: new_name.clone(),
        });
    }

    for reference in &index.references {
        if reference.resolved == Some(symbol_id) {
            edits.push(TextEdit {
                range: to_lsp_range(reference.range, &state.document.line_index),
                new_text: new_name.clone(),
            });
        }
    }

    if edits.is_empty() {
        return None;
    }

    let mut changes = HashMap::new();
    changes.insert(uri, edits);

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

pub fn handle_document_highlight(
    params: lsp_types::DocumentHighlightParams,
    documents: &HashMap<Uri, crate::server::DocumentState>,
) -> Option<Vec<lsp_types::DocumentHighlight>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let state = documents.get(&uri)?;
    let index = state.document.index.as_ref()?;

    let offset = state.document.line_index.offset(sneklsp_text::Position {
        line: pos.line,
        column: pos.character,
    })?;

    let symbol_id = find_symbol_at_position(offset, index)?;
    let mut highlights = Vec::new();

    if let Some(symbol) = index.symbols.iter().find(|s| s.id == symbol_id) {
        highlights.push(lsp_types::DocumentHighlight {
            range: to_lsp_range(symbol.selection_range, &state.document.line_index),
            kind: Some(lsp_types::DocumentHighlightKind::WRITE),
        });
    }

    for reference in &index.references {
        if reference.resolved == Some(symbol_id) {
            highlights.push(lsp_types::DocumentHighlight {
                range: to_lsp_range(reference.range, &state.document.line_index),
                kind: Some(lsp_types::DocumentHighlightKind::READ),
            });
        }
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

// utility
fn find_symbol_at_position(offset: sneklsp_text::TextSize, index: &IndexedModule) -> Option<u32> {
    // check definitions first
    for symbol in &index.symbols {
        if symbol.selection_range.contains(offset) {
            return Some(symbol.id);
        }
    }

    // check references
    for reference in &index.references {
        if reference.range.contains(offset) {
            return reference.resolved;
        }
    }

    None
}

fn to_lsp_range(range: TextRange, line_index: &LineIndex) -> Range {
    let start = line_index.position(range.start());
    let end = line_index.position(range.end());

    Range {
        start: Position {
            line: start.line,
            character: start.column,
        },
        end: Position {
            line: end.line,
            character: end.column,
        },
    }
}

fn to_lsp_symbol_kind(kind: sneklsp_index::SymbolKind) -> SymbolKind {
    match kind {
        sneklsp_index::SymbolKind::Function => SymbolKind::FUNCTION,
        sneklsp_index::SymbolKind::Class => SymbolKind::CLASS,
        sneklsp_index::SymbolKind::Variable => SymbolKind::VARIABLE,
        sneklsp_index::SymbolKind::Parameter => SymbolKind::VARIABLE,
        sneklsp_index::SymbolKind::Import => SymbolKind::MODULE,
        sneklsp_index::SymbolKind::ImportedSymbol => SymbolKind::VARIABLE,
        sneklsp_index::SymbolKind::Method => SymbolKind::METHOD,
        sneklsp_index::SymbolKind::Property => SymbolKind::PROPERTY,
        sneklsp_index::SymbolKind::TypeAlias => SymbolKind::TYPE_PARAMETER,
    }
}
