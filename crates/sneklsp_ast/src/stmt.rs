use crate::{Expression, Identifier};
use sneklsp_text::TextRange;

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    FunctionDef(FunctionDef),
    ClassDef(ClassDef),
    Return(ReturnStmt),
    Assign(AssignStmt),
    AugAssign(AugAssignStmt),
    If(IfStmt),
    For(ForStmt),
    While(WhileStmt),
    Import(ImportStmt),
    ImportFrom(ImportFromStmt),
    Expr(ExprStmt),
    Pass(PassStmt),
    Break(BreakStmt),
    Continue(ContinueStmt),
}

impl Statement {
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

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDef {
    pub name: Identifier,
    pub params: Vec<Parameter>,
    pub body: Vec<Statement>,
    pub returns: Option<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: Identifier,
    pub annotation: Option<Expression>,
    pub default: Option<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassDef {
    pub name: Identifier,
    pub bases: Vec<Expression>,
    pub body: Vec<Statement>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStmt {
    pub value: Option<Expression>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignStmt {
    pub targets: Vec<Expression>,
    pub value: Expression,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AugAssignStmt {
    pub target: Expression,
    pub op: crate::BinOp,
    pub value: Expression,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    pub test: Expression,
    pub body: Vec<Statement>,
    pub orelse: Vec<Statement>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    pub target: Expression,
    pub iter: Expression,
    pub body: Vec<Statement>,
    pub orelse: Vec<Statement>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    pub test: Expression,
    pub body: Vec<Statement>,
    pub orelse: Vec<Statement>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportStmt {
    pub names: Vec<Alias>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportFromStmt {
    pub module: Option<Identifier>,
    pub names: Vec<Alias>,
    pub level: u32,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alias {
    pub name: Identifier,
    pub asname: Option<Identifier>,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExprStmt {
    pub value: Expression,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PassStmt {
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BreakStmt {
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContinueStmt {
    pub range: TextRange,
}
