use lsp_types::{CompletionItemKind, Location, Position, Range, SymbolKind, Uri};
use rustc_hash::FxHashMap;

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
pub fn ranges_overlap_lsp(a: Range, b: Range) -> bool {
    a.start.line <= b.end.line
        && b.start.line <= a.end.line
        && !(a.start.line == b.end.line && a.start.character > b.end.character)
        && !(b.start.line == a.end.line && b.start.character > a.end.character)
}

#[inline]
pub fn is_callable_symbol(symbol: &SymbolData) -> bool {
    matches!(
        symbol.kind,
        sneklsp_index::SymbolKind::Function
            | sneklsp_index::SymbolKind::Class
            | sneklsp_index::SymbolKind::Method
    )
}

pub fn scope_container_name(index: &OwnedIndex, symbol: &SymbolData) -> Option<String> {
    if symbol.scope == 0 {
        return None;
    }

    let scope = index.scope(symbol.scope)?;
    let parent_scope_id = scope.parent?;
    let parent_scope = index.scope(parent_scope_id)?;

    for &sym_id in &parent_scope.symbols {
        if let Some(parent_sym) = index.symbol(sym_id) {
            if parent_sym.range == scope.range {
                return Some(index.symbol_name(parent_sym).to_string());
            }
        }
    }

    None
}

pub struct DocumentQuery<'a> {
    pub index: &'a OwnedIndex,
    pub line_index: &'a LineIndex,
    pub uri: &'a Uri,
}

impl<'a> DocumentQuery<'a> {
    pub fn find_symbol_at(&self, offset: TextSize) -> Option<&'a SymbolData> {
        if let Some(symbol) = self.index.symbol_at(offset) {
            return Some(symbol);
        }

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
        range: TextRange,
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
    documents: &'a FxHashMap<Uri, DocumentState>,
) -> Option<DocumentQuery<'a>> {
    let state = documents.get(uri)?;
    let index = state.document.index.as_ref()?;
    Some(DocumentQuery {
        index,
        line_index: &state.document.line_index,
        uri,
    })
}

pub fn resolve_index_for_file<'a>(
    file_uri: &Uri,
    file_id: sneklsp_vfs::FileId,
    documents: &'a FxHashMap<Uri, DocumentState>,
    analysis: &'a crate::analysis::AnalysisHost,
) -> Option<(&'a OwnedIndex, &'a LineIndex)> {
    if let Some(state) = documents.get(file_uri) {
        let idx = state.document.index.as_ref()?;
        return Some((idx, &state.document.line_index));
    }

    let idx = analysis.file_index(file_id)?;
    let li = analysis.file_line_index(file_id)?;
    Some((idx, li))
}
