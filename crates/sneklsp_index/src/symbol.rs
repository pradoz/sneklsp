use crate::scope::ScopeId;
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId(u32);

impl SymbolId {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Class,
    Variable,
    Parameter,
    Import,
    ImportedSymbol,
    Method,
    Property,
    TypeAlias,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Visibility {
    #[default]
    Public,
    Private,
    DunderPrivate,
}

impl Visibility {
    #[inline]
    pub fn from_name(name: &str) -> Self {
        if name.starts_with("__") && !name.ends_with("__") {
            Self::DunderPrivate
        } else if name.starts_with('_') {
            Self::Private
        } else {
            Self::Public
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbol<'src> {
    pub id: SymbolId,
    pub name: &'src str,
    pub kind: SymbolKind,
    pub range: TextRange,
    pub selection_range: TextRange,
    pub scope: ScopeId,
    pub visibility: Visibility,
}

impl<'src> Symbol<'src> {
    #[inline]
    pub fn new(
        id: SymbolId,
        name: &'src str,
        kind: SymbolKind,
        range: TextRange,
        selection_range: TextRange,
        scope: ScopeId,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            range,
            selection_range,
            scope,
            visibility: Visibility::from_name(name),
        }
    }
}
