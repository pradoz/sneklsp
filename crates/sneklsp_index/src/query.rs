use crate::{ModuleIndex, Symbol, SymbolKind};
use sneklsp_text::{TextRange, TextSize};

#[derive(Debug)]
pub struct PrepareRenameResult<'a, 'src> {
    pub symbol: &'a Symbol<'src>,
    pub occurrences: Vec<TextRange>,
}

impl<'src> ModuleIndex<'src> {
    pub fn prepare_rename(&self, pos: TextSize) -> Option<PrepareRenameResult<'_, 'src>> {
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
