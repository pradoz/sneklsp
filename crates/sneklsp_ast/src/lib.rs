mod expr;
mod stmt;

pub use expr::*;
pub use stmt::*;

use compact_str::CompactString;
use sneklsp_text::TextRange;

pub type Identifier = CompactString;

// this is the root. I will call you, root
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub body: Vec<Statement>,
    pub range: TextRange,
}
