use sneklsp_ast::*;
use sneklsp_lexer::{Lexer, Token, TokenKind};
use sneklsp_text::{TextRange, TextSize};

use crate::{ParseError, ParseResult};

pub struct Parser<'src, 'ast> {
    source: &'src str,
    arena: &'ast AstArena,
    lexer: Lexer<'src>,
    current: Token,
    previous: Token,
}

impl<'src, 'ast> Parser<'src, 'ast> {
    pub fn new(source: &'src str, arena: &'ast AstArena) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        let previous = current.clone();

        Self {
            source,
            arena,
            lexer,
            current,
            previous,
        }
    }

    pub fn parse_module(&mut self) -> ParseResult<Module<'ast>> {
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
            body: self.arena.alloc_slice(body),
            range: TextRange::new(start, end),
        })
    }

    fn parse_statement(&mut self) -> ParseResult<Statement<'ast>> {
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

    fn parse_function_def(&mut self) -> ParseResult<Statement<'ast>> {
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

        let func = self.arena.alloc(FunctionDef {
            name,
            params,
            body,
            returns,
            range: TextRange::new(start, end),
        });
        Ok(Statement::FunctionDef(func))
    }

    fn parse_parameters(&mut self) -> ParseResult<&'ast [Parameter<'ast>]> {
        let mut params = Vec::new();

        if self.check(TokenKind::RParen) {
            return Ok(self.arena.alloc_slice(params));
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

        Ok(self.arena.alloc_slice(params))
    }

    fn parse_class_def(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        self.expect(TokenKind::Class)?;

        let name = self.parse_identifier()?;
        let bases = if self.match_token(TokenKind::LParen) {
            let bases = self.parse_expression_list()?;
            self.expect(TokenKind::RParen)?;
            bases
        } else {
            self.arena
                .alloc_slice(std::iter::empty::<Expression<'ast>>())
        };

        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let end = self.previous.range.end();

        let class = self.arena.alloc(ClassDef {
            name,
            bases,
            body,
            range: TextRange::new(start, end),
        });

        Ok(Statement::ClassDef(class))
    }

    fn parse_if(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        self.expect(TokenKind::If)?;

        let test = self.parse_expression()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let orelse = if self.match_token(TokenKind::Elif) {
            let elif = self.parse_elif(start)?;
            self.arena.alloc_slice([elif])
        } else if self.match_token(TokenKind::Else) {
            self.expect(TokenKind::Colon)?;
            self.parse_block()?
        } else {
            self.arena
                .alloc_slice(std::iter::empty::<Statement<'ast>>())
        };

        let end = self.previous.range.end();

        let if_stmt = self.arena.alloc(IfStmt {
            test,
            body,
            orelse,
            range: TextRange::new(start, end),
        });

        Ok(Statement::If(if_stmt))
    }

    fn parse_elif(&mut self, start: TextSize) -> ParseResult<Statement<'ast>> {
        let test = self.parse_expression()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let orelse = if self.match_token(TokenKind::Elif) {
            let elif = self.parse_elif(start)?;
            self.arena.alloc_slice([elif])
        } else if self.match_token(TokenKind::Else) {
            self.expect(TokenKind::Colon)?;
            self.parse_block()?
        } else {
            self.arena
                .alloc_slice(std::iter::empty::<Statement<'ast>>())
        };

        let end = self.previous.range.end();

        let if_stmt = self.arena.alloc(IfStmt {
            test,
            body,
            orelse,
            range: TextRange::new(start, end),
        });

        Ok(Statement::If(if_stmt))
    }

    fn parse_for(&mut self) -> ParseResult<Statement<'ast>> {
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
            self.arena
                .alloc_slice(std::iter::empty::<Statement<'ast>>())
        };

        let end = self.previous.range.end();

        let for_stmt = self.arena.alloc(ForStmt {
            target,
            iter,
            body,
            orelse,
            range: TextRange::new(start, end),
        });

        Ok(Statement::For(for_stmt))
    }

    fn parse_while(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        self.expect(TokenKind::While)?;

        let test = self.parse_expression()?;
        self.expect(TokenKind::Colon)?;

        let body = self.parse_block()?;
        let orelse = if self.match_token(TokenKind::Else) {
            self.expect(TokenKind::Colon)?;
            self.parse_block()?
        } else {
            self.arena
                .alloc_slice(std::iter::empty::<Statement<'ast>>())
        };

        let end = self.previous.range.end();

        let while_stmt = self.arena.alloc(WhileStmt {
            test,
            body,
            orelse,
            range: TextRange::new(start, end),
        });

        Ok(Statement::While(while_stmt))
    }

    fn parse_return(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        // TODO: should `return` keyword be optional?
        self.expect(TokenKind::Return)?;

        let value = if self.check(TokenKind::Newline) || self.is_at_end() {
            None
        } else {
            Some(self.parse_expression()?)
        };

        self.expect_newline_or_eof()?;
        let end = self.current.range.end();

        let ret = self.arena.alloc(ReturnStmt {
            value,
            range: TextRange::new(start, end),
        });

        Ok(Statement::Return(ret))
    }

    fn parse_import(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        self.expect(TokenKind::Import)?;

        let names = self.parse_alias_list()?;

        self.expect_newline_or_eof()?;
        let end = self.current.range.end();

        let import = self.arena.alloc(ImportStmt {
            names,
            range: TextRange::new(start, end),
        });

        Ok(Statement::Import(import))
    }

    fn parse_import_from(&mut self) -> ParseResult<Statement<'ast>> {
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

        let import = self.arena.alloc(ImportFromStmt {
            module,
            names,
            level,
            range: TextRange::new(start, end),
        });

        Ok(Statement::ImportFrom(import))
    }

    fn parse_alias_list(&mut self) -> ParseResult<&'ast [Alias<'ast>]> {
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

        Ok(self.arena.alloc_slice(aliases))
    }

    fn parse_pass(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        self.expect(TokenKind::Pass)?;
        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        let pass = self.arena.alloc(PassStmt {
            range: TextRange::new(start, end),
        });

        Ok(Statement::Pass(pass))
    }

    fn parse_break(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        self.expect(TokenKind::Break)?;
        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        let brk = self.arena.alloc(BreakStmt {
            range: TextRange::new(start, end),
        });

        Ok(Statement::Break(brk))
    }

    fn parse_continue(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        self.expect(TokenKind::Continue)?;
        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        let cont = self.arena.alloc(ContinueStmt {
            range: TextRange::new(start, end),
        });

        Ok(Statement::Continue(cont))
    }

    fn parse_expr_or_assign(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.current.range.start();
        let expr = self.parse_expression()?;

        // basic assignment
        if self.match_token(TokenKind::Eq) {
            let value = self.parse_expression()?;
            self.expect_newline_or_eof()?;
            let end = self.previous.range.end();

            let assign = self.arena.alloc(AssignStmt {
                targets: self.arena.alloc_slice([expr]),
                value,
                range: TextRange::new(start, end),
            });

            return Ok(Statement::Assign(assign));
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

            let aug = self.arena.alloc(AugAssignStmt {
                target: expr,
                op,
                value,
                range: TextRange::new(start, end),
            });

            return Ok(Statement::AugAssign(aug));
        }

        self.expect_newline_or_eof()?;
        let end = self.previous.range.end();

        let expr_stmt = self.arena.alloc(ExprStmt {
            value: expr,
            range: TextRange::new(start, end),
        });

        Ok(Statement::Expr(expr_stmt))
    }

    fn parse_block(&mut self) -> ParseResult<&'ast [Statement<'ast>]> {
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

        Ok(self.arena.alloc_slice(statements))
    }

    // expression parsing with precedence climbing
    fn parse_expression(&mut self) -> ParseResult<Expression<'ast>> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> ParseResult<Expression<'ast>> {
        let mut left = self.parse_and_expr()?;

        while self.match_token(TokenKind::Or) {
            let right = self.parse_and_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op: BinOp::BitOr,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_and_expr(&mut self) -> ParseResult<Expression<'ast>> {
        let mut left = self.parse_not_expr()?;

        while self.match_token(TokenKind::And) {
            let right = self.parse_not_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op: BinOp::BitAnd,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_not_expr(&mut self) -> ParseResult<Expression<'ast>> {
        if self.match_token(TokenKind::Not) {
            let start = self.previous.range.start();
            let operand = self.parse_not_expr()?;
            let end = operand.range().end();

            let expr = self.arena.alloc(UnaryOpExpr {
                op: UnaryOp::Not,
                operand: operand,
                range: TextRange::new(start, end),
            });

            return Ok(Expression::UnaryOp(expr));
        }

        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> ParseResult<Expression<'ast>> {
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

            let expr = self.arena.alloc(CompareExpr {
                left: left,
                op: self.arena.alloc_slice([op]),
                comparators: self.arena.alloc_slice([right]),
                range,
            });

            return Ok(Expression::Compare(expr));
        }

        if self.check(TokenKind::Not) {
            self.advance();
            if self.match_token(TokenKind::In) {
                let right = self.parse_bitor_expr()?;
                let range = TextRange::new(left.range().start(), right.range().end());

                let expr = self.arena.alloc(CompareExpr {
                    left: left,
                    op: self.arena.alloc_slice([CompareOp::NotIn]),
                    comparators: self.arena.alloc_slice([right]),
                    range,
                });

                return Ok(Expression::Compare(expr));
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

        let expr = self.arena.alloc(CompareExpr {
            left: left,
            op: self.arena.alloc_slice([op]),
            comparators: self.arena.alloc_slice([right]),
            range,
        });

        Ok(Expression::Compare(expr))
    }

    fn parse_bitor_expr(&mut self) -> ParseResult<Expression<'ast>> {
        let mut left = self.parse_bitxor_expr()?;

        while self.match_token(TokenKind::Pipe) {
            let right = self.parse_bitxor_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op: BinOp::BitOr,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_bitxor_expr(&mut self) -> ParseResult<Expression<'ast>> {
        let mut left = self.parse_bitand_expr()?;

        while self.match_token(TokenKind::Caret) {
            let right = self.parse_bitand_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op: BinOp::BitXor,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_bitand_expr(&mut self) -> ParseResult<Expression<'ast>> {
        let mut left = self.parse_shift_expr()?;

        while self.match_token(TokenKind::Amp) {
            let right = self.parse_shift_expr()?;
            let range = TextRange::new(left.range().start(), right.range().end());

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op: BinOp::BitAnd,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_shift_expr(&mut self) -> ParseResult<Expression<'ast>> {
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

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_additive(&mut self) -> ParseResult<Expression<'ast>> {
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

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> ParseResult<Expression<'ast>> {
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

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op,
                right: right,
                range,
            });

            left = Expression::BinOp(expr);
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expression<'ast>> {
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

        let expr = self.arena.alloc(UnaryOpExpr {
            op,
            operand: operand,
            range: TextRange::new(start, end),
        });

        Ok(Expression::UnaryOp(expr))
    }

    fn parse_power(&mut self) -> ParseResult<Expression<'ast>> {
        let left = self.parse_call()?;

        if self.match_token(TokenKind::DoubleStar) {
            let right = self.parse_unary()?;
            let range = TextRange::new(left.range().start(), right.range().end());

            let expr = self.arena.alloc(BinOpExpr {
                left: left,
                op: BinOp::Pow,
                right: right,
                range,
            });

            return Ok(Expression::BinOp(expr));
        }

        Ok(left)
    }

    fn parse_call(&mut self) -> ParseResult<Expression<'ast>> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(TokenKind::LParen) {
                let args = self.parse_expression_list()?;
                self.expect(TokenKind::RParen)?;

                let range = TextRange::new(expr.range().start(), self.previous.range.end());
                let call = self.arena.alloc(CallExpr {
                    func: expr,
                    args,
                    range,
                });
                expr = Expression::Call(call);
            } else if self.match_token(TokenKind::Dot) {
                let attr = self.parse_identifier()?;
                let range = TextRange::new(expr.range().start(), self.previous.range.end());
                let attribute = self.arena.alloc(AttributeExpr {
                    value: expr,
                    attr,
                    range,
                });
                expr = Expression::Attribute(attribute);
            } else if self.match_token(TokenKind::LBracket) {
                let slice = self.parse_expression()?;
                self.expect(TokenKind::RBracket)?;

                let range = TextRange::new(expr.range().start(), self.previous.range.end());
                let subscript = self.arena.alloc(SubscriptExpr {
                    value: expr,
                    slice: slice,
                    range,
                });
                expr = Expression::Subscript(subscript);
            } else {
                break;
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> ParseResult<Expression<'ast>> {
        let token = &self.current;
        let range = token.range;

        match token.kind {
            TokenKind::Int => {
                let text = self.token_text();
                let value = text.parse().unwrap_or(0);
                self.advance();
                let expr = self.arena.alloc(IntExpr { value, range });
                Ok(Expression::Int(expr))
            }

            TokenKind::Float => {
                let text = self.token_text();
                let value = text.parse().unwrap_or(0.0);
                self.advance();
                let expr = self.arena.alloc(FloatExpr { value, range });
                Ok(Expression::Float(expr))
            }

            TokenKind::String => {
                let text = self.token_text();
                let value = self.arena.alloc_str(&text[1..text.len() - 1]);
                self.advance();
                let expr = self.arena.alloc(StringExpr { value, range });
                Ok(Expression::String(expr))
            }

            TokenKind::True => {
                self.advance();
                let expr = self.arena.alloc(BoolExpr { value: true, range });
                Ok(Expression::Bool(expr))
            }

            TokenKind::False => {
                self.advance();
                let expr = self.arena.alloc(BoolExpr {
                    value: false,
                    range,
                });
                Ok(Expression::Bool(expr))
            }

            TokenKind::None => {
                self.advance();
                let expr = self.arena.alloc(NoneExpr { range });
                Ok(Expression::None(expr))
            }

            TokenKind::Name => {
                let id = self.arena.alloc_str(self.token_text());
                self.advance();
                let expr = self.arena.alloc(NameExpr { id, range });
                Ok(Expression::Name(expr))
            }

            TokenKind::LParen => {
                self.advance();

                if self.check(TokenKind::RParen) {
                    self.advance(); // empty tuple
                    let range = TextRange::new(range.start(), self.previous.range.end());
                    let tuple = self.arena.alloc(TupleExpr {
                        elts: self
                            .arena
                            .alloc_slice(std::iter::empty::<Expression<'ast>>()),
                        range,
                    });
                    return Ok(Expression::Tuple(tuple));
                }

                let expr = self.parse_expression()?;

                if self.match_token(TokenKind::Comma) {
                    let mut elts = vec![expr];
                    if !self.check(TokenKind::RParen) {
                        let rest = self.parse_expression_list_vec()?;
                        elts.extend(rest);
                    }
                    self.expect(TokenKind::RParen)?;
                    let range = TextRange::new(range.start(), self.previous.range.end());
                    let tuple = self.arena.alloc(TupleExpr {
                        elts: self.arena.alloc_slice(elts),
                        range,
                    });
                    return Ok(Expression::Tuple(tuple));
                }

                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }

            TokenKind::LBracket => {
                let start = self.current.range.start();
                self.advance();

                let elts = if self.check(TokenKind::RBracket) {
                    self.arena
                        .alloc_slice(std::iter::empty::<Expression<'ast>>())
                } else {
                    self.parse_expression_list()?
                };

                self.expect(TokenKind::RBracket)?;
                let range = TextRange::new(start, self.previous.range.end());

                let list = self.arena.alloc(ListExpr { elts, range });
                Ok(Expression::List(list))
            }

            TokenKind::LBrace => {
                let start = self.current.range.start();
                self.advance();

                if self.check(TokenKind::RBrace) {
                    self.advance();
                    let range = TextRange::new(start, self.previous.range.end());
                    let dict = self.arena.alloc(DictExpr {
                        keys: self
                            .arena
                            .alloc_slice(std::iter::empty::<Option<Expression<'ast>>>()),
                        values: self
                            .arena
                            .alloc_slice(std::iter::empty::<Expression<'ast>>()),
                        range,
                    });
                    return Ok(Expression::Dict(dict));
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

                let dict = self.arena.alloc(DictExpr {
                    keys: self.arena.alloc_slice(keys),
                    values: self.arena.alloc_slice(values),
                    range,
                });
                Ok(Expression::Dict(dict))
            }

            _ => Err(ParseError::UnexpectedToken {
                offset: range.start(),
                expected: "expression".to_string(),
                found: format!("{:?}", token.kind),
            }),
        }
    }

    fn parse_expression_list(&mut self) -> ParseResult<&'ast [Expression<'ast>]> {
        let exprs = self.parse_expression_list_vec()?;
        Ok(self.arena.alloc_slice(exprs))
    }

    fn parse_expression_list_vec(&mut self) -> ParseResult<Vec<Expression<'ast>>> {
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

    fn parse_identifier(&mut self) -> ParseResult<Identifier<'ast>> {
        if self.check(TokenKind::Name) {
            let id = self.arena.alloc_str(self.token_text());
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

    #[test]
    fn binary_precendence() {
        let arena = AstArena::new();
        let source = "1 + 2 * 3";
        let module = Parser::new(source, &arena).parse_module().unwrap();
        assert_eq!(module.body.len(), 1);
    }

    #[test]
    fn function_with_params() {
        let arena = AstArena::new();
        let source = "def add(a, b):\n    return a + b";
        let module = Parser::new(source, &arena).parse_module().unwrap();
        assert!(matches!(module.body[0], Statement::FunctionDef(_)));
    }

    #[test]
    fn class_def() {
        let arena = AstArena::new();
        let source = "class Foo:\n    pass";
        let module = Parser::new(source, &arena).parse_module().unwrap();
        assert!(matches!(module.body[0], Statement::ClassDef(_)));
    }

    #[test]
    fn if_elif_else() {
        let arena = AstArena::new();
        let source = "if x:\n    a\nelif y:\n    b\nelse:\n    c";
        let module = Parser::new(source, &arena).parse_module().unwrap();
        assert!(matches!(module.body[0], Statement::If(_)));
    }

    #[test]
    fn list_literal() {
        let arena = AstArena::new();
        let source = "[1, 2, 3]";
        let module = Parser::new(source, &arena).parse_module().unwrap();
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
        let arena = AstArena::new();
        let source = "{a: 1, b: 2, c: 3}";
        let module = Parser::new(source, &arena).parse_module().unwrap();
        assert!(matches!(
            module.body[0],
            Statement::Expr(ExprStmt {
                value: Expression::Dict(_),
                ..
            })
        ));
    }
}
