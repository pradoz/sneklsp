mod arena;
mod expr;
mod stmt;

pub use arena::AstArena;
pub use expr::*;
pub use stmt::*;

use sneklsp_text::TextRange;

pub type Identifier<'ast> = &'ast str;

// macro generates range() for enums where every variant wraps a type with .range
macro_rules! impl_ranged_enum {
    ($name:ident<$lt:lifetime>, $($variant:ident),+ $(,)?) => {
        impl<$lt> $name<$lt> {
            pub const fn range(&self) -> TextRange {
                match self {
                    $(Self::$variant(inner) => inner.range,)+
                }
            }
        }
    };
}

impl_ranged_enum!(
    Expression<'ast>,
    Name,
    Int,
    Float,
    String,
    FString,
    Bytes,
    Bool,
    None,
    Ellipsis,
    BinOp,
    UnaryOp,
    BoolOp,
    Compare,
    Call,
    Attribute,
    Subscript,
    List,
    Tuple,
    Dict,
    Set,
    Lambda,
    IfExp,
    ListComp,
    SetComp,
    DictComp,
    GeneratorExp,
    Yield,
    YieldFrom,
    Await,
    Starred,
    Named,
    Slice,
);

impl_ranged_enum!(
    Statement<'ast>,
    FunctionDef,
    ClassDef,
    Return,
    Assign,
    AugAssign,
    AnnAssign,
    If,
    For,
    While,
    With,
    Try,
    Raise,
    Assert,
    Import,
    ImportFrom,
    Global,
    Nonlocal,
    Expr,
    Delete,
    Pass,
    Break,
    Continue,
);

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
