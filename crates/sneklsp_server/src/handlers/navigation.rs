use std::collections::HashMap;

use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range, ReferenceParams,
    SymbolInformation, Uri, WorkspaceSymbolParams,
};
use rustc_hash::FxHashSet;

use super::common::{
    from_lsp_position, get_document_query, is_callable_symbol, scope_container_name, to_lsp_range,
    to_lsp_symbol_kind,
};
use crate::analysis::AnalysisHost;
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, SymbolData};
use sneklsp_text::LineIndex;
use sneklsp_workspace::{ImportResolver, Workspace};

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

    if symbol.kind == sneklsp_index::SymbolKind::Import {
        let resolved = resolver.resolve_import(name)?;
        let target_path = workspace.vfs.file_path(resolved.file_id);
        let target_uri = target_path.to_uri()?;

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

    highlights.push(query.highlight(
        symbol.selection_range,
        lsp_types::DocumentHighlightKind::WRITE,
    ));

    for reference in query.index.references_to(symbol.id) {
        highlights.push(query.highlight(reference.range, lsp_types::DocumentHighlightKind::READ));
    }

    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}

pub fn handle_prepare_call_hierarchy(
    params: CallHierarchyPrepareParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<CallHierarchyItem>> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;

    let query = get_document_query(&uri, documents)?;
    let offset = from_lsp_position(pos, query.line_index)?;

    let symbol = query.find_symbol_at(offset)?;

    if !is_callable_symbol(symbol) {
        return None;
    }

    let name = query.index.symbol_name(symbol).to_string();

    Some(vec![CallHierarchyItem {
        name,
        kind: to_lsp_symbol_kind(symbol.kind),
        tags: None,
        detail: scope_container_name(query.index, symbol),
        uri: uri.clone(),
        range: to_lsp_range(symbol.range, query.line_index),
        selection_range: to_lsp_range(symbol.selection_range, query.line_index),
        data: Some(serde_json::to_value(symbol.id).unwrap()),
    }])
}

pub fn handle_incoming_calls(
    params: CallHierarchyIncomingCallsParams,
    documents: &HashMap<Uri, DocumentState>,
    analysis: &AnalysisHost,
    workspace: &Workspace,
) -> Option<Vec<CallHierarchyIncomingCall>> {
    let item = &params.item;
    let symbol_name = &item.name;
    let target_uri = &item.uri;

    let mut calls = Vec::new();

    if let Some(query) = get_document_query(target_uri, documents) {
        collect_incoming_calls_in_file(
            query.index,
            query.line_index,
            target_uri,
            symbol_name,
            &mut calls,
        );
    }

    for file_id in analysis.file_ids() {
        let vfs_path = workspace.vfs.file_path(file_id);
        let Some(file_uri) = vfs_path.to_uri() else {
            continue;
        };

        if file_uri == *target_uri {
            continue;
        }

        let (index, line_index) = if let Some(state) = documents.get(&file_uri) {
            match (state.document.index.as_ref(), &state.document.line_index) {
                (Some(idx), li) => (idx, li),
                _ => continue,
            }
        } else if let Some(file_analysis) = analysis.analyze_file(file_id) {
            match file_analysis.index.as_ref() {
                Some(idx) => (idx, &file_analysis.line_index),
                None => continue,
            }
        } else {
            continue;
        };

        collect_incoming_calls_in_file(index, line_index, &file_uri, symbol_name, &mut calls);
    }

    if calls.is_empty() { None } else { Some(calls) }
}

fn collect_incoming_calls_in_file(
    index: &OwnedIndex,
    line_index: &LineIndex,
    uri: &Uri,
    target_name: &str,
    calls: &mut Vec<CallHierarchyIncomingCall>,
) {
    for reference in index.references() {
        if index.reference_name(reference) != target_name {
            continue;
        }

        let Some(scope) = index.scope_at(reference.range.start()) else {
            continue;
        };

        if scope.kind != sneklsp_index::ScopeKind::Function {
            continue;
        }

        let parent_id = match scope.parent {
            Some(p) => p,
            None => continue,
        };

        let Some(parent_scope) = index.scope(parent_id) else {
            continue;
        };

        let caller = parent_scope.symbols.iter().find_map(|&sym_id| {
            let sym = index.symbol(sym_id)?;
            if matches!(
                sym.kind,
                sneklsp_index::SymbolKind::Function | sneklsp_index::SymbolKind::Method
            ) && sym.range == scope.range
            {
                Some(sym)
            } else {
                None
            }
        });

        let Some(caller_sym) = caller else { continue };

        let caller_name = index.symbol_name(caller_sym).to_string();

        calls.push(CallHierarchyIncomingCall {
            from: CallHierarchyItem {
                name: caller_name,
                kind: to_lsp_symbol_kind(caller_sym.kind),
                tags: None,
                detail: scope_container_name(index, caller_sym),
                uri: uri.clone(),
                range: to_lsp_range(caller_sym.range, line_index),
                selection_range: to_lsp_range(caller_sym.selection_range, line_index),
                data: None,
            },
            from_ranges: vec![to_lsp_range(reference.range, line_index)],
        });
    }
}

pub fn handle_outgoing_calls(
    params: CallHierarchyOutgoingCallsParams,
    documents: &HashMap<Uri, DocumentState>,
) -> Option<Vec<CallHierarchyOutgoingCall>> {
    let item = &params.item;
    let uri = &item.uri;

    let query = get_document_query(uri, documents)?;
    let symbol_id: u32 = item
        .data
        .as_ref()
        .and_then(|d| serde_json::from_value(d.clone()).ok())?;
    let symbol = query.index.symbol(symbol_id)?;

    let mut calls = Vec::new();

    for reference in query.index.references() {
        if !symbol.range.contains(reference.range.start()) {
            continue;
        }

        let Some(resolved_id) = reference.resolved else {
            continue;
        };
        let Some(target) = query.index.symbol(resolved_id) else {
            continue;
        };

        if !is_callable_symbol(target) {
            continue;
        }

        let target_name = query.index.symbol_name(target).to_string();

        calls.push(CallHierarchyOutgoingCall {
            to: CallHierarchyItem {
                name: target_name,
                kind: to_lsp_symbol_kind(target.kind),
                tags: None,
                detail: scope_container_name(query.index, target),
                uri: uri.clone(),
                range: to_lsp_range(target.range, query.line_index),
                selection_range: to_lsp_range(target.selection_range, query.line_index),
                data: Some(serde_json::to_value(target.id).unwrap()),
            },
            from_ranges: vec![to_lsp_range(reference.range, query.line_index)],
        });
    }

    if calls.is_empty() { None } else { Some(calls) }
}

pub fn handle_workspace_symbol(
    params: WorkspaceSymbolParams,
    documents: &HashMap<Uri, DocumentState>,
    analysis: &AnalysisHost,
    workspace: &Workspace,
) -> Option<Vec<SymbolInformation>> {
    let query = params.query.to_lowercase();
    let mut results = Vec::new();
    let mut seen_files: FxHashSet<Uri> = FxHashSet::default();

    for (uri, state) in documents {
        let Some(index) = state.document.index.as_ref() else {
            continue;
        };
        seen_files.insert(uri.clone());
        collect_matching_symbols(index, &state.document.line_index, uri, &query, &mut results);
    }

    for file_id in analysis.file_ids() {
        let vfs_path = workspace.vfs.file_path(file_id);
        let Some(file_uri) = vfs_path.to_uri() else {
            continue;
        };

        if seen_files.contains(&file_uri) {
            continue;
        }

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
    for symbol in index.symbols() {
        if !is_workspace_searchable(symbol) {
            continue;
        }

        let name = index.symbol_name(symbol);

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
