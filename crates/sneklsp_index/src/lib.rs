mod definition;
mod incremental;
mod indexer;
mod interval;
mod owned;
mod reference;
mod scope;
mod symbol;

pub use definition::Definition;
pub use incremental::{can_update_incrementally, find_affected_scopes};
pub use indexer::{Indexer, index_module};
pub use interval::IntervalTree;
pub use owned::{OwnedIndex, ReferenceData, ScopeData, SymbolData};
pub use reference::{Reference, ReferenceId};
pub use scope::{Scope, ScopeId, ScopeKind};
pub use symbol::{Symbol, SymbolId, SymbolKind, Visibility};

use rustc_hash::FxHashMap;
use sneklsp_text::{TextRange, TextSize};

#[derive(Debug)]
pub struct ModuleIndex<'src> {
    symbols: Vec<Symbol<'src>>,
    scopes: Vec<Scope>,
    definitions: Vec<Definition>,
    references: Vec<Reference<'src>>,
    name_to_symbols: FxHashMap<&'src str, Vec<SymbolId>>,
    scope_tree: IntervalTree<ScopeId>,
    reference_tree: IntervalTree<ReferenceId>,
    definition_tree: IntervalTree<SymbolId>,
}

impl<'src> ModuleIndex<'src> {
    pub fn new() -> Self {
        Self {
            symbols: Vec::new(),
            scopes: Vec::new(),
            definitions: Vec::new(),
            references: Vec::new(),
            name_to_symbols: FxHashMap::default(),
            scope_tree: IntervalTree::new(),
            reference_tree: IntervalTree::new(),
            definition_tree: IntervalTree::new(),
        }
    }

    pub fn set_symbol_docstring(&mut self, id: SymbolId, docstring: &'src str) {
        self.symbols[id.as_usize()].docstring = Some(docstring);
    }

    pub fn add_module_scope(&mut self, range: TextRange) -> ScopeId {
        debug_assert!(self.scopes.is_empty(), "module scope must be first");
        let scope = Scope::module(range);
        self.scopes.push(scope);
        ScopeId::ROOT
    }

    pub fn add_scope(&mut self, kind: ScopeKind, parent: ScopeId, range: TextRange) -> ScopeId {
        let id = ScopeId::new(self.scopes.len() as u32);
        let scope = Scope::new(id, kind, Some(parent), range);
        self.scopes.push(scope);
        self.scopes[parent.as_usize()].add_child(id);
        id
    }

    pub fn add_symbol(
        &mut self,
        name: &'src str,
        kind: SymbolKind,
        range: TextRange,
        selection_range: TextRange,
        scope: ScopeId,
    ) -> SymbolId {
        let id = SymbolId::new(self.symbols.len() as u32);
        let symbol = Symbol::new(id, name, kind, range, selection_range, scope);
        self.symbols.push(symbol);
        self.scopes[scope.as_usize()].add_symbol(id);
        self.name_to_symbols
            .entry(name)
            .or_insert_with(Vec::new)
            .push(id);
        self.definitions.push(Definition::new(id, selection_range));
        id
    }

    pub fn add_reference(
        &mut self,
        name: &'src str,
        range: TextRange,
        resolved: Option<SymbolId>,
    ) -> ReferenceId {
        let id = ReferenceId::new(self.references.len() as u32);
        let reference = match resolved {
            Some(symbol) => Reference::resolved(id, name, range, symbol),
            None => Reference::unresolved(id, name, range),
        };
        self.references.push(reference);
        id
    }

    pub fn finish(&mut self) {
        self.scope_tree = IntervalTree::with_capacity(self.scopes.len());
        for scope in &self.scopes {
            self.scope_tree.insert(scope.range, scope.id);
        }
        self.scope_tree.finish();

        self.reference_tree = IntervalTree::with_capacity(self.references.len());
        for reference in &self.references {
            self.reference_tree.insert(reference.range, reference.id);
        }
        self.reference_tree.finish();

        self.definition_tree = IntervalTree::with_capacity(self.definitions.len());
        for definition in &self.definitions {
            self.definition_tree
                .insert(definition.range, definition.symbol);
        }
    }

    #[inline]
    pub fn symbol(&self, id: SymbolId) -> &Symbol<'src> {
        &self.symbols[id.as_usize()]
    }

    #[inline]
    pub fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.as_usize()]
    }

    #[inline]
    pub fn reference(&self, id: ReferenceId) -> &Reference<'src> {
        &self.references[id.as_usize()]
    }

    #[inline]
    pub fn root_scope(&self) -> &Scope {
        &self.scopes[ScopeId::ROOT.as_usize()]
    }

    #[inline]
    pub fn symbols(&self) -> &[Symbol<'src>] {
        &self.symbols
    }

    #[inline]
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    #[inline]
    pub fn references(&self) -> &[Reference<'src>] {
        &self.references
    }

    pub fn scope_at(&self, pos: TextSize) -> Option<&Scope> {
        self.scope_tree
            .find_innermost(pos)
            .map(|id| &self.scopes[id.as_usize()])
    }

    pub fn definition_at(&self, pos: TextSize) -> Option<&Symbol<'src>> {
        self.definition_tree
            .find_innermost(pos)
            .map(|id| &self.symbols[id.as_usize()])
    }

    pub fn reference_at(&self, pos: TextSize) -> Option<&Reference<'src>> {
        self.reference_tree
            .find_innermost(pos)
            .map(|id| &self.references[id.as_usize()])
    }

    pub fn references_to(&self, symbol: SymbolId) -> impl Iterator<Item = &Reference<'src>> {
        self.references
            .iter()
            .filter(move |r| r.resolved == Some(symbol))
    }

    pub fn resolve_name(&self, name: &str, from_scope: ScopeId) -> Option<SymbolId> {
        let mut current = Some(from_scope);

        // resolve with LEGB (local, enclosing, global, builtin)
        while let Some(scope_id) = current {
            let scope = self.scope(scope_id);

            // skip class scopes for nested lookups
            if !scope.kind.skip_in_resolution() || scope_id == from_scope {
                for &symbol_id in &scope.symbols {
                    if self.symbol(symbol_id).name == name {
                        return Some(symbol_id);
                    }
                }
            }

            current = scope.parent;
        }

        None // unresolved
    }

    pub fn all_occurrences(&self, symbol: SymbolId) -> Vec<TextRange> {
        let mut ranges = Vec::new();
        ranges.push(self.symbol(symbol).selection_range);

        for reference in &self.references {
            if reference.resolved == Some(symbol) {
                ranges.push(reference.range);
            }
        }

        ranges
    }
}

impl Default for ModuleIndex<'_> {
    fn default() -> Self {
        Self::new()
    }
}
