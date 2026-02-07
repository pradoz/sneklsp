use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DocumentSymbol,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeKind,
    FoldingRangeParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverParams, Location, MarkupContent, MarkupKind, Position, Range, ReferenceParams,
    RenameParams, SelectionRange, SelectionRangeParams, SignatureHelp, SignatureHelpParams,
    SignatureInformation, SymbolKind, TextEdit, Uri, WorkspaceEdit,
};
use std::collections::{HashMap, HashSet};

use crate::builtins::{BUILTINS, BuiltinInfo};
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, ScopeData, SymbolData};
use sneklsp_text::{LineIndex, TextRange, TextSize};
use sneklsp_workspace::{ImportResolver, Workspace};

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

pub fn handle_folding_range(
    params: FoldingRangeParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<FoldingRange>> {
    let uri = params.text_document.uri;
    let query = get_document_query(&uri, documents)?;

    let mut ranges = Vec::new();

    for scope in query.index.scopes() {
        let kind = match scope.kind {
            sneklsp_index::ScopeKind::Module => continue,
            sneklsp_index::ScopeKind::Function
            | sneklsp_index::ScopeKind::Class
            | sneklsp_index::ScopeKind::Lambda
            | sneklsp_index::ScopeKind::Comprehension => FoldingRangeKind::Region,
        };

        let start = query.line_index.position(scope.range.start());
        let end = query.line_index.position(scope.range.end());

        if end.line > start.line {
            ranges.push(FoldingRange {
                start_line: start.line,
                start_character: Some(start.column),
                end_line: end.line,
                end_character: Some(end.column),
                kind: Some(kind),
                collapsed_text: None,
            });
        }
    }

    fold_import_blocks(query.index, query.line_index, &mut ranges);

    if ranges.is_empty() {
        None
    } else {
        Some(ranges)
    }
}

fn fold_import_blocks(index: &OwnedIndex, line_index: &LineIndex, ranges: &mut Vec<FoldingRange>) {
    let root_scope = match index.root_scope() {
        Some(s) => s,
        None => return,
    };

    let mut import_symbols: Vec<&SymbolData> = Vec::new();

    for &sym_id in &root_scope.symbols {
        if let Some(symbol) = index.symbol(sym_id) {
            if matches!(
                symbol.kind,
                sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
            ) {
                import_symbols.push(symbol);
            }
        }
    }

    if import_symbols.len() < 2 {
        return;
    }

    // sort by start position
    let mut block_start = line_index.position(import_symbols[0].range.start());
    let mut block_end = line_index.position(import_symbols[0].range.end());

    for symbol in &import_symbols[1..] {
        let sym_start = line_index.position(symbol.range.start());
        let sym_end = line_index.position(symbol.range.end());

        // consective/adjacent lines
        if sym_start.line <= block_end.line + 1 {
            block_end = sym_end;
        } else {
            // emit previous block if multi-line
            if block_end.line > block_start.line {
                ranges.push(FoldingRange {
                    start_line: block_start.line,
                    start_character: Some(block_start.column),
                    end_line: block_end.line,
                    end_character: Some(block_end.column),
                    kind: Some(FoldingRangeKind::Imports),
                    collapsed_text: None,
                });
            }

            block_start = sym_start;
            block_end = sym_end;
        }
    }

    // emit final block
    if block_end.line > block_start.line {
        ranges.push(FoldingRange {
            start_line: block_start.line,
            start_character: Some(block_start.column),
            end_line: block_end.line,
            end_character: Some(block_end.column),
            kind: Some(FoldingRangeKind::Imports),
            collapsed_text: None,
        });
    }
}

pub fn handle_goto_definition(
    params: GotoDefinitionParams,
    documents: &HashMap<Uri, DocumentState>,
    workspace: &Workspace,
) -> Option<GotoDefinitionResponse> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;

    if matches!(
        symbol.kind,
        sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
    ) {
        if let Some(location) = resolve_import_definition(symbol, query.index, workspace) {
            return Some(GotoDefinitionResponse::Scalar(location));
        }
    }

    Some(GotoDefinitionResponse::Scalar(
        query.location(symbol.selection_range),
    ))
}

fn resolve_import_definition(
    symbol: &SymbolData,
    index: &OwnedIndex,
    workspace: &Workspace,
) -> Option<Location> {
    let resolver = ImportResolver::new(workspace);
    let name = index.symbol_name(symbol);

    // try to find the import statement that defined this symbol
    if symbol.kind == sneklsp_index::SymbolKind::Import {
        let resolved = resolver.resolve_import(name)?;
        let target_path = workspace.vfs.file_path(resolved.file_id);
        let target_uri = target_path.to_uri()?;

        // jump to top of file
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        };

        return Some(Location {
            uri: target_uri,
            range,
        });
    }

    // `from foo import bar` --> find `foo` module and `bar` symbol
    if symbol.kind == sneklsp_index::SymbolKind::ImportedSymbol {
        let results = workspace.find_exported_symbol(name);

        for (file_id, symbol_id) in results {
            let target_path = workspace.vfs.file_path(file_id);
            let target_uri = target_path.to_uri()?;
            let target_state = workspace.file_state(file_id)?;
            let target_index = target_state.index.as_ref()?;
            let target_line_index = &target_state.line_index;

            if let Some(target_sym) = target_index.symbol(symbol_id) {
                return Some(Location {
                    uri: target_uri,
                    range: to_lsp_range(target_sym.range, target_line_index),
                });
            }
        }
    }

    None
}

enum HoverTarget<'a> {
    Symbol(&'a SymbolData),
    Builtin(&'static BuiltinInfo),
}

pub fn handle_hover(params: HoverParams, documents: &HashMap<Uri, DocumentState>) -> Option<Hover> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let target = find_hover_target(&query, offset)?;

    let (signature, doc, range) = match target {
        HoverTarget::Symbol(symbol) => (
            format_symbol_signature(symbol, query.index),
            query.index.symbol_docstring(symbol).map(|s| s.to_string()),
            Some(to_lsp_range(symbol.selection_range, query.line_index)),
        ),
        HoverTarget::Builtin(builtin) => (
            builtin.signature.to_string(),
            if builtin.doc.is_empty() {
                None
            } else {
                Some(builtin.doc.to_string())
            },
            None,
        ),
    };

    let mut contents = String::new();
    contents.push_str("```python\n");
    contents.push_str(&signature);
    contents.push_str("\n```");

    if let Some(doc) = doc {
        contents.push_str("\n\n---\n\n");
        contents.push_str(&doc);
    }

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: contents,
        }),
        range,
    })
}

fn find_hover_target<'a>(
    query: &'a DocumentQuery<'a>,
    offset: TextSize,
) -> Option<HoverTarget<'a>> {
    // try symbol in this file first
    if let Some(symbol) = query.find_symbol_at(offset) {
        return Some(HoverTarget::Symbol(symbol));
    }

    // try unresolved reference -> builtin lookup
    if let Some(reference) = query.index.reference_at(offset) {
        if reference.resolved.is_none() {
            let name = query.index.reference_name(reference);
            if let Some(builtin) = crate::builtins::lookup(name) {
                return Some(HoverTarget::Builtin(builtin));
            }
        }
    }

    None
}

fn format_symbol_signature(symbol: &SymbolData, index: &OwnedIndex) -> String {
    if let Some(sig) = index.symbol_signature(symbol) {
        return sig.to_string();
    }

    // fallback for symbols without signature ranges
    let name = index.symbol_name(symbol);
    match symbol.kind {
        sneklsp_index::SymbolKind::Variable => name.to_string(),
        sneklsp_index::SymbolKind::Parameter => name.to_string(),
        sneklsp_index::SymbolKind::Import => format!("import {}", name),
        sneklsp_index::SymbolKind::ImportedSymbol => format!("from ... import {}", name),
        sneklsp_index::SymbolKind::Property => format!("@property\n{}", name),
        sneklsp_index::SymbolKind::TypeAlias => format!("type {} = ...", name),
        _ => name.to_string(),
    }
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

enum CallTarget<'a> {
    Symbol(&'a SymbolData),
    Builtin(&'static crate::builtins::BuiltinInfo),
}

pub fn handle_selection_range(
    params: SelectionRangeParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<SelectionRange>> {
    let uri = params.text_document.uri;
    let query = get_document_query(&uri, documents)?;

    let mut results = Vec::with_capacity(params.positions.len());

    for pos in params.positions {
        let offset = match from_lsp_position(pos, query.line_index) {
            Some(o) => o,
            None => {
                results.push(SelectionRange {
                    range: Range {
                        start: pos,
                        end: pos,
                    },
                    parent: None,
                });
                continue;
            }
        };

        let selection = build_selection_range(query.index, query.line_index, offset);
        results.push(selection);
    }

    Some(results)
}

fn build_selection_range(
    index: &OwnedIndex,
    line_index: &LineIndex,
    offset: TextSize,
) -> SelectionRange {
    let mut containing: Vec<&ScopeData> = Vec::new();

    for scope in index.scopes() {
        if scope.range.contains(offset) {
            containing.push(scope);
        }
    }

    containing.sort_unstable_by_key(|s| s.range.len().to_u32()); // sort by innermost first

    let symbol_range = index.symbol_at(offset).map(|s| s.selection_range);
    let ref_range = index.reference_at(offset).map(|s| s.range);

    let mut ranges: Vec<TextRange> = Vec::new();
    if let Some(r) = ref_range {
        ranges.push(r);
    } else if let Some(r) = symbol_range {
        ranges.push(r);
    }

    if let Some(symbol) = index.symbol_at(offset) {
        if symbol.range != symbol.selection_range {
            if ranges.last().map_or(true, |&last| last != symbol.range) {
                ranges.push(symbol.range);
            }
        }
    }

    for scope in &containing {
        if ranges.last().map_or(true, |&last| last != scope.range) {
            ranges.push(scope.range);
        }
    }

    ranges.dedup();

    let mut current: Option<SelectionRange> = None;

    for range in ranges.into_iter().rev() {
        let lsp_range = to_lsp_range(range, line_index);
        current = Some(SelectionRange {
            range: lsp_range,
            parent: current.map(Box::new),
        });
    }

    current.unwrap_or_else(|| {
        let pos = line_index.position(offset);
        SelectionRange {
            range: Range {
                start: Position {
                    line: pos.line,
                    character: pos.column,
                },
                end: Position {
                    line: pos.line,
                    character: pos.column,
                },
            },
            parent: None,
        }
    })
}

pub fn handle_signature_help(
    params: SignatureHelpParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<SignatureHelp> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let (target, active_param) = find_call_context(query.index, offset)?;

    let (signature_label, documentation) = match target {
        CallTarget::Symbol(symbol) => (
            match query.index.symbol_signature(symbol) {
                Some(sig) => sig.to_string(),
                None => format!("{}(...)", query.index.symbol_name(symbol)),
            },
            query.index.symbol_docstring(symbol).map(|doc| {
                lsp_types::Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.to_string(),
                })
            }),
        ),
        CallTarget::Builtin(builtin) => (
            builtin.signature.to_string(),
            if builtin.doc.is_empty() {
                None
            } else {
                Some(lsp_types::Documentation::MarkupContent(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: builtin.doc.to_string(),
                }))
            },
        ),
    };

    let signature = SignatureInformation {
        label: signature_label,
        documentation,
        parameters: None,
        active_parameter: Some(active_param),
    };

    Some(SignatureHelp {
        signatures: vec![signature],
        active_signature: Some(0),
        active_parameter: Some(active_param),
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
    let scope = query.index.scope_at(offset);
    let scope_id = scope.map(|s| s.id);
    collect_visible_symbols(query.index, scope_id, &mut seen, &mut items);

    add_builtin_completions(&mut seen, &mut items);

    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::Array(items))
    }
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

fn find_call_context<'a>(index: &'a OwnedIndex, offset: TextSize) -> Option<(CallTarget<'a>, u32)> {
    let source = index.source();
    let cursor = offset.to_usize().min(source.len());
    let bytes = source.as_bytes();

    // walk backwards to find the opening paren of the call
    let mut depth: u32 = 0;
    let mut paren_pos = None;

    for i in (0..cursor).rev() {
        match bytes[i] {
            b')' | b']' | b'}' => depth += 1,
            b'(' => {
                if depth == 0 {
                    paren_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
            b'[' | b'{' => {
                if depth == 0 {
                    break; // already inside bracketry
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let paren_pos = paren_pos?;

    let active_param = count_commas_at_depth_zero(bytes, paren_pos + 1, cursor);

    // scan backwards for function name
    let func_end = paren_pos;
    let mut func_start = func_end;
    while func_start > 0 && bytes[func_start - 1].is_ascii_whitespace() {
        func_start -= 1;
    }
    let name_end = func_start;
    while func_start > 0
        && (bytes[func_start - 1].is_ascii_alphanumeric() || bytes[func_start - 1] == b'_')
    {
        func_start -= 1;
    }

    if func_start == name_end {
        return None; // no identifier found
    }

    let func_offset = TextSize::new(func_start as u32);
    let func_name = &source[func_start..name_end];

    // try resolving definition and then reference
    if let Some(symbol) = index.symbol_at(func_offset) {
        if is_callable_symbol(symbol) {
            return Some((CallTarget::Symbol(symbol), active_param));
        }
    };

    if let Some(reference) = index.reference_at(func_offset) {
        if let Some(sym_id) = reference.resolved {
            if let Some(symbol) = index.symbol(sym_id) {
                if is_callable_symbol(symbol) {
                    return Some((CallTarget::Symbol(symbol), active_param));
                }
            }
        }
    }

    // fall back to builtins
    if let Some(builtin) = crate::builtins::lookup(func_name) {
        return Some((CallTarget::Builtin(builtin), active_param));
    }

    None
}

fn is_callable_symbol(symbol: &SymbolData) -> bool {
    matches!(
        symbol.kind,
        sneklsp_index::SymbolKind::Function
            | sneklsp_index::SymbolKind::Class
            | sneklsp_index::SymbolKind::Method
    )
}

fn count_commas_at_depth_zero(bytes: &[u8], start: usize, end: usize) -> u32 {
    let mut depth: u32 = 0;
    let mut commas: u32 = 0;

    for &b in &bytes[start..end] {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => commas += 1,
            _ => {}
        }
    }

    commas
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
