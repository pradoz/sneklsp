use std::collections::HashMap;

use lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, CodeActionResponse,
    Position, Range, TextEdit, Uri, WorkspaceEdit,
};

use super::common::{
    DocumentQuery, from_lsp_position, get_document_query, ranges_overlap_lsp, to_lsp_range,
};
use crate::analysis::AnalysisHost;
use crate::server::DocumentState;
use sneklsp_index::SymbolData;
use sneklsp_text::LineIndex;
use sneklsp_vfs::FileId;
use sneklsp_workspace::Workspace;

pub fn handle_rename(
    params: lsp_types::RenameParams,
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

        edits.push(TextEdit {
            range: to_lsp_range(symbol.selection_range, line_index),
            new_text: new_name.to_string(),
        });

        for reference in index.references_to(symbol.id) {
            edits.push(TextEdit {
                range: to_lsp_range(reference.range, line_index),
                new_text: new_name.to_string(),
            });
        }
    }

    edits
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

pub fn handle_code_action(
    params: CodeActionParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<CodeActionResponse> {
    let uri = params.text_document.uri;
    let query = get_document_query(&uri, documents)?;

    let mut actions = Vec::new();

    remove_unused_import_actions(&query, &uri, &params.range, &mut actions);
    sort_imports_action(&query, &uri, &mut actions);
    add_missing_self_actions(&query, &uri, &params.range, &mut actions);

    if actions.is_empty() {
        None
    } else {
        Some(actions)
    }
}

fn remove_unused_import_actions(
    query: &DocumentQuery<'_>,
    uri: &Uri,
    cursor_range: &Range,
    actions: &mut Vec<CodeActionOrCommand>,
) {
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
        if !ranges_overlap_lsp(*cursor_range, symbol_range) {
            continue;
        }

        let name = query.index.symbol_name(symbol);
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
}

fn sort_imports_action(
    query: &DocumentQuery<'_>,
    uri: &Uri,
    actions: &mut Vec<CodeActionOrCommand>,
) {
    let source = query.index.source();

    let mut import_entries: Vec<(u32, u32, String)> = Vec::new();

    for symbol in query.index.symbols() {
        if !matches!(
            symbol.kind,
            sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
        ) {
            continue;
        }

        if symbol.scope != 0 {
            continue;
        }

        let start_pos = query.line_index.position(symbol.range.start());
        let end_pos = query.line_index.position(symbol.range.end());

        let line_start_offset = query.line_index.offset(sneklsp_text::Position {
            line: start_pos.line,
            column: 0,
        });
        let next_line_offset = query.line_index.offset(sneklsp_text::Position {
            line: end_pos.line + 1,
            column: 0,
        });

        let text = match (line_start_offset, next_line_offset) {
            (Some(start), Some(end)) => {
                let s = start.to_usize();
                let e = end.to_usize().min(source.len());
                source[s..e].to_string()
            }
            (Some(start), None) => {
                let s = start.to_usize();
                let mut t = source[s..].to_string();
                if !t.ends_with('\n') {
                    t.push('\n');
                }
                t
            }
            _ => continue,
        };

        import_entries.push((start_pos.line, end_pos.line, text));
    }

    if import_entries.len() < 2 {
        return;
    }

    let texts: Vec<&str> = import_entries.iter().map(|(_, _, t)| t.as_str()).collect();
    let mut sorted_texts = texts.clone();
    sorted_texts.sort_unstable_by(|a, b| import_sort_key(a).cmp(&import_sort_key(b)));

    if texts == sorted_texts {
        return;
    }

    let first_line = import_entries.iter().map(|(s, _, _)| *s).min().unwrap();
    let last_line = import_entries.iter().map(|(_, e, _)| *e).max().unwrap();

    let sorted_text: String = sorted_texts.into_iter().collect();

    let range = Range {
        start: Position {
            line: first_line,
            character: 0,
        },
        end: Position {
            line: last_line + 1,
            character: 0,
        },
    };

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![TextEdit {
            range,
            new_text: sorted_text,
        }],
    );

    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
        title: "Sort imports".to_string(),
        kind: Some(CodeActionKind::SOURCE_ORGANIZE_IMPORTS),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: None,
        disabled: None,
        data: None,
    }));
}

fn import_sort_key(line: &str) -> (u8, String) {
    let trimmed = line.trim();
    let group = if trimmed.starts_with("from") { 1 } else { 0 };
    (group, trimmed.to_lowercase())
}

fn add_missing_self_actions(
    query: &DocumentQuery<'_>,
    uri: &Uri,
    cursor_range: &Range,
    actions: &mut Vec<CodeActionOrCommand>,
) {
    for scope in query.index.scopes() {
        if scope.kind != sneklsp_index::ScopeKind::Class {
            continue;
        }

        for &child_id in &scope.children {
            let Some(child_scope) = query.index.scope(child_id) else {
                continue;
            };

            if child_scope.kind != sneklsp_index::ScopeKind::Function {
                continue;
            }

            let method = scope.symbols.iter().find_map(|&sym_id| {
                let sym = query.index.symbol(sym_id)?;
                if sym.kind == sneklsp_index::SymbolKind::Method && sym.range == child_scope.range {
                    Some(sym)
                } else {
                    None
                }
            });

            let Some(method_sym) = method else { continue };

            let method_name = query.index.symbol_name(method_sym);

            if method_name.starts_with("__") && method_name.ends_with("__") {
                continue;
            }

            let method_range = to_lsp_range(method_sym.selection_range, query.line_index);
            if !ranges_overlap_lsp(*cursor_range, method_range) {
                continue;
            }

            let has_self_or_cls = child_scope.symbols.iter().any(|&sym_id| {
                let Some(sym) = query.index.symbol(sym_id) else {
                    return false;
                };
                if sym.kind != sneklsp_index::SymbolKind::Parameter {
                    return false;
                }
                let name = query.index.symbol_name(sym);
                name == "self" || name == "cls"
            });

            if has_self_or_cls {
                continue;
            }

            let source = query.index.source();
            let name_end = method_sym.selection_range.end().to_usize();
            let after_name = &source[name_end..];
            let Some(paren_offset) = after_name.find('(') else {
                continue;
            };
            let insert_offset = name_end + paren_offset + 1;

            let has_params = child_scope.symbols.iter().any(|&sym_id| {
                query
                    .index
                    .symbol(sym_id)
                    .map_or(false, |s| s.kind == sneklsp_index::SymbolKind::Parameter)
            });

            let insert_text = if has_params { "self, " } else { "self" };
            let insert_pos = query
                .line_index
                .position(sneklsp_text::TextSize::new(insert_offset as u32));

            let position = Position {
                line: insert_pos.line,
                character: insert_pos.column,
            };

            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![TextEdit {
                    range: Range {
                        start: position,
                        end: position,
                    },
                    new_text: insert_text.to_string(),
                }],
            );

            actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                title: format!("Add 'self' parameter to '{}'", method_name),
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
