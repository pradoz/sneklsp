use crate::symbol::SymbolId;
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceId(u32);

impl ReferenceId {
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference<'src> {
    pub id: ReferenceId,
    pub name: &'src str,
    pub range: TextRange,
    pub resolved: Option<SymbolId>,
}

impl<'src> Reference<'src> {
    #[inline]
    pub const fn unresolved(id: ReferenceId, name: &'src str, range: TextRange) -> Self {
        Self {
            id,
            name,
            range,
            resolved: None,
        }
    }

    #[inline]
    pub const fn resolved(
        id: ReferenceId,
        name: &'src str,
        range: TextRange,
        symbol: SymbolId,
    ) -> Self {
        Self {
            id,
            name,
            range,
            resolved: Some(symbol),
        }
    }

    #[inline]
    pub const fn is_resolved(&self) -> bool {
        self.resolved.is_some()
    }
}
