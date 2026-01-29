use crate::Identifier;
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expression<'ast> {
    Name(&'ast NameExpr<'ast>),
    Int(&'ast IntExpr),
    Float(&'ast FloatExpr),
    String(&'ast StringExpr<'ast>),
    Bool(&'ast BoolExpr),
    None(&'ast NoneExpr),
    BinOp(&'ast BinOpExpr<'ast>),
    UnaryOp(&'ast UnaryOpExpr<'ast>),
    Compare(&'ast CompareExpr<'ast>),
    Call(&'ast CallExpr<'ast>),
    Attribute(&'ast AttributeExpr<'ast>),
    Subscript(&'ast SubscriptExpr<'ast>),
    List(&'ast ListExpr<'ast>),
    Tuple(&'ast TupleExpr<'ast>),
    Dict(&'ast DictExpr<'ast>),
}

impl<'ast> Expression<'ast> {
    pub const fn range(&self) -> TextRange {
        match self {
            Expression::Name(e) => e.range,
            Expression::Int(e) => e.range,
            Expression::Float(e) => e.range,
            Expression::String(e) => e.range,
            Expression::Bool(e) => e.range,
            Expression::None(e) => e.range,
            Expression::BinOp(e) => e.range,
            Expression::UnaryOp(e) => e.range,
            Expression::Compare(e) => e.range,
            Expression::Call(e) => e.range,
            Expression::Attribute(e) => e.range,
            Expression::Subscript(e) => e.range,
            Expression::List(e) => e.range,
            Expression::Tuple(e) => e.range,
            Expression::Dict(e) => e.range,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NameExpr<'ast> {
    pub id: Identifier<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntExpr {
    pub value: i64,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloatExpr {
    pub value: f64,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StringExpr<'ast> {
    pub value: &'ast str,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoolExpr {
    pub value: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoneExpr {
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinOpExpr<'ast> {
    pub left: Expression<'ast>,
    pub op: BinOp,
    pub right: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,      // +
    Sub,      // -
    Mult,     // *
    Div,      // /
    FloorDiv, // //
    Mod,      // %
    Pow,      // **
    BitOr,    // |
    BitXor,   // ^
    BitAnd,   // &
    LShift,   // <<
    RShift,   // >>
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnaryOpExpr<'ast> {
    pub op: UnaryOp,
    pub operand: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,    // !
    UAdd,   // +
    USub,   // -
    Invert, // ~
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompareExpr<'ast> {
    pub left: Expression<'ast>,
    pub op: &'ast [CompareOp],
    pub comparators: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Eq,    // ==
    NotEq, // !=
    Lt,    // <
    LtE,   // <=
    Gt,    // >
    GtE,   // >=
    Is,    // is
    IsNot, // is not
    In,    // in
    NotIn, // not in
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CallExpr<'ast> {
    pub func: Expression<'ast>,
    pub args: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttributeExpr<'ast> {
    pub value: Expression<'ast>,
    pub attr: Identifier<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubscriptExpr<'ast> {
    pub value: Expression<'ast>,
    pub slice: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListExpr<'ast> {
    pub elts: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TupleExpr<'ast> {
    pub elts: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DictExpr<'ast> {
    pub keys: &'ast [Option<Expression<'ast>>],
    pub values: &'ast [Expression<'ast>],
    pub range: TextRange,
}
