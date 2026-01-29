use crate::{BinOp, Expression, Identifier};
use sneklsp_text::TextRange;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Statement<'ast> {
    FunctionDef(&'ast FunctionDef<'ast>),
    ClassDef(&'ast ClassDef<'ast>),
    Return(&'ast ReturnStmt<'ast>),
    Assign(&'ast AssignStmt<'ast>),
    AugAssign(&'ast AugAssignStmt<'ast>),
    If(&'ast IfStmt<'ast>),
    For(&'ast ForStmt<'ast>),
    While(&'ast WhileStmt<'ast>),
    Import(&'ast ImportStmt<'ast>),
    ImportFrom(&'ast ImportFromStmt<'ast>),
    Expr(&'ast ExprStmt<'ast>),
    Pass(&'ast PassStmt),
    Break(&'ast BreakStmt),
    Continue(&'ast ContinueStmt),
}

impl<'ast> Statement<'ast> {
    pub const fn range(&self) -> TextRange {
        match self {
            Statement::FunctionDef(s) => s.range,
            Statement::ClassDef(s) => s.range,
            Statement::Return(s) => s.range,
            Statement::Assign(s) => s.range,
            Statement::AugAssign(s) => s.range,
            Statement::If(s) => s.range,
            Statement::For(s) => s.range,
            Statement::While(s) => s.range,
            Statement::Import(s) => s.range,
            Statement::ImportFrom(s) => s.range,
            Statement::Expr(s) => s.range,
            Statement::Pass(s) => s.range,
            Statement::Break(s) => s.range,
            Statement::Continue(s) => s.range,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunctionDef<'ast> {
    pub name: Identifier<'ast>,
    pub params: &'ast [Parameter<'ast>],
    pub body: &'ast [Statement<'ast>],
    pub returns: Option<Expression<'ast>>,
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
    pub body: &'ast [Statement<'ast>],
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
pub struct ExprStmt<'ast> {
    pub value: Expression<'ast>,
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
