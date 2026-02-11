use crate::{BinOp, Expression, Identifier};
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Statement<'ast> {
    FunctionDef(&'ast FunctionDef<'ast>),
    ClassDef(&'ast ClassDef<'ast>),
    Return(&'ast ReturnStmt<'ast>),
    Assign(&'ast AssignStmt<'ast>),
    AugAssign(&'ast AugAssignStmt<'ast>),
    AnnAssign(&'ast AnnAssignStmt<'ast>),
    If(&'ast IfStmt<'ast>),
    For(&'ast ForStmt<'ast>),
    While(&'ast WhileStmt<'ast>),
    With(&'ast WithStmt<'ast>),
    Try(&'ast TryStmt<'ast>),
    Raise(&'ast RaiseStmt<'ast>),
    Assert(&'ast AssertStmt<'ast>),
    Import(&'ast ImportStmt<'ast>),
    ImportFrom(&'ast ImportFromStmt<'ast>),
    Global(&'ast GlobalStmt<'ast>),
    Nonlocal(&'ast NonlocalStmt<'ast>),
    Expr(&'ast ExprStmt<'ast>),
    Delete(&'ast DeleteStmt<'ast>),
    Pass(&'ast PassStmt),
    Break(&'ast BreakStmt),
    Continue(&'ast ContinueStmt),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunctionDef<'ast> {
    pub name: Identifier<'ast>,
    pub params: &'ast Parameters<'ast>,
    pub body: &'ast [Statement<'ast>],
    pub decorators: &'ast [Expression<'ast>],
    pub returns: Option<Expression<'ast>>,
    pub is_async: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parameters<'ast> {
    pub posonlyargs: &'ast [Parameter<'ast>],
    pub args: &'ast [Parameter<'ast>],
    pub vararg: Option<&'ast Parameter<'ast>>,
    pub kwonlyargs: &'ast [Parameter<'ast>],
    pub kw_defaults: &'ast [Option<Expression<'ast>>],
    pub kwarg: Option<&'ast Parameter<'ast>>,
    pub defaults: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Parameter<'ast> {
    pub name: Identifier<'ast>,
    pub annotation: Option<Expression<'ast>>,
    pub default: Option<Expression<'ast>>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassDef<'ast> {
    pub name: Identifier<'ast>,
    pub bases: &'ast [Expression<'ast>],
    pub keywords: &'ast [Keyword<'ast>],
    pub body: &'ast [Statement<'ast>],
    pub decorators: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyword<'ast> {
    pub arg: Option<Identifier<'ast>>,
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReturnStmt<'ast> {
    pub value: Option<Expression<'ast>>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssignStmt<'ast> {
    pub targets: &'ast [Expression<'ast>],
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AugAssignStmt<'ast> {
    pub target: Expression<'ast>,
    pub op: BinOp,
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnnAssignStmt<'ast> {
    pub target: Expression<'ast>,
    pub annotation: Expression<'ast>,
    pub value: Option<Expression<'ast>>,
    pub simple: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IfStmt<'ast> {
    pub test: Expression<'ast>,
    pub body: &'ast [Statement<'ast>],
    pub orelse: &'ast [Statement<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForStmt<'ast> {
    pub target: Expression<'ast>,
    pub iter: Expression<'ast>,
    pub body: &'ast [Statement<'ast>],
    pub orelse: &'ast [Statement<'ast>],
    pub is_async: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhileStmt<'ast> {
    pub test: Expression<'ast>,
    pub body: &'ast [Statement<'ast>],
    pub orelse: &'ast [Statement<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WithStmt<'ast> {
    pub items: &'ast [WithItem<'ast>],
    pub body: &'ast [Statement<'ast>],
    pub is_async: bool,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WithItem<'ast> {
    pub context_expr: Expression<'ast>,
    pub optional_vars: Option<Expression<'ast>>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TryStmt<'ast> {
    pub body: &'ast [Statement<'ast>],
    pub handlers: &'ast [ExceptHandler<'ast>],
    pub orelse: &'ast [Statement<'ast>],
    pub finalbody: &'ast [Statement<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExceptHandler<'ast> {
    pub typ: Option<Expression<'ast>>,
    pub name: Option<Identifier<'ast>>,
    pub body: &'ast [Statement<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RaiseStmt<'ast> {
    pub exc: Option<Expression<'ast>>,
    pub cause: Option<Expression<'ast>>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AssertStmt<'ast> {
    pub test: Expression<'ast>,
    pub msg: Option<Expression<'ast>>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportStmt<'ast> {
    pub names: &'ast [Alias<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportFromStmt<'ast> {
    pub module: Option<Identifier<'ast>>,
    pub names: &'ast [Alias<'ast>],
    pub level: u32,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alias<'ast> {
    pub name: Identifier<'ast>,
    pub asname: Option<Identifier<'ast>>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalStmt<'ast> {
    pub names: &'ast [Identifier<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonlocalStmt<'ast> {
    pub names: &'ast [Identifier<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExprStmt<'ast> {
    pub value: Expression<'ast>,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeleteStmt<'ast> {
    pub targets: &'ast [Expression<'ast>],
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PassStmt {
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreakStmt {
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContinueStmt {
    pub range: TextRange,
}
