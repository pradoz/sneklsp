use crate::symbol::SymbolId;
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(u32);

impl ScopeId {
    pub const ROOT: ScopeId = ScopeId(0);

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

    #[inline]
    pub const fn is_root(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    Module,
    Class,
    Function,
    Comprehension,
    Lambda,
}

impl ScopeKind {
    #[inline]
    pub const fn is_local_namespace(self) -> bool {
        matches!(self, Self::Function | Self::Comprehension | Self::Lambda)
    }

    #[inline]
    pub const fn skip_in_resolution(self) -> bool {
        matches!(self, Self::Class)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub range: TextRange,
    pub symbols: Vec<SymbolId>,
    pub children: Vec<ScopeId>,
}

impl Scope {
    pub fn new(id: ScopeId, kind: ScopeKind, parent: Option<ScopeId>, range: TextRange) -> Self {
        Self {
            id,
            kind,
            parent,
            range,
            symbols: Vec::new(),
            children: Vec::new(),
        }
    }

    #[inline]
    pub fn module(range: TextRange) -> Self {
        Self::new(ScopeId::ROOT, ScopeKind::Module, None, range)
    }

    #[inline]
    pub fn add_symbol(&mut self, symbol: SymbolId) {
        self.symbols.push(symbol);
    }

    #[inline]
    pub fn add_child(&mut self, child: ScopeId) {
        self.children.push(child);
    }
}
