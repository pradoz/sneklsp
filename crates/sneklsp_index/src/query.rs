use crate::{
    ModuleIndex, Reference, ReferenceId, Scope, ScopeId, Symbol, SymbolId, SymbolKind, symbol,
};
use sneklsp_text::TextSize;

#[derive(Debug, Clone)]
pub enum DefinitionResult<'a, 'src> {
    Local(&'a Symbol<'src>),
    Import { module: &'src str, name: &'src str },
    Unresolved { name: &'src str },
}

#[derive(Debug, Clone)]
pub struct ReferenceInfo<'a, 'src> {
    pub reference: &'a Reference<'src>,
    pub is_definition: bool,
}

#[derive(Debug, Clone)]
pub struct ReferenceResult<'a, 'src> {
    pub symbol: &'a Symbol<'src>,
    pub references: Vec<ReferenceInfo<'a, 'src>>,
}

#[derive(Debug)]
pub struct DocumentSymbol<'a, 'src> {
    pub symbol: &'a Symbol<'src>,
    pub children: Vec<DocumentSymbol<'a, 'src>>,
}

/// Result of prepare rename query
#[derive(Debug)]
pub struct PrepareRenameResult<'a, 'src> {
    pub symbol: &'a Symbol<'src>,
    pub occurrences: Vec<sneklsp_text::TextRange>,
}

impl<'src> ModuleIndex<'src> {
    pub fn goto_definition(&self, pos: TextSize) -> Option<DefinitionResult> {
        // already on a definition
        if let Some(symbol) = self.definition_at(pos) {
            return Some(DefinitionResult::Local(symbol));
        }

        let reference = self.reference_at(pos)?;
        match reference.resolved {
            Some(symbol_id) => {
                let symbol = self.symbol(symbol_id);
                Some(DefinitionResult::Local(symbol))
            }
            None => Some(DefinitionResult::Unresolved {
                name: reference.name,
            }),
        }
    }

    pub fn find_references(&self, pos: TextSize) -> Option<ReferenceResult> {
        let symbol_id = if let Some(symbol) = self.definition_at(pos) {
            symbol.id
        } else if let Some(reference) = self.reference_at(pos) {
            reference.resolved?
        } else {
            return None;
        };

        let symbol = self.symbol(symbol_id);
        let mut references = Vec::new();

        references.push(ReferenceInfo {
            reference: &Reference::resolved(
                ReferenceId::new(u32::MAX), // dummy id for definition
                symbol.name,
                symbol.range,
                symbol_id,
            ),
            is_definition: true,
        });

        // TODO: cannot return temp reference - just return ranges
        Some(ReferenceResult { symbol, references })
    }

    pub fn document_symbols(&self) -> Vec<DocumentSymbol<'_, 'src>> {
        let root = self.root_scope();
        self.symbols_for_scope_recursive(root.id)
    }

    fn symbols_for_scope_recursive(&self, id: ScopeId) -> Vec<DocumentSymbol<'_, 'src>> {
        let scope = self.scope(id);
        let mut result = Vec::new();

        for &symbol_id in &scope.symbols {
            let symbol = self.symbol(symbol_id);
            let children = self.find_symbol_children(symbol);
            result.push(DocumentSymbol { symbol, children });
        }

        result
    }

    fn find_symbol_children(&self, symbol: &Symbol<'src>) -> Vec<DocumentSymbol<'_, 'src>> {
        match symbol.kind {
            SymbolKind::Function | SymbolKind::Method | SymbolKind::Class => {
                for scope in self.scopes() {
                    if scope.parent == Some(symbol.scope) && scope.range == symbol.range {
                        return self.symbols_for_scope_recursive(scope.id);
                    }
                }
            }
            _ => {}
        }

        Vec::new()
    }

    pub fn prepare_rename(&self, pos: TextSize) -> Option<PrepareRenameResult> {
        let symbol_id = if let Some(symbol) = self.definition_at(pos) {
            symbol.id
        } else if let Some(reference) = self.reference_at(pos) {
            reference.resolved?
        } else {
            return None;
        };

        let symbol = self.symbol(symbol_id);

        if matches!(symbol.kind, SymbolKind::Import | SymbolKind::ImportedSymbol) {
            // TODO: check if rename is allowed (not from an external module import)
        }

        let occurrences = self.all_occurrences(symbol_id);
        Some(PrepareRenameResult {
            symbol,
            occurrences,
        })
    }
}
