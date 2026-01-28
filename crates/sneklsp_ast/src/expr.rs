use crate::Identifier;
use sneklsp_text::TextRange;

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Name(NameExpr),
    Int(IntExpr),
    Float(FloatExpr),
    String(StringExpr),
    Bool(BoolExpr),
    None(NoneExpr),
    BinOp(BinOpExpr),
    UnaryOp(UnaryOpExpr),
    Compare(CompareExpr),
    Call(CallExpr),
    Attribute(AttributeExpr),
    Subscript(SubscriptExpr),
    List(ListExpr),
    Tuple(TupleExpr),
    Dict(DictExpr),
}

impl Expression {
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

#[derive(Debug, Clone, PartialEq)]
pub struct NameExpr {
    pub id: Identifier,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntExpr {
    pub value: i64,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatExpr {
    pub value: f64,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StringExpr {
    pub value: String,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoolExpr {
    pub value: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoneExpr {
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinOpExpr {
    pub left: Box<Expression>,
    pub op: BinOp,
    pub right: Box<Expression>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct UnaryOpExpr {
    pub op: UnaryOp,
    pub operand: Box<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,    // !
    UAdd,   // +
    USub,   // -
    Invert, // ~
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareExpr {
    pub left: Box<Expression>,
    pub op: Vec<CompareOp>,
    pub comparators: Vec<Expression>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CallExpr {
    pub func: Box<Expression>,
    pub args: Vec<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttributeExpr {
    pub value: Box<Expression>,
    pub attr: Identifier,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubscriptExpr {
    pub value: Box<Expression>,
    pub slice: Box<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListExpr {
    pub elts: Vec<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TupleExpr {
    pub elts: Vec<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DictExpr {
    pub keys: Vec<Option<Expression>>,
    pub values: Vec<Expression>,
    pub range: TextRange,
}
