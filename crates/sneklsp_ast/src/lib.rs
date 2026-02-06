mod arena;
mod expr;
mod stmt;

pub use arena::AstArena;
pub use expr::*;
pub use stmt::*;

use sneklsp_text::TextRange;

pub type Identifier<'ast> = &'ast str;

impl<'ast> Parameters<'ast> {
    pub const fn empty() -> Self {
        Self {
            posonlyargs: &[],
            args: &[],
            vararg: None,
            kwonlyargs: &[],
            kw_defaults: &[],
            kwarg: None,
            defaults: &[],
            range: TextRange::new(
                sneklsp_text::TextSize::new(0),
                sneklsp_text::TextSize::new(0),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Module<'ast> {
    pub body: &'ast [Statement<'ast>],
    pub range: TextRange,
}
