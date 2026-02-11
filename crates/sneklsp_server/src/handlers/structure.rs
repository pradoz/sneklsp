use std::collections::HashMap;

use lsp_types::{
    DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeKind,
    FoldingRangeParams, InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, Position, Range,
    SelectionRange, SelectionRangeParams, Uri,
};

use super::common::{
    from_lsp_position, get_document_query, is_callable_symbol, to_lsp_range, to_lsp_symbol_kind,
};
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, ScopeData, SymbolData};
use sneklsp_text::{LineIndex, TextRange, TextSize};

pub fn handle_document_symbol(
    params: DocumentSymbolParams,
    documents: &HashMap<Uri, DocumentState>,
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

    let mut block_start = line_index.position(import_symbols[0].range.start());
    let mut block_end = line_index.position(import_symbols[0].range.end());

    for symbol in &import_symbols[1..] {
        let sym_start = line_index.position(symbol.range.start());
        let sym_end = line_index.position(symbol.range.end());

        if sym_start.line <= block_end.line + 1 {
            block_end = sym_end;
        } else {
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

    containing.sort_unstable_by_key(|s| s.range.len().to_u32());

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

pub fn handle_inlay_hint(
    params: InlayHintParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<InlayHint>> {
    let uri = params.text_document.uri;
    let query = get_document_query(&uri, documents)?;

    let source = query.index.source();
    let request_range = params.range;
    let mut hints = Vec::new();

    for reference in query.index.references() {
        let resolved_id = match reference.resolved {
            Some(id) => id,
            None => continue,
        };

        let ref_pos = query.line_index.position(reference.range.start());

        if ref_pos.line < request_range.start.line || ref_pos.line > request_range.end.line {
            continue;
        }

        let target = match query.index.symbol(resolved_id) {
            Some(s) => s,
            None => continue,
        };

        if !is_callable_symbol(target) {
            continue;
        }

        let param_names = match extract_param_names(query.index, target) {
            Some(names) if !names.is_empty() => names,
            _ => continue,
        };

        let ref_end = reference.range.end().to_usize();
        let after_ref = &source[ref_end..];

        let paren_offset = match after_ref.find('(') {
            Some(o) => ref_end + o,
            None => continue,
        };

        let arg_positions = find_argument_starts(source, paren_offset + 1);

        for (i, &arg_offset) in arg_positions.iter().enumerate() {
            if i >= param_names.len() {
                break;
            }

            let param_name = &param_names[i];

            if *param_name == "self" || *param_name == "cls" {
                continue;
            }

            let arg_text = &source[arg_offset..];
            if looks_like_keyword_arg(arg_text) {
                break;
            }

            let pos = query
                .line_index
                .position(sneklsp_text::TextSize::new(arg_offset as u32));
            hints.push(InlayHint {
                position: Position {
                    line: pos.line,
                    character: pos.column,
                },
                label: InlayHintLabel::String(format!("{}:", param_name)),
                kind: Some(InlayHintKind::PARAMETER),
                text_edits: None,
                tooltip: None,
                padding_left: None,
                padding_right: Some(true),
                data: None,
            });
        }
    }

    if hints.is_empty() { None } else { Some(hints) }
}

fn extract_param_names(index: &OwnedIndex, symbol: &SymbolData) -> Option<Vec<String>> {
    let sig = index.symbol_signature(symbol)?;

    let paren_start = sig.find('(')?;
    let paren_content = &sig[paren_start + 1..];
    let paren_end = find_matching_close(paren_content)?;
    let params_str = &paren_content[..paren_end];

    let mut names = Vec::new();
    let mut depth = 0u32;
    let mut current_start = 0;

    for (i, ch) in params_str.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(name) = extract_single_param_name(&params_str[current_start..i]) {
                    names.push(name);
                }
                current_start = i + 1;
            }
            _ => {}
        }
    }

    if current_start < params_str.len() {
        if let Some(name) = extract_single_param_name(&params_str[current_start..]) {
            names.push(name);
        }
    }

    Some(names)
}

fn extract_single_param_name(param: &str) -> Option<String> {
    let trimmed = param.trim();

    if trimmed.is_empty() || trimmed == "/" || trimmed == "*" {
        return None;
    }

    let trimmed = trimmed.trim_start_matches('*');

    let name: String = trimmed
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() { None } else { Some(name) }
}

fn find_matching_close(s: &str) -> Option<usize> {
    let mut depth = 0u32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => return Some(i),
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn find_argument_starts(source: &str, after_paren: usize) -> Vec<usize> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut positions = Vec::new();
    let mut depth = 0u32;
    let mut i = after_paren;

    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }

    if i < len && bytes[i] != b')' {
        positions.push(i);
    }

    while i < len {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    break;
                }
                depth = depth.saturating_sub(1);
            }
            b',' if depth == 0 => {
                let mut j = i + 1;
                while j < len && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < len && bytes[j] != b')' {
                    positions.push(j);
                }
            }
            b'\'' | b'"' => {
                i = skip_string_literal(bytes, i);
            }
            _ => {}
        }
        i += 1;
    }

    positions
}

fn skip_string_literal(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;
    let len = bytes.len();

    let triple = i + 1 < len && bytes[i] == quote && bytes[i + 1] == quote;
    if triple {
        i += 2;
    }

    while i < len {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            if triple {
                if i + 2 < len && bytes[i + 1] == quote && bytes[i + 2] == quote {
                    return i + 2;
                }
            } else {
                return i;
            }
        }
        i += 1;
    }

    i.saturating_sub(1)
}

fn looks_like_keyword_arg(text: &str) -> bool {
    let trimmed = text.trim_start();
    if let Some(eq_pos) = trimmed.find('=') {
        if eq_pos > 0 {
            let before = &trimmed[..eq_pos];
            let after_eq = trimmed.as_bytes().get(eq_pos + 1);
            if before.chars().all(|c| c.is_alphanumeric() || c == '_') && after_eq != Some(&b'=') {
                return true;
            }
        }
    }
    false
}
