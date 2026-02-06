use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DocumentSymbol,
    DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams, GotoDefinitionResponse,
    Location, Position, Range, ReferenceParams, RenameParams, SymbolKind, TextEdit, Uri,
    WorkspaceEdit,
};
use std::collections::{HashMap, HashSet};

use crate::builtins::BUILTINS;
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, SymbolData};
use sneklsp_text::{LineIndex, TextRange, TextSize};

#[inline]
pub fn to_lsp_range(range: TextRange, line_index: &LineIndex) -> Range {
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

#[inline]
pub fn from_lsp_position(pos: Position, line_index: &LineIndex) -> Option<TextSize> {
    line_index.offset(sneklsp_text::Position {
        line: pos.line,
        column: pos.character,
    })
}

#[inline]
pub fn to_lsp_symbol_kind(kind: sneklsp_index::SymbolKind) -> SymbolKind {
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

#[inline]
pub fn to_lsp_completion_kind(kind: sneklsp_index::SymbolKind) -> CompletionItemKind {
    match kind {
        sneklsp_index::SymbolKind::Function => CompletionItemKind::FUNCTION,
        sneklsp_index::SymbolKind::Class => CompletionItemKind::CLASS,
        sneklsp_index::SymbolKind::Variable => CompletionItemKind::VARIABLE,
        sneklsp_index::SymbolKind::Parameter => CompletionItemKind::VARIABLE,
        sneklsp_index::SymbolKind::Import => CompletionItemKind::MODULE,
        sneklsp_index::SymbolKind::ImportedSymbol => CompletionItemKind::REFERENCE,
        sneklsp_index::SymbolKind::Method => CompletionItemKind::METHOD,
        sneklsp_index::SymbolKind::Property => CompletionItemKind::PROPERTY,
        sneklsp_index::SymbolKind::TypeAlias => CompletionItemKind::TYPE_PARAMETER,
    }
}

pub struct DocumentQuery<'a> {
    pub index: &'a OwnedIndex,
    pub line_index: &'a LineIndex,
    pub uri: &'a Uri,
}

impl<'a> DocumentQuery<'a> {
    pub fn find_symbol_at(&self, offset: TextSize) -> Option<&'a SymbolData> {
        // check definitions
        if let Some(symbol) = self.index.symbol_at(offset) {
            return Some(symbol);
        }

        // check references
        if let Some(reference) = self.index.reference_at(offset) {
            if let Some(sym_id) = reference.resolved {
                return self.index.symbol(sym_id);
            }
        }

        None
    }

    pub fn all_occurrence_ranges(&self, symbol_id: u32) -> Vec<Range> {
        let mut ranges = Vec::new();

        if let Some(symbol) = self.index.symbol(symbol_id) {
            ranges.push(to_lsp_range(symbol.selection_range, self.line_index));
        }

        for reference in self.index.references_to(symbol_id) {
            ranges.push(to_lsp_range(reference.range, self.line_index));
        }

        ranges
    }

    #[inline]
    pub fn location(&self, range: TextRange) -> Location {
        Location {
            uri: self.uri.clone(),
            range: to_lsp_range(range, self.line_index),
        }
    }

    #[inline]
    pub fn highlight(
        &self,
        range: sneklsp_text::TextRange,
        kind: lsp_types::DocumentHighlightKind,
    ) -> lsp_types::DocumentHighlight {
        lsp_types::DocumentHighlight {
            range: to_lsp_range(range, self.line_index),
            kind: Some(kind),
        }
    }
}

pub fn get_document_query<'a>(
    uri: &'a Uri,
    documents: &'a HashMap<Uri, DocumentState>,
) -> Option<DocumentQuery<'a>> {
    let state = documents.get(uri)?;
    let index = state.document.index.as_ref()?;
    Some(DocumentQuery {
        index,
        line_index: &state.document.line_index,
        uri,
    })
}

fn to_document_symbol(
    symbol: &SymbolData,
    index: &OwnedIndex,
    line_index: &LineIndex,
) -> Option<DocumentSymbol> {
    if matches!(symbol.visibility, sneklsp_index::Visibility::DunderPrivate) {
        return None;
    }

    let children = find_symbol_children(symbol, index, line_index);
    let name = index.symbol_name(symbol).to_string();

    #[allow(deprecated)]
    Some(DocumentSymbol {
        name,
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
    })
}

fn find_symbol_children(
    parent: &SymbolData,
    index: &OwnedIndex,
    line_index: &LineIndex,
) -> Vec<DocumentSymbol> {
    let mut children = Vec::new();

    match parent.kind {
        sneklsp_index::SymbolKind::Function
        | sneklsp_index::SymbolKind::Class
        | sneklsp_index::SymbolKind::Method => {
            // find child scope that matches this symbol range
            for scope in index.scopes() {
                if scope.parent == Some(parent.scope) && scope.range == parent.range {
                    for &sym_id in &scope.symbols {
                        if let Some(sym) = index.symbol(sym_id) {
                            if let Some(doc_sym) = to_document_symbol(sym, index, line_index) {
                                children.push(doc_sym);
                            }
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

    let root_scope = index.root_scope()?;
    for &sym_id in &root_scope.symbols {
        if let Some(symbol) = index.symbol(sym_id) {
            if let Some(doc_sym) = to_document_symbol(symbol, index, &state.document.line_index) {
                symbols.push(doc_sym);
            }
        }
    }

    Some(DocumentSymbolResponse::Nested(symbols))
}

pub fn handle_goto_definition(
    params: GotoDefinitionParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<GotoDefinitionResponse> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;

    Some(GotoDefinitionResponse::Scalar(
        query.location(symbol.selection_range),
    ))
}

pub fn handle_references(
    params: ReferenceParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<Location>> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let include_decl = params.context.include_declaration;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;
    let mut locations = Vec::new();

    if include_decl {
        locations.push(query.location(symbol.selection_range));
    }

    for reference in query.index.references_to(symbol.id) {
        locations.push(query.location(reference.range));
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

pub fn handle_rename(
    params: RenameParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<WorkspaceEdit> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;

    let edits: Vec<TextEdit> = query
        .all_occurrence_ranges(symbol.id)
        .into_iter()
        .map(|range| TextEdit {
            range,
            new_text: new_name.clone(),
        })
        .collect();

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
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<lsp_types::DocumentHighlight>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;

    let mut highlights = Vec::new();

    // definition should WRITE
    highlights.push(query.highlight(
        symbol.selection_range,
        lsp_types::DocumentHighlightKind::WRITE,
    ));

    // references should READ
    for reference in query.index.references_to(symbol.id) {
        highlights.push(query.highlight(reference.range, lsp_types::DocumentHighlightKind::READ));
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

pub fn handle_completion(
    params: CompletionParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<CompletionResponse> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let mut items = Vec::new();
    let mut seen = HashSet::new();

    // collect all visible symbols as cursor
    let scope_id = find_scope_at(query.index, offset);
    collect_visible_symbols(query.index, scope_id, &mut seen, &mut items);

    add_builtin_completions(&mut seen, &mut items);

    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::Array(items))
    }
}

fn find_scope_at(index: &OwnedIndex, offset: TextSize) -> Option<u32> {
    let mut best: Option<(u32, u32)> = None; // (scope_id, range_len)

    for scope in index.scopes() {
        if scope.range.contains(offset) {
            let len = scope.range.len().to_u32();
            match best {
                Some((_, best_len)) if len < best_len => {
                    best = Some((scope.id, len));
                }
                None => {
                    best = Some((scope.id, len));
                }
                _ => {}
            }
        }
    }

    best.map(|(id, _)| id)
}

fn collect_visible_symbols(
    index: &OwnedIndex,
    scope_id: Option<u32>,
    seen: &mut std::collections::HashSet<String>,
    items: &mut Vec<CompletionItem>,
) {
    let mut current = scope_id;

    while let Some(sid) = current {
        if let Some(scope) = index.scope(sid) {
            // skip class scopes for name lookup
            let skip = scope.kind == sneklsp_index::ScopeKind::Class && scope_id != Some(sid);

            if !skip {
                for &sym_id in &scope.symbols {
                    if let Some(symbol) = index.symbol(sym_id) {
                        let name = index.symbol_name(symbol).to_string();

                        if seen.insert(name.clone()) {
                            items.push(CompletionItem {
                                label: name,
                                kind: Some(to_lsp_completion_kind(symbol.kind)),
                                detail: symbol_detail(symbol),
                                ..Default::default()
                            });
                        }
                    }
                }
            }

            current = scope.parent;
        } else {
            break;
        }
    }
}

fn symbol_detail(symbol: &SymbolData) -> Option<String> {
    match symbol.kind {
        sneklsp_index::SymbolKind::Function | sneklsp_index::SymbolKind::Method => {
            Some("function".to_string())
        }
        sneklsp_index::SymbolKind::Class => Some("class".to_string()),
        sneklsp_index::SymbolKind::Parameter => Some("parameter".to_string()),
        sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol => {
            Some("import".to_string())
        }
        _ => None,
    }
}

fn add_builtin_completions(
    seen: &mut std::collections::HashSet<String>,
    items: &mut Vec<CompletionItem>,
) {
    for builtin in BUILTINS {
        if seen.insert((*builtin.name).to_string()) {
            items.push(CompletionItem {
                label: builtin.name.to_string(),
                kind: Some(builtin.kind),
                detail: Some(builtin.detail.to_string()),
                ..Default::default()
            });
        }
    }
}
