use std::sync::{Arc, OnceLock};

use crate::ModuleIndex;
use sneklsp_text::TextRange;

/// sorted by start
struct PositionalIndex {
    /// (start, end, symbol_index)
    symbols: Vec<(u32, u32, u32)>,
    /// (start, end, reference_index)
    references: Vec<(u32, u32, u32)>,
    /// (start, end, scope_index) sorted after start by width descending
    scopes: Vec<(u32, u32, u32)>,
}

impl PositionalIndex {
    fn build(symbols: &[SymbolData], references: &[ReferenceData], scopes: &[ScopeData]) -> Self {
        let mut sym_entries: Vec<(u32, u32, u32)> = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.selection_range.start().to_u32(),
                    s.selection_range.end().to_u32(),
                    i as u32,
                )
            })
            .collect();
        sym_entries.sort_unstable_by_key(|e| e.0);

        let mut ref_entries: Vec<(u32, u32, u32)> = references
            .iter()
            .enumerate()
            .map(|(i, r)| (r.range.start().to_u32(), r.range.end().to_u32(), i as u32))
            .collect();
        ref_entries.sort_unstable_by_key(|e| e.0);

        let mut scope_entries: Vec<(u32, u32, u32)> = scopes
            .iter()
            .enumerate()
            .map(|(i, s)| (s.range.start().to_u32(), s.range.end().to_u32(), i as u32))
            .collect();
        scope_entries.sort_unstable_by_key(|e| e.0);

        Self {
            symbols: sym_entries,
            references: ref_entries,
            scopes: scope_entries,
        }
    }

    #[inline]
    fn find_innermost(entries: &[(u32, u32, u32)], offset: u32) -> Option<u32> {
        if entries.is_empty() {
            return None;
        }

        let partition = entries.partition_point(|e| e.0 <= offset);
        if partition == 0 {
            return None;
        }

        let mut best: Option<(u32, u32)> = None; // (width, index)
        //
        // all of these entries have start <= offset
        for entry in entries[..partition].iter().rev() {
            if let Some((best_width, _)) = best {
                if offset - entry.0 >= best_width {
                    break;
                }
            }

            // check if contained
            if offset < entry.1 {
                let width = entry.1 - entry.0;
                match best {
                    Some((bw, _)) if width < bw => best = Some((width, entry.2)),
                    None => best = Some((width, entry.2)),
                    _ => {}
                }
            }
        }

        best.map(|(_, idx)| idx)
    }
}

pub struct OwnedIndex {
    inner: Arc<OwnedIndexInner>,
}

pub struct OwnedIndexInner {
    source: String,
    symbols: Vec<SymbolData>,
    scopes: Vec<ScopeData>,
    references: Vec<ReferenceData>,
    positional: OnceLock<PositionalIndex>,
}

impl Clone for OwnedIndex {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl PartialEq for OwnedIndex {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for OwnedIndex {}

impl std::hash::Hash for OwnedIndex {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct SymbolData {
    pub id: u32,
    pub name_start: u32,
    pub name_len: u16,
    pub kind: crate::SymbolKind,
    pub range: TextRange,
    pub selection_range: TextRange,
    pub scope: u32,
    pub visibility: crate::Visibility,
    pub docstring_start: Option<u32>,
    pub docstring_len: u32,
    pub signature_range: Option<TextRange>,
}

#[derive(Debug, Clone)]
pub struct ScopeData {
    pub id: u32,
    pub kind: crate::ScopeKind,
    pub parent: Option<u32>,
    pub range: TextRange,
    pub symbols: Vec<u32>,
    pub children: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct ReferenceData {
    pub id: u32,
    pub name_start: u32,
    pub name_len: u16,
    pub range: TextRange,
    pub resolved: Option<u32>,
}

impl OwnedIndex {
    pub fn new(source: String, index: &ModuleIndex<'_>) -> Self {
        let symbols = index
            .symbols()
            .iter()
            .map(|s| {
                let name_start = s.selection_range.start().to_u32();
                let name_len = s.name.len() as u16;
                let (docstring_start, docstring_len) = match s.docstring {
                    Some(doc) => {
                        let search_start = s.range.start().to_usize();
                        let search_end = (s.range.end().to_usize()).min(source.len());
                        let search_region = &source[search_start..search_end];

                        if let Some(offset) = search_region.find(doc) {
                            (Some((search_start + offset) as u32), doc.len() as u32)
                        } else {
                            (None, 0)
                        }
                    }
                    None => (None, 0),
                };
                let signature_range = compute_signature_range(s, &source);

                SymbolData {
                    id: s.id.as_u32(),
                    name_start,
                    name_len,
                    kind: s.kind,
                    range: s.range,
                    selection_range: s.selection_range,
                    scope: s.scope.as_u32(),
                    visibility: s.visibility,
                    docstring_start,
                    docstring_len,
                    signature_range,
                }
            })
            .collect();

        let scopes = index
            .scopes()
            .iter()
            .map(|s| ScopeData {
                id: s.id.as_u32(),
                kind: s.kind,
                parent: s.parent.map(|p| p.as_u32()),
                range: s.range,
                symbols: s.symbols.iter().map(|id| id.as_u32()).collect(),
                children: s.children.iter().map(|id| id.as_u32()).collect(),
            })
            .collect();

        let references = index
            .references()
            .iter()
            .map(|r| {
                let name_start = r.range.start().to_u32();
                let name_len = r.name.len() as u16;
                ReferenceData {
                    id: r.id.as_u32(),
                    name_start,
                    name_len,
                    range: r.range,
                    resolved: r.resolved.map(|id| id.as_u32()),
                }
            })
            .collect();

        Self {
            inner: Arc::new(OwnedIndexInner {
                source,
                symbols,
                scopes,
                references,
                positional: OnceLock::new(),
            }),
        }
    }

    #[inline]
    fn positional(&self) -> &PositionalIndex {
        self.inner.positional.get_or_init(|| {
            PositionalIndex::build(
                &self.inner.symbols,
                &self.inner.references,
                &self.inner.scopes,
            )
        })
    }

    #[inline]
    pub fn source(&self) -> &str {
        &self.inner.source
    }

    #[inline]
    pub fn into_source(self) -> String {
        match Arc::try_unwrap(self.inner) {
            Ok(inner) => inner.source,
            Err(arc) => arc.source.clone(),
        }
    }

    #[inline]
    pub fn symbol_name(&self, symbol: &SymbolData) -> &str {
        let start = symbol.name_start as usize;
        let end = start + symbol.name_len as usize;
        &self.inner.source[start..end]
    }

    #[inline]
    pub fn reference_name(&self, reference: &ReferenceData) -> &str {
        let start = reference.name_start as usize;
        let end = start + reference.name_len as usize;
        &self.inner.source[start..end]
    }

    #[inline]
    pub fn symbols(&self) -> &[SymbolData] {
        &self.inner.symbols
    }

    #[inline]
    pub fn scopes(&self) -> &[ScopeData] {
        &self.inner.scopes
    }

    #[inline]
    pub fn references(&self) -> &[ReferenceData] {
        &self.inner.references
    }

    #[inline]
    pub fn symbol(&self, id: u32) -> Option<&SymbolData> {
        self.inner.symbols.get(id as usize)
    }

    #[inline]
    pub fn scope(&self, id: u32) -> Option<&ScopeData> {
        self.inner.scopes.get(id as usize)
    }

    #[inline]
    pub fn root_scope(&self) -> Option<&ScopeData> {
        self.inner.scopes.first()
    }

    #[inline]
    pub fn symbol_docstring(&self, symbol: &SymbolData) -> Option<&str> {
        let start = symbol.docstring_start? as usize;
        let end = start + symbol.docstring_len as usize;
        if end <= self.inner.source.len() {
            Some(&self.inner.source[start..end])
        } else {
            None
        }
    }

    #[inline]
    pub fn symbol_signature(&self, symbol: &SymbolData) -> Option<&str> {
        let range = symbol.signature_range?;
        let start = range.start().to_usize();
        let end = range.end().to_usize();
        if end <= self.inner.source.len() {
            Some(&self.inner.source[start..end])
        } else {
            None
        }
    }

    pub fn symbol_at(&self, offset: sneklsp_text::TextSize) -> Option<&SymbolData> {
        let idx = PositionalIndex::find_innermost(&self.positional().symbols, offset.to_u32())?;
        self.inner.symbols.get(idx as usize)
    }

    pub fn scope_at(&self, offset: sneklsp_text::TextSize) -> Option<&ScopeData> {
        let idx = PositionalIndex::find_innermost(&self.positional().scopes, offset.to_u32())?;
        self.inner.scopes.get(idx as usize)
    }

    pub fn reference_at(&self, offset: sneklsp_text::TextSize) -> Option<&ReferenceData> {
        let idx = PositionalIndex::find_innermost(&self.positional().references, offset.to_u32())?;
        self.inner.references.get(idx as usize)
    }

    pub fn references_to(&self, symbol_id: u32) -> impl Iterator<Item = &ReferenceData> {
        self.inner
            .references
            .iter()
            .filter(move |r| r.resolved == Some(symbol_id))
    }
}

fn compute_signature_range(symbol: &crate::Symbol<'_>, source: &str) -> Option<TextRange> {
    match symbol.kind {
        crate::SymbolKind::Function | crate::SymbolKind::Method | crate::SymbolKind::Class => {}
        _ => return None,
    }

    let start = symbol.range.start().to_usize();
    let end = symbol.range.end().to_usize().min(source.len());
    let slice = &source[start..end];

    // find colon that starts block body (not inside bracketry)
    let mut depth = 0u32;
    let mut colon_offset = None;
    for (i, b) in slice.bytes().enumerate() {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b':' if depth == 0 => {
                colon_offset = Some(i);
                break;
            }
            b'\n' if depth == 0 => break,
            _ => {}
        }
    }

    let sig_end = colon_offset.unwrap_or(slice.len());

    let sig_text = &slice[..sig_end]; // trim
    let trimmed_len = sig_text.trim_end().len();

    if trimmed_len == 0 {
        return None;
    }

    Some(TextRange::new(
        sneklsp_text::TextSize::new(start as u32),
        sneklsp_text::TextSize::new((start + trimmed_len) as u32),
    ))
}

impl std::fmt::Debug for OwnedIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedIndex")
            .field("source_len", &self.inner.source.len())
            .field("symbols", &self.inner.symbols.len())
            .field("scopes", &self.inner.scopes.len())
            .field("references", &self.inner.references.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index_module;
    use sneklsp_ast::AstArena;

    #[test]
    fn roundtrip_symbol_names() {
        let source = "def foo():\n    x = 1".to_string();
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(&source, &arena).unwrap();
        let index = index_module(&source, &module);

        let owned = OwnedIndex::new(source.to_string(), &index);

        for (orig, data) in index.symbols().iter().zip(owned.symbols()) {
            assert_eq!(orig.name, owned.symbol_name(data));
        }
    }

    #[test]
    fn symbol_lookup() {
        let source = "x = 1".to_string();
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(&source, &arena).unwrap();
        let index = index_module(&source, &module);

        let owned = OwnedIndex::new(source.to_string(), &index);

        let sym = owned.symbol_at(sneklsp_text::TextSize::new(0)).unwrap();
        assert_eq!(owned.symbol_name(sym), "x");
    }

    #[test]
    fn cheap_clone_is_arc() {
        let source = "x = 1\ny = 2".to_string();
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(&source, &arena).unwrap();
        let index = index_module(&source, &module);

        let owned = OwnedIndex::new(source.to_string(), &index);
        let cloned = owned.clone();

        // same Arc pointer — no data copied
        assert_eq!(owned, cloned);
        assert_eq!(owned.symbols().len(), cloned.symbols().len());
    }
}
