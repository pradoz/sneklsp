use bumpalo::Bump;

pub struct AstArena {
    bump: Bump,
}

impl AstArena {
    pub fn new() -> Self {
        Self { bump: Bump::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bump: Bump::with_capacity(capacity),
        }
    }

    #[inline]
    pub fn alloc<T>(&self, val: T) -> &T {
        self.bump.alloc(val)
    }

    #[inline]
    pub fn alloc_slice<T, I>(&self, iter: I) -> &[T]
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: ExactSizeIterator,
    {
        self.bump.alloc_slice_fill_iter(iter)
    }

    #[inline]
    pub fn alloc_str(&self, s: &str) -> &str {
        self.bump.alloc_str(s)
    }

    pub fn allocated_bytes(&self) -> usize {
        self.bump.allocated_bytes()
    }

    pub fn reset(&mut self) {
        self.bump.reset()
    }
}

impl Default for AstArena {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AstArena {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ASTArena")
            .field("allocated_bytes", &self.bump.allocated_bytes())
            .finish()
    }
}
