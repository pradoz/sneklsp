use crate::symbol::SymbolId;
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Definition {
    pub symbol: SymbolId,
    pub range: TextRange,
}

impl Definition {
    #[inline]
    pub const fn new(symbol: SymbolId, range: TextRange) -> Self {
        Self { symbol, range }
    }
}
