use compact_str::CompactString;
use sneklsp_ast::*;
use sneklsp_lexer::{Lexer, Token, TokenKind};
use sneklsp_text::{TextRange, TextSize};

use crate::{ParseError, ParseResult};

pub struct Parser<'src> {
    source: &'src str,
    lexer: Lexer<'src>,
    current: Token,
    previous: Token,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        let previous = current.clone();

        Self {
            source,
            lexer,
            current,
            previous,
        }
    }

    pub fn parse_module(&mut self) -> ParseResult<Module> {
        let start = self.current.range.start();
        let mut body = Vec::new();

        while !self.is_at_end() {
            while self.check(TokenKind::Newline) {
                self.advance();
            }
            if self.is_at_end() {
                break;
            }
            body.push(self.parse_statement()?);
        }

        let end = self.previous.range.end();

        Ok(Module {
            body,
            range: TextRange::new(start, end),
        })
    }

    fn parse_statement(&mut self) -> ParseResult<Statement> {
        match self.current.kind {
            TokenKind::Def => self.parse_function_def(),
            TokenKind::Class => self.parse_class_def(),
            TokenKind::If => self.parse_if(),
            TokenKind::For => self.parse_for(),
            TokenKind::While => self.parse_while(),
            TokenKind::Return => self.parse_return(),
            TokenKind::Import => self.parse_import(),
            TokenKind::From => self.parse_import_from(),
            TokenKind::Pass => self.parse_pass(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => self.parse_continue(),
            _ => self.parse_expr_or_assign(),
        }
    }

    fn parse_function_def(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::Def)?;

        let name = self.parse_identifier()?;

        self.expect(TokenKind::LParen)?;
        let params = self.parse_parameters()?;
        self.expect(TokenKind::RParen)?;

        let returns = if self.match_token(TokenKind::Arrow) {
            Some(self.parse_expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Colon)?;
        let body = self.parse_block()?;
        let end = self.previous.range.end();

        Ok(Statement::FunctionDef(FunctionDef {
            name,
            params,
            body,
            returns,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_parameters(&mut self) -> ParseResult<Vec<Parameter>> {
        let mut params = Vec::new();

        if self.check(TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            let start = self.current.range.start();
            let name = self.parse_identifier()?;

            let annotation = if self.match_token(TokenKind::Colon) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            let default = if self.match_token(TokenKind::Eq) {
                Some(self.parse_expression()?)
            } else {
                None
            };

            let end = self.previous.range.end();
            params.push(Parameter {
                name,
                annotation,
                default,
                range: TextRange::new(start, end),
            });

            if !self.match_token(TokenKind::Comma) {
                break;
            }
            if self.check(TokenKind::RParen) {
                break;
            }
        }

        Ok(params)
    }

    fn parse_class_def(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::Class)?;

        let name = self.parse_identifier()?;
        let bases = if self.match_token(TokenKind::LParen) {
            let bases = self.parse_expression_list()?;
            self.expect(TokenKind::RParen)?;
            bases
        } else {
            vec![]
        };

        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let end = self.previous.range.end();

        Ok(Statement::ClassDef(ClassDef {
            name,
            bases,
            body,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_if(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::If)?;

        let test = self.parse_expression()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let orelse = if self.match_token(TokenKind::Elif) {
            vec![self.parse_elif(start)?]
        } else if self.match_token(TokenKind::Else) {
            self.expect(TokenKind::Colon)?;
            self.parse_block()?
        } else {
            vec![]
        };

        let end = self.previous.range.end();

        Ok(Statement::If(IfStmt {
            test,
            body,
            orelse,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_elif(&mut self, start: TextSize) -> ParseResult<Statement> {
        let test = self.parse_expression()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let orelse = if self.match_token(TokenKind::Elif) {
            vec![self.parse_elif(start)?]
        } else if self.match_token(TokenKind::Else) {
            self.expect(TokenKind::Colon)?;
            self.parse_block()?
        } else {
            vec![]
        };

        let end = self.previous.range.end();

        Ok(Statement::If(IfStmt {
            test,
            body,
            orelse,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_for(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::For)?;

        let target = self.parse_expression()?;
        self.expect(TokenKind::In)?;
        let iter = self.parse_expression()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let orelse = if self.match_token(TokenKind::Else) {
            self.expect(TokenKind::Colon)?;
            self.parse_block()?
        } else {
            vec![]
        };

        let end = self.previous.range.end();

        Ok(Statement::For(ForStmt {
            target,
            iter,
            body,
            orelse,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_while(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::While)?;

        let test = self.parse_expression()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let orelse = if self.match_token(TokenKind::Else) {
            self.expect(TokenKind::Colon)?;
            self.parse_block()?
        } else {
            vec![]
        };

        let end = self.previous.range.end();

        Ok(Statement::While(WhileStmt {
            test,
            body,
            orelse,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_return(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        // TODO: `return` keyword should not be required
        self.expect(TokenKind::Return)?;

        let value = if self.check(TokenKind::Newline) || self.is_at_end() {
            None
        } else {
            Some(self.parse_expression()?)
        };

        self.expect_newline_or_eof()?;
        let end = self.current.range.end();
        Ok(Statement::Return(ReturnStmt {
            value,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_import(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::Import)?;

        let names = self.parse_alias_list()?;

        self.expect_newline_or_eof()?;
        let end = self.current.range.end();
        Ok(Statement::Import(ImportStmt {
            names,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_import_from(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::From)?;

        let mut level = 0u32;
        while self.match_token(TokenKind::Dot) {
            level += 1;
        }

        let module = if self.check(TokenKind::Name) {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        self.expect(TokenKind::Import)?;
        let names = self.parse_alias_list()?;

        self.expect_newline_or_eof()?;
        let end = self.current.range.end();

        Ok(Statement::ImportFrom(ImportFromStmt {
            module,
            names,
            level,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_alias_list(&mut self) -> ParseResult<Vec<Alias>> {
        let mut aliases = Vec::new();

        loop {
            let start = self.current.range.start();

            let name = self.parse_identifier()?;
            let asname = if self.match_token(TokenKind::As) {
                Some(self.parse_identifier()?)
            } else {
                None
            };

            let end = self.previous.range.end();
            aliases.push(Alias {
                name,
                asname,
                range: TextRange::new(start, end),
            });

            if !self.match_token(TokenKind::Comma) {
                break;
            }
        }

        Ok(aliases)
    }

    fn parse_pass(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::Pass)?;
        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        Ok(Statement::Pass(PassStmt {
            range: TextRange::new(start, end),
        }))
    }

    fn parse_break(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::Break)?;
        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        Ok(Statement::Break(BreakStmt {
            range: TextRange::new(start, end),
        }))
    }

    fn parse_continue(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        self.expect(TokenKind::Continue)?;
        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        Ok(Statement::Continue(ContinueStmt {
            range: TextRange::new(start, end),
        }))
    }

    fn parse_expr_or_assign(&mut self) -> ParseResult<Statement> {
        let start = self.current.range.start();
        let expr = self.parse_expression()?;

        // basic assignment
        if self.match_token(TokenKind::Eq) {
            let value = self.parse_expression()?;
            self.expect_newline_or_eof()?;
            let end = self.previous.range.end();

            return Ok(Statement::Assign(AssignStmt {
                targets: vec![expr],
                value,
                range: TextRange::new(start, end),
            }));
        }

        // augmented assignment
        let aug_op = match self.current.kind {
            TokenKind::PlusEq => Some(BinOp::Add),
            TokenKind::MinusEq => Some(BinOp::Sub),
            TokenKind::StarEq => Some(BinOp::Mult),
            TokenKind::SlashEq => Some(BinOp::Div),
            TokenKind::PercentEq => Some(BinOp::Mod),
            // TODO: DoubleSlashEq, LogicalOpEq
            _ => None,
        };

        if let Some(op) = aug_op {
            self.advance();
            let value = self.parse_expression()?;
            self.expect_newline_or_eof()?;
            let end = self.previous.range.end();

            return Ok(Statement::AugAssign(AugAssignStmt {
                target: expr,
                op,
                value,
                range: TextRange::new(start, end),
            }));
        }

        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        Ok(Statement::Expr(ExprStmt {
            value: expr,
            range: TextRange::new(start, end),
        }))
    }

    fn parse_block(&mut self) -> ParseResult<Vec<Statement>> {
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;

        let mut statements = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.is_at_end() {
            while self.check(TokenKind::Newline) {
                self.advance();
            }

            if self.check(TokenKind::Dedent) || self.is_at_end() {
                break;
            }

            statements.push(self.parse_statement()?);
        }

        if self.check(TokenKind::Dedent) {
            self.advance();
        }

        Ok(statements)
    }

    // expression parsing with precedence climbing
    fn parse_expression(&mut self) -> ParseResult<Expression> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_and_expr()?;

        while self.match_token(TokenKind::Or) {
            let right = self.parse_and_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op: BinOp::BitOr,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_and_expr(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_not_expr()?;

        while self.match_token(TokenKind::And) {
            let right = self.parse_not_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op: BinOp::BitAnd,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_not_expr(&mut self) -> ParseResult<Expression> {
        if self.match_token(TokenKind::Not) {
            let start = self.previous.range.start();
            let operand = self.parse_not_expr()?;
            let end = operand.range().end();

            return Ok(Expression::UnaryOp(UnaryOpExpr {
                op: UnaryOp::Not,
                operand: Box::new(operand),
                range: TextRange::new(start, end),
            }));
        }

        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> ParseResult<Expression> {
        let left = self.parse_bitor_expr()?;

        if self.check(TokenKind::Is) {
            self.advance();
            let op = if self.match_token(TokenKind::Not) {
                CompareOp::IsNot
            } else {
                CompareOp::Is
            };
            let right = self.parse_bitor_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            return Ok(Expression::Compare(CompareExpr {
                left: Box::new(left),
                op: vec![op],
                comparators: vec![right],
                range,
            }));
        }

        if self.check(TokenKind::Not) {
            self.advance();
            if self.match_token(TokenKind::In) {
                let right = self.parse_bitor_expr()?;
                let range = TextRange::new(left.range().start(), right.range().end());
                return Ok(Expression::Compare(CompareExpr {
                    left: Box::new(left),
                    op: vec![CompareOp::NotIn],
                    comparators: vec![right],
                    range,
                }));
            }
            // TODO: backtrack if we find `not` but not `not in`
            return Err(ParseError::UnexpectedToken {
                offset: self.current.range.start(),
                expected: "in".to_string(),
                found: format!("{:?}", self.current.kind),
            });
        }

        let op = match self.current.kind {
            TokenKind::EqEq => CompareOp::Eq,
            TokenKind::NotEq => CompareOp::NotEq,
            TokenKind::Lt => CompareOp::Lt,
            TokenKind::LtEq => CompareOp::LtE,
            TokenKind::Gt => CompareOp::Gt,
            TokenKind::GtEq => CompareOp::GtE,
            TokenKind::In => CompareOp::In,
            _ => return Ok(left),
        };

        self.advance();

        let right = self.parse_bitor_expr()?;
        let range = TextRange::new(left.range().start(), right.range().end());

        Ok(Expression::Compare(CompareExpr {
            left: Box::new(left),
            op: vec![op],
            comparators: vec![right],
            range,
        }))
    }

    fn parse_bitor_expr(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_bitxor_expr()?;

        while self.match_token(TokenKind::Pipe) {
            let right = self.parse_bitxor_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op: BinOp::BitOr,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_bitxor_expr(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_bitand_expr()?;

        while self.match_token(TokenKind::Caret) {
            let right = self.parse_bitand_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op: BinOp::BitXor,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_bitand_expr(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_shift_expr()?;

        while self.match_token(TokenKind::Amp) {
            let right = self.parse_shift_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op: BinOp::BitAnd,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_shift_expr(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_additive()?;

        loop {
            let op = match self.current.kind {
                TokenKind::LtLt => BinOp::LShift,
                TokenKind::GtGt => BinOp::RShift,
                _ => break,
            };

            self.advance();
            let right = self.parse_additive()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_multiplicative()?;

        loop {
            let op = match self.current.kind {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };

            self.advance();
            let right = self.parse_multiplicative()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_unary()?;

        loop {
            let op = match self.current.kind {
                TokenKind::Star => BinOp::Mult,
                TokenKind::Slash => BinOp::Div,
                TokenKind::DoubleSlash => BinOp::FloorDiv,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };

            self.advance();
            let right = self.parse_unary()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op,
                right: Box::new(right),
                range,
            });
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expression> {
        let op = match self.current.kind {
            TokenKind::Minus => UnaryOp::USub,
            TokenKind::Plus => UnaryOp::UAdd,
            TokenKind::Tilde => UnaryOp::Invert,
            _ => return self.parse_power(),
        };

        let start = self.current.range.start();
        self.advance();

        let operand = self.parse_unary()?;
        let end = operand.range().end();

        Ok(Expression::UnaryOp(UnaryOpExpr {
            op,
            operand: Box::new(operand),
            range: TextRange::new(start, end),
        }))
    }

    fn parse_power(&mut self) -> ParseResult<Expression> {
        let left = self.parse_call()?;

        if self.match_token(TokenKind::DoubleStar) {
            let right = self.parse_unary()?;
            let range = TextRange::new(left.range().start(), right.range().end());
            return Ok(Expression::BinOp(BinOpExpr {
                left: Box::new(left),
                op: BinOp::Pow,
                right: Box::new(right),
                range,
            }));
        }

        Ok(left)
    }

    fn parse_call(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(TokenKind::LParen) {
                let args = self.parse_expression_list()?;
                self.expect(TokenKind::RParen)?;

                let range = TextRange::new(expr.range().start(), self.previous.range.end());
                expr = Expression::Call(CallExpr {
                    func: Box::new(expr),
                    args,
                    range,
                });
            } else if self.match_token(TokenKind::Dot) {
                let attr = self.parse_identifier()?;
                let range = TextRange::new(expr.range().start(), self.previous.range.end());
                expr = Expression::Attribute(AttributeExpr {
                    value: Box::new(expr),
                    attr,
                    range,
                });
            } else if self.match_token(TokenKind::LBracket) {
                let slice = self.parse_expression()?;
                self.expect(TokenKind::RBracket)?;

                let range = TextRange::new(expr.range().start(), self.previous.range.end());
                expr = Expression::Subscript(SubscriptExpr {
                    value: Box::new(expr),
                    slice: Box::new(slice),
                    range,
                });
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expression> {
        let token = &self.current;
        let range = token.range;

        match token.kind {
            TokenKind::Int => {
                let text = self.token_text();
                let value = text.parse().unwrap_or(0);
                self.advance();
                Ok(Expression::Int(IntExpr { value, range }))
            }

            TokenKind::Float => {
                let text = self.token_text();
                let value = text.parse().unwrap_or(0.0);
                self.advance();
                Ok(Expression::Float(FloatExpr { value, range }))
            }

            TokenKind::String => {
                let text = self.token_text();
                let value = text[1..text.len() - 1].to_string();
                self.advance();
                Ok(Expression::String(StringExpr { value, range }))
            }

            TokenKind::True => {
                self.advance();
                Ok(Expression::Bool(BoolExpr { value: true, range }))
            }

            TokenKind::False => {
                self.advance();
                Ok(Expression::Bool(BoolExpr {
                    value: false,
                    range,
                }))
            }

            TokenKind::None => {
                self.advance();
                Ok(Expression::None(NoneExpr { range }))
            }

            TokenKind::Name => {
                let id = CompactString::new(self.token_text());
                self.advance();
                Ok(Expression::Name(NameExpr { id, range }))
            }

            TokenKind::LParen => {
                self.advance();

                if self.check(TokenKind::RParen) {
                    self.advance(); // empty tuple
                    let range = TextRange::new(range.start(), self.previous.range.end());
                    return Ok(Expression::Tuple(TupleExpr {
                        elts: vec![],
                        range,
                    }));
                }

                let expr = self.parse_expression()?;

                if self.match_token(TokenKind::Comma) {
                    let mut elts = vec![expr];
                    if !self.check(TokenKind::RParen) {
                        elts.extend(self.parse_expression_list()?);
                    }
                    self.expect(TokenKind::RParen)?;
                    let range = TextRange::new(range.start(), self.previous.range.end());
                    return Ok(Expression::Tuple(TupleExpr { elts, range }));
                }

                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }

            TokenKind::LBracket => {
                let start = self.current.range.start();
                self.advance();

                let elts = if self.check(TokenKind::RBracket) {
                    vec![]
                } else {
                    self.parse_expression_list()?
                };

                self.expect(TokenKind::RBracket)?;
                let range = TextRange::new(start, self.previous.range.end());

                Ok(Expression::List(ListExpr { elts, range }))
            }

            TokenKind::LBrace => {
                let start = self.current.range.start();
                self.advance();

                if self.check(TokenKind::RBrace) {
                    self.advance();
                    let range = TextRange::new(start, self.previous.range.end());
                    return Ok(Expression::Dict(DictExpr {
                        keys: vec![],
                        values: vec![],
                        range,
                    }));
                }

                let mut keys = Vec::new();
                let mut values = Vec::new();

                loop {
                    let key = self.parse_expression()?;
                    self.expect(TokenKind::Colon)?;
                    let value = self.parse_expression()?;

                    keys.push(Some(key));
                    values.push(value);

                    if !self.match_token(TokenKind::Comma) {
                        break;
                    }
                    if self.check(TokenKind::RBrace) {
                        break;
                    }
                }

                self.expect(TokenKind::RBrace)?;
                let range = TextRange::new(start, self.previous.range.end());

                Ok(Expression::Dict(DictExpr {
                    keys,
                    values,
                    range,
                }))
            }

            _ => Err(ParseError::UnexpectedToken {
                offset: range.start(),
                expected: "expression".to_string(),
                found: format!("{:?}", token.kind),
            }),
        }
    }

    fn parse_expression_list(&mut self) -> ParseResult<Vec<Expression>> {
        let mut exprs = Vec::new();

        if self.check(TokenKind::RParen)
            || self.check(TokenKind::RBracket)
            || self.check(TokenKind::RBrace)
        {
            return Ok(exprs);
        }

        exprs.push(self.parse_expression()?);

        while self.match_token(TokenKind::Comma) {
            if self.check(TokenKind::RParen)
                || self.check(TokenKind::RBracket)
                || self.check(TokenKind::RBrace)
            {
                break;
            }
            exprs.push(self.parse_expression()?);
        }

        Ok(exprs)
    }

    fn parse_identifier(&mut self) -> ParseResult<Identifier> {
        if self.check(TokenKind::Name) {
            let id = CompactString::new(self.token_text());
            self.advance();
            Ok(id)
        } else {
            Err(ParseError::UnexpectedToken {
                offset: self.current.range.start(),
                expected: "identifier".to_string(),
                found: format!("{:?}", self.current.kind),
            })
        }
    }

    // utility methods
    fn token_text(&self) -> &str {
        let range = self.current.range;
        &self.source[range.start().to_usize()..range.end().to_usize()]
    }

    fn advance(&mut self) {
        self.previous = self.current.clone();
        self.current = self.lexer.next_token();
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.current.kind == kind
    }

    fn match_token(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: TokenKind) -> ParseResult<()> {
        if self.check(kind) {
            self.advance();
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                offset: self.current.range.start(),
                expected: format!("{kind:?}"),
                found: format!("{:?}", self.current.kind),
            })
        }
    }

    fn expect_newline_or_eof(&mut self) -> ParseResult<()> {
        if self.check(TokenKind::Newline) {
            self.advance();
            Ok(())
        } else if self.check(TokenKind::Eof) || self.check(TokenKind::Dedent) {
            Ok(())
        } else {
            Err(ParseError::UnexpectedToken {
                offset: self.current.range.start(),
                expected: "newline".to_string(),
                found: format!("{:?}", self.current.kind),
            })
        }
    }

    fn is_at_end(&self) -> bool {
        self.current.kind == TokenKind::Eof
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sneklsp_ast::Statement;

    fn parse(source: &str) -> ParseResult<Module> {
        Parser::new(source).parse_module()
    }

    #[test]
    fn binary_precendence() {
        let source = "1 + 2 * 3";
        let module = parse(source).unwrap();
        assert_eq!(module.body.len(), 1);
    }

    #[test]
    fn function_with_params() {
        let source = "def add(a, b):\n    return a + b";
        let module = parse(source).unwrap();
        assert!(matches!(module.body[0], Statement::FunctionDef(_)));
    }

    #[test]
    fn class_def() {
        let source = "class Foo:\n    pass";
        let module = parse(source).unwrap();
        assert!(matches!(module.body[0], Statement::ClassDef(_)));
    }

    #[test]
    fn if_elif_else() {
        let source = "if x:\n    a\nelif y:\n    b\nelse:\n    c";
        let module = parse(source).unwrap();
        assert!(matches!(module.body[0], Statement::If(_)));
    }

    #[test]
    fn list_literal() {
        let source = "[1, 2, 3]";
        let module = parse(source).unwrap();
        assert!(matches!(
            module.body[0],
            Statement::Expr(ExprStmt {
                value: Expression::List(_),
                ..
            })
        ));
    }

    #[test]
    fn dict_literal() {
        let source = "{a: 1, b: 2, c: 3}";
        let module = parse(source).unwrap();
        assert!(matches!(
            module.body[0],
            Statement::Expr(ExprStmt {
                value: Expression::Dict(_),
                ..
            })
        ));
    }
}
