use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, DocumentSymbol,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRange, FoldingRangeKind,
    FoldingRangeParams, GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents,
    HoverParams, InlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, Location,
    MarkupContent, MarkupKind, Position, Range, ReferenceParams, RenameParams, SelectionRange,
    SelectionRangeParams, SignatureHelp, SignatureHelpParams, SignatureInformation,
    SymbolInformation, SymbolKind, TextEdit, Uri, WorkspaceEdit, WorkspaceSymbolParams,
};
use rustc_hash::FxHashSet;
use std::collections::{HashMap, HashSet};

use crate::analysis::AnalysisHost;
use crate::builtins::{BUILTINS, BuiltinInfo};
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, ScopeData, SymbolData};
use sneklsp_text::{LineIndex, TextRange, TextSize};
use sneklsp_vfs::FileId;
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

#[inline]
pub fn from_lsp_position(pos: Position, line_index: &LineIndex) -> Option<TextSize> {
    line_index.offset(sneklsp_text::Position {
        line: pos.line,
        column: pos.character,
    })
}

#[inline]
fn ranges_overlap_lsp(a: Range, b: Range) -> bool {
    a.start.line <= b.end.line
        && b.start.line <= a.end.line
        && !(a.start.line == b.end.line && a.start.character > b.end.character)
        && !(b.start.line == a.end.line && b.start.character > a.end.character)
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
pub fn handle_inlay_hint(
    params: InlayHintParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<InlayHint>> {
    let uri = params.text_document.uri;
    let query = get_document_query(&uri, documents)?;

    let source = query.index.source();
    let request_range = params.range;
    let mut hints = Vec::new();

    // scan references that resolve to callable symbols
    for reference in query.index.references() {
        let resolved_id = match reference.resolved {
            Some(id) => id,
            None => continue,
        };

        let ref_pos = query.line_index.position(reference.range.start());

        // skip references outside the requested range
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

        // get the signature to extract parameter names
        let param_names = match extract_param_names(query.index, target) {
            Some(names) if !names.is_empty() => names,
            _ => continue,
        };

        // find the opening paren after the reference
        let ref_end = reference.range.end().to_usize();
        let after_ref = &source[ref_end..];

        let paren_offset = match after_ref.find('(') {
            Some(o) => ref_end + o,
            None => continue,
        };

        // find argument positions
        let arg_positions = find_argument_starts(source, paren_offset + 1);

        // annotate each positional arg with parameter name
        for (i, &arg_offset) in arg_positions.iter().enumerate() {
            if i >= param_names.len() {
                break;
            }

            let param_name = &param_names[i];

            // skip `self` and `cls` parameters
            if *param_name == "self" || *param_name == "cls" {
                continue;
            }

            // skip if the argument already looks like a keyword arg
            let arg_text = &source[arg_offset..];
            if looks_like_keyword_arg(arg_text) {
                break; // keyword args start, no more positional hints
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

    // extract content between first ( and matching )
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

    // last parameter
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

    // skip *args, **kwargs
    let trimmed = trimmed.trim_start_matches('*');

    // take identifier before `:` or `=`
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

    // find first argument
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
                // skip whitespace after comma
                let mut j = i + 1;
                while j < len && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < len && bytes[j] != b')' {
                    positions.push(j);
                }
            }
            b'\'' | b'"' => {
                // skip string literal annotations
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

    // check triple quote
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
    // check for `name=` pattern (but not `==`)
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

pub fn handle_goto_definition(
    params: GotoDefinitionParams,
    documents: &HashMap<Uri, DocumentState>,
    workspace: &Workspace,
    analysis: &AnalysisHost,
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
        if let Some(location) = resolve_import_via_salsa(symbol, query.index, analysis) {
            return Some(GotoDefinitionResponse::Scalar(location));
        }
        if let Some(location) = resolve_import_definition(symbol, query.index, workspace) {
            return Some(GotoDefinitionResponse::Scalar(location));
        }
    }

    Some(GotoDefinitionResponse::Scalar(
        query.location(symbol.selection_range),
    ))
}

fn resolve_import_via_salsa(
    symbol: &SymbolData,
    index: &OwnedIndex,
    analysis: &AnalysisHost,
) -> Option<Location> {
    let name = index.symbol_name(symbol);

    if symbol.kind == sneklsp_index::SymbolKind::Import {
        let target_file = analysis.resolve_module_file(name)?;
        let path = target_file.path(analysis.db());
        let target_uri: Uri = format!("file://{}", path).parse().ok()?;

        return Some(Location {
            uri: target_uri,
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
        });
    }

    if symbol.kind == sneklsp_index::SymbolKind::ImportedSymbol {
        // search all modules for this exported symbol
        for file_id in analysis.file_ids() {
            let Some(exports) = analysis.exported_symbols(file_id) else {
                continue;
            };

            for export in exports {
                if export.name == name {
                    let target_file_salsa = analysis.file_for_id(file_id)?;
                    let path = target_file_salsa.path(analysis.db());
                    let target_uri: Uri = format!("file://{}", path).parse().ok()?;
                    let line_index = analysis.line_index(file_id)?;
                    let range = to_lsp_range(export.range, line_index);

                    return Some(Location {
                        uri: target_uri,
                        range,
                    });
                }
            }
        }
    }

    None
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
            let target_state = workspace.get_file_state(file_id)?;
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

    let name = index.symbol_name(symbol);
    match symbol.kind {
        sneklsp_index::SymbolKind::Variable | sneklsp_index::SymbolKind::Parameter => {
            let ty = sneklsp_db::infer_symbol_type(index, symbol);
            if ty.is_unknown() {
                name.to_string()
            } else {
                format!("{}: {}", name, ty.display())
            }
        }
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
    analysis: &AnalysisHost,
    workspace: &Workspace,
) -> Option<WorkspaceEdit> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    let new_name = params.new_name;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;
    let symbol_name = query.index.symbol_name(symbol).to_string();

    // current file
    let local_edits: Vec<TextEdit> = query
        .all_occurrence_ranges(symbol.id)
        .into_iter()
        .map(|range| TextEdit {
            range,
            new_text: new_name.clone(),
        })
        .collect();

    if local_edits.is_empty() {
        return None;
    }

    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();
    changes.insert(uri.clone(), local_edits);

    // cross-file
    find_cross_file_edits(
        &symbol_name,
        &new_name,
        &uri,
        analysis,
        workspace,
        documents,
        &mut changes,
    );

    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}

fn find_cross_file_edits(
    symbol_name: &str,
    new_name: &str,
    origin_uri: &Uri,
    analysis: &AnalysisHost,
    workspace: &Workspace,
    documents: &HashMap<Uri, DocumentState>,
    changes: &mut HashMap<Uri, Vec<TextEdit>>,
) {
    for file_id in analysis.file_ids() {
        let vfs_path = workspace.vfs.file_path(file_id);
        let Some(file_uri) = vfs_path.to_uri() else {
            continue;
        };

        if file_uri == *origin_uri {
            continue;
        }

        let edits = collect_edits_in_file(
            file_id,
            symbol_name,
            new_name,
            &file_uri,
            workspace,
            documents,
        );

        if !edits.is_empty() {
            changes.insert(file_uri, edits);
        }
    }
}

fn collect_edits_in_file(
    file_id: FileId,
    symbol_name: &str,
    new_name: &str,
    file_uri: &Uri,
    workspace: &Workspace,
    documents: &HashMap<Uri, DocumentState>,
) -> Vec<TextEdit> {
    // try open document first, then workspace state, then salsa
    let (index, line_index) = if let Some(state) = documents.get(file_uri) {
        match (state.document.index.as_ref(), &state.document.line_index) {
            (Some(idx), li) => (idx, li),
            _ => return Vec::new(),
        }
    } else if let Some(state) = workspace.get_file_state(file_id) {
        match state.index.as_ref() {
            Some(idx) => (idx, &state.line_index),
            None => return Vec::new(),
        }
    } else {
        return Vec::new();
    };

    let mut edits = Vec::new();

    for symbol in index.symbols() {
        if !matches!(
            symbol.kind,
            sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
        ) {
            continue;
        }

        if index.symbol_name(symbol) != symbol_name {
            continue;
        }

        // rename the import binding itself
        edits.push(TextEdit {
            range: to_lsp_range(symbol.selection_range, line_index),
            new_text: new_name.to_string(),
        });

        // rename all references to this import in the file
        for reference in index.references_to(symbol.id) {
            edits.push(TextEdit {
                range: to_lsp_range(reference.range, line_index),
                new_text: new_name.to_string(),
            });
        }
    }

    edits
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

pub fn handle_prepare_rename(
    params: lsp_types::TextDocumentPositionParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<lsp_types::PrepareRenameResponse> {
    let uri = params.text_document.uri;
    let pos = params.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;
    let name = query.index.symbol_name(symbol);

    if crate::builtins::lookup(name).is_some() {
        return None;
    }

    let range = to_lsp_range(symbol.selection_range, query.line_index);

    Some(lsp_types::PrepareRenameResponse::RangeWithPlaceholder {
        range,
        placeholder: name.to_string(),
    })
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

pub fn handle_code_action(
    params: CodeActionParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<CodeActionResponse> {
    let uri = params.text_document.uri;
    let query = get_document_query(&uri, documents)?;

    let mut actions = Vec::new();

    for symbol in query.index.symbols() {
        if !matches!(
            symbol.kind,
            sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
        ) {
            continue;
        }

        let has_refs = query.index.references_to(symbol.id).next().is_some();
        if has_refs {
            continue;
        }

        let symbol_range = to_lsp_range(symbol.selection_range, query.line_index);

        // only offer action if cursor/selection overlaps the unused import
        if !ranges_overlap_lsp(params.range, symbol_range) {
            continue;
        }

        let name = query.index.symbol_name(symbol);

        // compute the range to delete: the entire line containing the import
        let full_line_range = line_range_for_symbol(symbol, query.line_index);

        let mut changes = HashMap::new();
        changes.insert(
            uri.clone(),
            vec![TextEdit {
                range: full_line_range,
                new_text: String::new(),
            }],
        );

        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: format!("Remove unused import '{}'", name),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: Some(true),
            disabled: None,
            data: None,
        }));
    }

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

fn line_range_for_symbol(symbol: &SymbolData, line_index: &LineIndex) -> Range {
    let start_pos = line_index.position(symbol.range.start());
    let end_pos = line_index.position(symbol.range.end());

    Range {
        start: Position {
            line: start_pos.line,
            character: 0,
        },
        end: Position {
            line: end_pos.line + 1,
            character: 0,
        },
    }
}

struct ImportContext {
    insert_line: u32,
    existing_imports: FxHashSet<String>,
}

pub fn handle_completion(
    params: CompletionParams,
    documents: &HashMap<Uri, DocumentState>,
    analysis: &AnalysisHost,
    workspace: &Workspace,
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

    let import_ctx = build_import_context(query.index, query.line_index);
    collect_cross_file_completions(analysis, workspace, &import_ctx, &mut seen, &mut items);

    add_builtin_completions(&mut seen, &mut items);

    if items.is_empty() {
        None
    } else {
        Some(CompletionResponse::Array(items))
    }
}

fn build_import_context(index: &OwnedIndex, line_index: &LineIndex) -> ImportContext {
    let mut last_import_line: Option<u32> = None;
    let mut existing_imports = FxHashSet::default();

    for symbol in index.symbols() {
        match symbol.kind {
            sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol => {
                let name = index.symbol_name(symbol);
                existing_imports.insert(name.to_string());

                let end_pos = line_index.position(symbol.range.end());
                match last_import_line {
                    Some(line) if end_pos.line > line => {
                        last_import_line = Some(end_pos.line);
                    }
                    None => {
                        last_import_line = Some(end_pos.line);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    let insert_line = match last_import_line {
        Some(line) => line + 1,
        None => {
            // no imports. insert after module docstring if present, otherwise at line 0
            find_line_after_docstring(index, line_index)
        }
    };

    ImportContext {
        insert_line,
        existing_imports,
    }
}

fn find_line_after_docstring(index: &OwnedIndex, line_index: &LineIndex) -> u32 {
    let source = index.source();
    let trimmed = source.trim_start();

    // check if file starts module docstring
    if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
        let quote = &trimmed[..3];
        if let Some(end) = trimmed[3..].find(quote) {
            let docstring_end = (source.len() - trimmed.len()) + 3 + end + 3;
            let pos = line_index.position(sneklsp_text::TextSize::new(docstring_end as u32));
            return pos.line + 1;
        }
    } else if trimmed.starts_with('"') || trimmed.starts_with('\'') {
        let quote = trimmed.as_bytes()[0];
        if let Some(end) = trimmed[1..].find(|c: char| c as u8 == quote) {
            let docstring_end = (source.len() - trimmed.len()) + 1 + end + 1;
            let pos = line_index.position(sneklsp_text::TextSize::new(docstring_end as u32));
            return pos.line + 1;
        }
    }

    0
}

fn make_import_edit(module_name: &str, symbol_name: &str, insert_line: u32) -> TextEdit {
    let import_text = format!("from {} import {}\n", module_name, symbol_name);
    let position = Position {
        line: insert_line,
        character: 0,
    };

    TextEdit {
        range: Range {
            start: position,
            end: position,
        },
        new_text: import_text,
    }
}

fn collect_cross_file_completions(
    analysis: &AnalysisHost,
    workspace: &Workspace,
    import_ctx: &ImportContext,
    seen: &mut HashSet<String>,
    items: &mut Vec<CompletionItem>,
) {
    for file_id in analysis.file_ids() {
        let Some(module_name) = workspace.resolve_module_name(file_id) else {
            continue;
        };

        let Some(exports) = analysis.exported_symbols(file_id) else {
            continue;
        };

        for export in exports {
            // already imported
            if import_ctx.existing_imports.contains(&export.name) {
                continue;
            }

            if !seen.insert(export.name.clone()) {
                continue;
            }

            let edit = make_import_edit(&module_name, &export.name, import_ctx.insert_line);

            items.push(CompletionItem {
                label: export.name.clone(),
                kind: Some(to_lsp_completion_kind(export.kind)),
                detail: Some(format!("from {}", module_name)),
                additional_text_edits: Some(vec![edit]),
                sort_text: Some(format!("~{}", export.name)), // sort after local symbols
                ..Default::default()
            });
        }
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

pub fn handle_workspace_symbol(
    params: WorkspaceSymbolParams,
    documents: &HashMap<Uri, DocumentState>,
    analysis: &AnalysisHost,
    workspace: &Workspace,
) -> Option<Vec<SymbolInformation>> {
    let query = params.query.to_lowercase();
    let mut results = Vec::new();
    let mut seen_files: HashSet<Uri> = HashSet::new();

    // search open documents first
    for (uri, state) in documents {
        let Some(index) = state.document.index.as_ref() else {
            continue;
        };
        seen_files.insert(uri.clone());
        collect_matching_symbols(index, &state.document.line_index, uri, &query, &mut results);
    }

    // search workspace files not already covered
    for file_id in analysis.file_ids() {
        let vfs_path = workspace.vfs.file_path(file_id);
        let Some(file_uri) = vfs_path.to_uri() else {
            continue;
        };

        if seen_files.contains(&file_uri) {
            continue;
        }

        // try salsa analysis first, then workspace state
        if let Some(file_analysis) = analysis.analyze_file(file_id) {
            if let Some(ref index) = file_analysis.index {
                collect_matching_symbols(
                    index,
                    &file_analysis.line_index,
                    &file_uri,
                    &query,
                    &mut results,
                );
                continue;
            }
        }

        if let Some(file_state) = workspace.get_file_state(file_id) {
            if let Some(ref index) = file_state.index {
                collect_matching_symbols(
                    index,
                    &file_state.line_index,
                    &file_uri,
                    &query,
                    &mut results,
                );
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

fn collect_matching_symbols(
    index: &OwnedIndex,
    line_index: &LineIndex,
    uri: &Uri,
    query: &str,
    results: &mut Vec<SymbolInformation>,
) {
    // only search top-level and class-level symbols for workspace search
    for symbol in index.symbols() {
        if !is_workspace_searchable(symbol) {
            continue;
        }

        let name = index.symbol_name(symbol);

        // empty query returns all symbols
        if !query.is_empty() && !name.to_lowercase().contains(query) {
            continue;
        }

        #[allow(deprecated)]
        results.push(SymbolInformation {
            name: name.to_string(),
            kind: to_lsp_symbol_kind(symbol.kind),
            tags: None,
            deprecated: None,
            location: Location {
                uri: uri.clone(),
                range: to_lsp_range(symbol.selection_range, line_index),
            },
            container_name: scope_container_name(index, symbol),
        });
    }
}

fn is_workspace_searchable(symbol: &SymbolData) -> bool {
    matches!(
        symbol.kind,
        sneklsp_index::SymbolKind::Function
            | sneklsp_index::SymbolKind::Class
            | sneklsp_index::SymbolKind::Method
            | sneklsp_index::SymbolKind::Variable
            | sneklsp_index::SymbolKind::Import
            | sneklsp_index::SymbolKind::ImportedSymbol
            | sneklsp_index::SymbolKind::TypeAlias
    )
}

fn scope_container_name(index: &OwnedIndex, symbol: &SymbolData) -> Option<String> {
    if symbol.scope == 0 {
        return None;
    }

    let scope = index.scope(symbol.scope)?;
    let parent_scope_id = scope.parent?;
    let parent_scope = index.scope(parent_scope_id)?;

    // find the symbol that owns the parent scope
    for &sym_id in &parent_scope.symbols {
        if let Some(parent_sym) = index.symbol(sym_id) {
            if parent_sym.range == scope.range {
                return Some(index.symbol_name(parent_sym).to_string());
            }
        }
    }

    None
}
