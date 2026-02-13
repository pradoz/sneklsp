use lsp_types::{
    CompletionItem, CompletionParams, CompletionResponse, Position, Range, TextEdit, Uri,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::common::{from_lsp_position, get_document_query, to_lsp_completion_kind};
use crate::analysis::AnalysisHost;
use crate::builtins::BUILTINS;
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, SymbolData};
use sneklsp_text::LineIndex;
use sneklsp_workspace::Workspace;

struct ImportContext {
    insert_line: u32,
    existing_imports: FxHashSet<String>,
}

pub fn handle_completion(
    params: CompletionParams,
    documents: &FxHashMap<Uri, DocumentState>,
    analysis: &AnalysisHost,
    workspace: &Workspace,
) -> Option<CompletionResponse> {
    let uri = params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let mut items = Vec::new();
    let mut seen = FxHashSet::default();

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

fn collect_visible_symbols(
    index: &OwnedIndex,
    scope_id: Option<u32>,
    seen: &mut FxHashSet<String>,
    items: &mut Vec<CompletionItem>,
) {
    let mut current = scope_id;

    while let Some(sid) = current {
        if let Some(scope) = index.scope(sid) {
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
        None => find_line_after_docstring(index, line_index),
    };

    ImportContext {
        insert_line,
        existing_imports,
    }
}

fn find_line_after_docstring(index: &OwnedIndex, line_index: &LineIndex) -> u32 {
    let source = index.source();
    let trimmed = source.trim_start();

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
    seen: &mut FxHashSet<String>,
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
                sort_text: Some(format!("~{}", export.name)),
                ..Default::default()
            });
        }
    }
}

fn add_builtin_completions(seen: &mut FxHashSet<String>, items: &mut Vec<CompletionItem>) {
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
