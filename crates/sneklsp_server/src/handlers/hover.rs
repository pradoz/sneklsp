use std::collections::HashMap;

use lsp_types::{
    Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, SignatureHelp,
    SignatureHelpParams, SignatureInformation, Uri,
};

use super::common::{
    DocumentQuery, from_lsp_position, get_document_query, is_callable_symbol, to_lsp_range,
};
use crate::builtins::BuiltinInfo;
use crate::server::DocumentState;
use sneklsp_index::{OwnedIndex, SymbolData};
use sneklsp_text::TextSize;

enum HoverTarget<'a> {
    Symbol(&'a SymbolData),
    Builtin(&'static BuiltinInfo),
}

enum CallTarget<'a> {
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
    if let Some(symbol) = query.find_symbol_at(offset) {
        return Some(HoverTarget::Symbol(symbol));
    }

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

fn find_call_context<'a>(index: &'a OwnedIndex, offset: TextSize) -> Option<(CallTarget<'a>, u32)> {
    let source = index.source();
    let cursor = offset.to_usize().min(source.len());
    let bytes = source.as_bytes();

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
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let paren_pos = paren_pos?;

    let active_param = count_commas_at_depth_zero(bytes, paren_pos + 1, cursor);

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
        return None;
    }

    let func_offset = TextSize::new(func_start as u32);
    let func_name = &source[func_start..name_end];

    if let Some(symbol) = index.symbol_at(func_offset) {
        if is_callable_symbol(symbol) {
            return Some((CallTarget::Symbol(symbol), active_param));
        }
    }

    if let Some(reference) = index.reference_at(func_offset) {
        if let Some(sym_id) = reference.resolved {
            if let Some(symbol) = index.symbol(sym_id) {
                if is_callable_symbol(symbol) {
                    return Some((CallTarget::Symbol(symbol), active_param));
                }
            }
        }
    }

    if let Some(builtin) = crate::builtins::lookup(func_name) {
        return Some((CallTarget::Builtin(builtin), active_param));
    }

    None
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
