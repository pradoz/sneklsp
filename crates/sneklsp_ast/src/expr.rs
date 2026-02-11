use crate::Identifier;
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Expression<'ast> {
    Name(&'ast NameExpr<'ast>),
    Int(&'ast IntExpr),
    Float(&'ast FloatExpr),
    String(&'ast StringExpr<'ast>),
    FString(&'ast FStringExpr<'ast>),
    Bytes(&'ast BytesExpr<'ast>),
    Bool(&'ast BoolExpr),
    None(&'ast NoneExpr),
    Ellipsis(&'ast EllipsisExpr),
    BinOp(&'ast BinOpExpr<'ast>),
    UnaryOp(&'ast UnaryOpExpr<'ast>),
    BoolOp(&'ast BoolOpExpr<'ast>),
    Compare(&'ast CompareExpr<'ast>),
    Call(&'ast CallExpr<'ast>),
    Attribute(&'ast AttributeExpr<'ast>),
    Subscript(&'ast SubscriptExpr<'ast>),
    List(&'ast ListExpr<'ast>),
    Tuple(&'ast TupleExpr<'ast>),
    Dict(&'ast DictExpr<'ast>),
    Set(&'ast SetExpr<'ast>),
    Lambda(&'ast LambdaExpr<'ast>),
    IfExp(&'ast IfExpr<'ast>),
    ListComp(&'ast ListCompExpr<'ast>),
    SetComp(&'ast SetCompExpr<'ast>),
    DictComp(&'ast DictCompExpr<'ast>),
    GeneratorExp(&'ast GeneratorExpr<'ast>),
    Yield(&'ast YieldExpr<'ast>),
    YieldFrom(&'ast YieldFromExpr<'ast>),
    Await(&'ast AwaitExpr<'ast>),
    Starred(&'ast StarredExpr<'ast>),
    Named(&'ast NamedExpr<'ast>),
    Slice(&'ast SliceExpr<'ast>),
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
pub struct FStringExpr<'ast> {
    pub values: &'ast [FStringPart<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FStringPart<'ast> {
    Literal(&'ast str),
    FormattedValue(&'ast FormattedValue<'ast>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormattedValue<'ast> {
    pub value: Expression<'ast>,
    pub conversion: Option<char>,
    pub format_spec: Option<&'ast [FStringPart<'ast>]>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BytesExpr<'ast> {
    pub value: &'ast [u8],
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
pub struct EllipsisExpr {
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
pub struct BoolOpExpr<'ast> {
    pub op: BoolOp,
    pub values: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    And,
    Or,
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
    pub keywords: &'ast [crate::Keyword<'ast>],
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetExpr<'ast> {
    pub elts: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LambdaExpr<'ast> {
    pub params: &'ast crate::Parameters<'ast>,
    pub body: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfExpr<'ast> {
    pub test: Expression<'ast>,
    pub body: Expression<'ast>,
    pub orelse: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Comprehension<'ast> {
    pub target: Expression<'ast>,
    pub iter: Expression<'ast>,
    pub ifs: &'ast [Expression<'ast>],
    pub is_async: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListCompExpr<'ast> {
    pub elt: Expression<'ast>,
    pub generators: &'ast [Comprehension<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SetCompExpr<'ast> {
    pub elt: Expression<'ast>,
    pub generators: &'ast [Comprehension<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DictCompExpr<'ast> {
    pub key: Expression<'ast>,
    pub value: Expression<'ast>,
    pub generators: &'ast [Comprehension<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeneratorExpr<'ast> {
    pub elt: Expression<'ast>,
    pub generators: &'ast [Comprehension<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YieldExpr<'ast> {
    pub value: Option<Expression<'ast>>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YieldFromExpr<'ast> {
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AwaitExpr<'ast> {
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StarredExpr<'ast> {
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NamedExpr<'ast> {
    pub target: Expression<'ast>,
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliceExpr<'ast> {
    pub lower: Option<Expression<'ast>>,
    pub upper: Option<Expression<'ast>>,
    pub step: Option<Expression<'ast>>,
    pub range: TextRange,
}
