mod arena;
mod expr;
mod stmt;

pub use arena::AstArena;
pub use expr::*;
pub use stmt::*;

use sneklsp_text::TextRange;

pub type Identifier<'ast> = &'ast str;

// this is the root. I will call you, root
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Module<'ast> {
    pub body: &'ast [Statement<'ast>],
    pub range: TextRange,
}
