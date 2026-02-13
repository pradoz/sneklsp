use lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse, InsertTextFormat,
    Position, Range, TextEdit, Uri,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::common::{
    from_lsp_position, get_document_query, is_callable_kind, to_lsp_completion_kind,
};
use crate::analysis::AnalysisHost;
use crate::builtins::BUILTINS;
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, SymbolData};
use sneklsp_text::{LineIndex, TextSize};
use sneklsp_workspace::Workspace;

enum CompletionContext<'a> {
    SelfDot { class_scope_id: u32 },
    ModuleDot { module_name: &'a str },
    AttributeDot { symbol: &'a SymbolData },
    General,
}

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

    let prefix = extract_prefix(query.index.source(), offset);
    let ctx = detect_context(query.index, offset, &prefix);

    let mut items = Vec::new();
    let mut seen = FxHashSet::default();

    match ctx {
        CompletionContext::SelfDot { class_scope_id } => {
            collect_class_members(query.index, class_scope_id, &mut seen, &mut items);
        }
        CompletionContext::ModuleDot { module_name } => {
            collect_module_exports(module_name, analysis, workspace, &mut seen, &mut items);
        }
        CompletionContext::AttributeDot { symbol } => {
            let mut found_members = false;
            if symbol.kind == sneklsp_index::SymbolKind::Class {
                for scope in query.index.scopes() {
                    if scope.range == symbol.range && scope.kind == sneklsp_index::ScopeKind::Class
                    {
                        collect_class_members(query.index, scope.id, &mut seen, &mut items);
                        found_members = true;
                        break;
                    }
                }
            }
            if !found_members {
                let scope = query.index.scope_at(offset);
                let scope_id = scope.map(|s| s.id);
                collect_visible_symbols(query.index, scope_id, &mut seen, &mut items);
                add_builtin_completions(&mut seen, &mut items);
            }
        }
        CompletionContext::General => {
            let scope = query.index.scope_at(offset);
            let scope_id = scope.map(|s| s.id);
            collect_visible_symbols(query.index, scope_id, &mut seen, &mut items);

            let import_ctx = build_import_context(query.index, query.line_index);
            collect_cross_file_completions(analysis, workspace, &import_ctx, &mut seen, &mut items);

            add_builtin_completions(&mut seen, &mut items);
            add_keyword_completions(query.index, query.line_index, offset, &mut items);
        }
    }

    Some(CompletionResponse::Array(items))
}

fn extract_prefix(source: &str, offset: TextSize) -> String {
    let cursor = offset.to_usize().min(source.len());
    let bytes = source.as_bytes();

    let mut start = cursor;
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }

    // prefix should be empty after a dot
    if start > 0 && bytes[start - 1] == b'.' {
        return String::new();
    }

    source[start..cursor].to_string()
}

fn detect_context<'a>(
    index: &'a OwnedIndex,
    offset: TextSize,
    prefix: &str,
) -> CompletionContext<'a> {
    let source = index.source();
    let cursor = offset.to_usize().min(source.len());

    // walk back past the prefix to check for a dot
    let before_prefix = cursor.saturating_sub(prefix.len());
    let trimmed = source[..before_prefix].trim_end();
    if !trimmed.ends_with('.') {
        return CompletionContext::General;
    }

    let dot_pos = trimmed.len() - 1;
    let before_dot = trimmed[..dot_pos].trim_end();

    let name_start = before_dot
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let name = &before_dot[name_start..];

    if name.is_empty() {
        return CompletionContext::General;
    }

    if name == "self" || name == "cls" {
        if let Some(class_scope_id) = find_enclosing_class_scope(index, offset) {
            return CompletionContext::SelfDot { class_scope_id };
        }
    }

    let scope = index.scope_at(offset);
    let scope_id = scope.map(|s| s.id).unwrap_or(0);
    let symbol = index.resolve_name(name, scope_id);

    match symbol {
        Some(sym)
            if matches!(
                sym.kind,
                sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
            ) =>
        {
            CompletionContext::ModuleDot {
                module_name: index.symbol_name(sym),
            }
        }
        Some(sym) => CompletionContext::AttributeDot { symbol: sym },
        None => CompletionContext::General,
    }
}

fn find_enclosing_class_scope(index: &OwnedIndex, offset: TextSize) -> Option<u32> {
    let scope = index.scope_at(offset)?;
    if scope.kind != sneklsp_index::ScopeKind::Function {
        return None;
    }

    let parent_id = scope.parent?;
    let parent = index.scope(parent_id)?;

    if parent.kind == sneklsp_index::ScopeKind::Class {
        Some(parent.id)
    } else {
        None
    }
}

fn collect_class_members(
    index: &OwnedIndex,
    class_scope_id: u32,
    seen: &mut FxHashSet<String>,
    items: &mut Vec<CompletionItem>,
) {
    let Some(scope) = index.scope(class_scope_id) else {
        return;
    };

    for &sym_id in &scope.symbols {
        let Some(symbol) = index.symbol(sym_id) else {
            continue;
        };

        let name = index.symbol_name(symbol);

        if !seen.insert(name.to_string()) {
            continue;
        }

        let (insert_text, insert_format) = callable_insert_text(name, symbol.kind);

        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(to_lsp_completion_kind(symbol.kind)),
            detail: member_detail(symbol, index),
            insert_text,
            insert_text_format: insert_format,
            sort_text: Some(format!("0{}", name)),
            ..Default::default()
        });
    }

    for &child_id in &scope.children {
        let Some(child_scope) = index.scope(child_id) else {
            continue;
        };
        if child_scope.kind != sneklsp_index::ScopeKind::Function {
            continue;
        }

        let is_init = scope.symbols.iter().any(|&sym_id| {
            index.symbol(sym_id).map_or(false, |s| {
                s.kind == sneklsp_index::SymbolKind::Method
                    && s.range == child_scope.range
                    && index.symbol_name(s) == "__init__"
            })
        });

        if !is_init {
            continue;
        }

        for &sym_id in &child_scope.symbols {
            let Some(symbol) = index.symbol(sym_id) else {
                continue;
            };
            if symbol.kind == sneklsp_index::SymbolKind::Parameter {
                continue;
            }
            if symbol.kind != sneklsp_index::SymbolKind::Variable {
                continue;
            }

            let name = index.symbol_name(symbol);
            if seen.insert(name.to_string()) {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some("instance attribute".to_string()),
                    sort_text: Some(format!("0{}", name)),
                    ..Default::default()
                });
            }
        }
    }
}

fn collect_module_exports(
    module_name: &str,
    analysis: &AnalysisHost,
    workspace: &Workspace,
    seen: &mut FxHashSet<String>,
    items: &mut Vec<CompletionItem>,
) {
    // try salsa resolution first
    if let Some(target_file) = analysis.resolve_module_file(module_name) {
        let db = analysis.db();
        let exports = sneklsp_db::file_exported_symbols(db, target_file);
        for export in exports {
            if seen.insert(export.name.clone()) {
                let (insert_text, insert_format) = callable_insert_text(&export.name, export.kind);
                items.push(CompletionItem {
                    label: export.name.clone(),
                    kind: Some(to_lsp_completion_kind(export.kind)),
                    detail: Some(module_name.to_string()),
                    insert_text,
                    insert_text_format: insert_format,
                    sort_text: Some(format!("0{}", export.name)),
                    ..Default::default()
                });
            }
        }
        return;
    }

    // fallback to workspace vfs
    if let Some(file_id) = workspace.resolve_module(module_name) {
        if let Some(exports) = analysis.exported_symbols(file_id) {
            for export in exports {
                if seen.insert(export.name.clone()) {
                    let (insert_text, insert_format) =
                        callable_insert_text(&export.name, export.kind);
                    items.push(CompletionItem {
                        label: export.name.clone(),
                        kind: Some(to_lsp_completion_kind(export.kind)),
                        detail: Some(module_name.to_string()),
                        insert_text,
                        insert_text_format: insert_format,
                        sort_text: Some(format!("0{}", export.name)),
                        ..Default::default()
                    });
                }
            }
        }
    }
}

fn add_keyword_completions(
    index: &OwnedIndex,
    _line_index: &LineIndex,
    offset: TextSize,
    items: &mut Vec<CompletionItem>,
) {
    let source = index.source();
    let cursor = offset.to_usize().min(source.len());

    let line_start = source[..cursor].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let line_prefix = source[line_start..cursor].trim_start();

    let is_line_start =
        line_prefix.is_empty() || line_prefix.chars().all(|c| c.is_alphabetic() || c == '_');

    if !is_line_start {
        return;
    }

    let scope = index.scope_at(offset);
    let in_function = scope.map_or(false, |s| s.kind == sneklsp_index::ScopeKind::Function);

    let stmt_keywords = [
        ("def", "def ${1:name}($2):\n\t$0"),
        ("class", "class ${1:Name}:\n\t$0"),
        ("if", "if ${1:condition}:\n\t$0"),
        ("elif", "elif ${1:condition}:\n\t$0"),
        ("else", "else:\n\t$0"),
        ("for", "for ${1:item} in ${2:iterable}:\n\t$0"),
        ("while", "while ${1:condition}:\n\t$0"),
        ("try", "try:\n\t${1:pass}\nexcept ${2:Exception}:\n\t$0"),
        ("with", "with ${1:expr} as ${2:name}:\n\t$0"),
        ("import", "import $0"),
        ("from", "from ${1:module} import $0"),
        ("raise", "raise $0"),
        ("assert", "assert $0"),
        ("pass", "pass"),
        ("break", "break"),
        ("continue", "continue"),
        ("return", "return $0"),
    ];

    for (kw, snippet) in &stmt_keywords {
        if (*kw == "return" || *kw == "break" || *kw == "continue") && !in_function {
            continue;
        }

        if !line_prefix.is_empty() && !kw.starts_with(line_prefix) {
            continue;
        }

        items.push(CompletionItem {
            label: kw.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("keyword".to_string()),
            insert_text: Some(snippet.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            sort_text: Some(format!("~{}", kw)),
            ..Default::default()
        });
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
                            let (insert_text, insert_format) =
                                callable_insert_text(&name, symbol.kind);

                            let sort_prefix = if scope_id == Some(sid) { "1" } else { "2" };

                            items.push(CompletionItem {
                                label: name.clone(),
                                kind: Some(to_lsp_completion_kind(symbol.kind)),
                                detail: symbol_detail(symbol),
                                insert_text,
                                insert_text_format: insert_format,
                                sort_text: Some(format!("{}{}", sort_prefix, name)),
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

fn member_detail(symbol: &SymbolData, index: &OwnedIndex) -> Option<String> {
    match symbol.kind {
        sneklsp_index::SymbolKind::Method | sneklsp_index::SymbolKind::Function => {
            index.symbol_signature(symbol).map(|s| s.to_string())
        }
        sneklsp_index::SymbolKind::Class => Some("class".to_string()),
        sneklsp_index::SymbolKind::Variable => Some("attribute".to_string()),
        sneklsp_index::SymbolKind::Property => Some("property".to_string()),
        _ => None,
    }
}

fn build_import_context(index: &OwnedIndex, line_index: &LineIndex) -> ImportContext {
    let mut last_import_line: Option<u32> = None;
    let mut existing_imports = FxHashSet::default();

    for symbol in index.symbols() {
        if matches!(
            symbol.kind,
            sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol
        ) {
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

            if seen.contains(&export.name) {
                continue;
            }
            seen.insert(export.name.clone());

            let edit = make_import_edit(&module_name, &export.name, import_ctx.insert_line);
            let (insert_text, insert_format) = callable_insert_text(&export.name, export.kind);

            items.push(CompletionItem {
                label: export.name.clone(),
                kind: Some(to_lsp_completion_kind(export.kind)),
                detail: Some(format!("from {}", module_name)),
                insert_text,
                insert_text_format: insert_format,
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

#[inline]
fn callable_insert_text(
    name: &str,
    kind: sneklsp_index::SymbolKind,
) -> (Option<String>, Option<InsertTextFormat>) {
    if is_callable_kind(kind) {
        (
            Some(format!("{}($0)", name)),
            Some(InsertTextFormat::SNIPPET),
        )
    } else {
        (None, None)
    }
}
