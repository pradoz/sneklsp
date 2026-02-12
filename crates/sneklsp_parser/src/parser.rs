use crate::{ParseError, ParseResult};

use sneklsp_ast::*;
use sneklsp_lexer::{Lexer, Token, TokenKind};
use sneklsp_text::{TextRange, TextSize};

// precedence levels for binary operators
const PREC_BITOR: u8 = 4;
const PREC_BITXOR: u8 = 5;
const PREC_BITAND: u8 = 6;
const PREC_SHIFT: u8 = 7;
const PREC_ADD: u8 = 8;
const PREC_MUL: u8 = 9;
const PREC_POW: u8 = 11;

#[inline]
const fn binop_prec(kind: TokenKind) -> Option<(BinOp, u8, bool)> {
    match kind {
        TokenKind::Pipe => Some((BinOp::BitOr, PREC_BITOR, false)),
        TokenKind::Caret => Some((BinOp::BitXor, PREC_BITXOR, false)),
        TokenKind::Amp => Some((BinOp::BitAnd, PREC_BITAND, false)),
        TokenKind::LtLt => Some((BinOp::LShift, PREC_SHIFT, false)),
        TokenKind::GtGt => Some((BinOp::RShift, PREC_SHIFT, false)),
        TokenKind::Plus => Some((BinOp::Add, PREC_ADD, false)),
        TokenKind::Minus => Some((BinOp::Sub, PREC_ADD, false)),
        TokenKind::Star => Some((BinOp::Mult, PREC_MUL, false)),
        TokenKind::Slash => Some((BinOp::Div, PREC_MUL, false)),
        TokenKind::DoubleSlash => Some((BinOp::FloorDiv, PREC_MUL, false)),
        TokenKind::Percent => Some((BinOp::Mod, PREC_MUL, false)),
        TokenKind::DoubleStar => Some((BinOp::Pow, PREC_POW, true)),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    Strict,
    Recovering,
}

enum RecoveredStatement<'ast> {
    Ok(Statement<'ast>),
    Error(ParseError),
    Eof,
}

pub struct Parser<'src, 'ast> {
    source: &'src str,
    arena: &'ast AstArena,
    lexer: Lexer<'src>,
    current: Token,
    previous: Token,
    errors: Vec<ParseError>,
    mode: ParseMode,
}

impl<'src, 'ast> Parser<'src, 'ast> {
    pub fn new(source: &'src str, arena: &'ast AstArena) -> Self {
        let mut lexer = Lexer::new(source);
        let mut current = lexer.next_token();

        while current.kind == TokenKind::Comment {
            current = lexer.next_token();
        }

        Self {
            source,
            arena,
            lexer,
            previous: current,
            current,
            errors: Vec::new(),
            mode: ParseMode::Strict,
        }
    }

    pub fn with_recovery(mut self) -> Self {
        self.mode = ParseMode::Recovering;
        self
    }

    pub fn take_errors(&mut self) -> Vec<ParseError> {
        std::mem::take(&mut self.errors)
    }

    #[inline]
    fn empty_slice<T>(&self) -> &'ast [T] {
        self.arena.alloc_slice(std::iter::empty::<T>())
    }

    #[inline]
    fn range(&self, start: TextSize) -> TextRange {
        TextRange::new(start, self.previous.range.end())
    }

    #[inline]
    fn start(&self) -> TextSize {
        self.current.range.start()
    }

    #[inline]
    fn token_text(&self) -> &str {
        &self.source[self.current.range.start().to_usize()..self.current.range.end().to_usize()]
    }

    #[inline]
    fn advance(&mut self) {
        self.previous = std::mem::replace(&mut self.current, self.lexer.next_token());

        while self.current.kind == TokenKind::Comment {
            self.current = self.lexer.next_token();
        }
    }

    #[inline]
    fn check(&self, kind: TokenKind) -> bool {
        self.current.kind == kind
    }

    #[inline]
    fn at_end(&self) -> bool {
        self.current.kind == TokenKind::Eof
    }

    #[inline]
    fn consume(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    #[inline]
    fn expect(&mut self, kind: TokenKind) -> ParseResult<()> {
        if self.consume(kind) {
            Ok(())
        } else {
            Err(self.err(&format!("{kind:?}")))
        }
    }

    #[inline]
    fn expect_newline(&mut self) -> ParseResult<()> {
        if self.consume(TokenKind::Newline)
            || self.check(TokenKind::Eof)
            || self.check(TokenKind::Dedent)
        {
            Ok(())
        } else if self.mode == ParseMode::Recovering && self.is_stmt_start() {
            Ok(())
        } else {
            Err(self.err("newline"))
        }
    }

    fn expect_recover(&mut self, kind: TokenKind) {
        if !self.consume(kind) && self.mode == ParseMode::Recovering {
            self.errors.push(self.err(&format!("{kind:?}")));
        }
    }

    // recover from unclosed brackets by assuming close
    fn expect_close(&mut self, kind: TokenKind) -> ParseResult<()> {
        if self.consume(kind) {
            return Ok(());
        }

        if self.mode == ParseMode::Recovering {
            self.errors.push(self.err(&format!("{kind:?}")));
            if matches!(
                kind,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace
            ) {
                self.lexer.decrement_bracket_depth();
            }
            return Ok(());
        }

        Err(self.err(&format!("{kind:?}")))
    }

    fn missing_expr(&self) -> Expression<'ast> {
        let pos = self.current.range.start();
        let range = TextRange::new(pos, pos);
        Expression::Name(self.arena.alloc(NameExpr {
            id: self.arena.alloc_str(""),
            range,
        }))
    }

    // try to parse an expression. fall back to missing placeholder in recovery mode
    fn parse_expr_or_missing(&mut self) -> ParseResult<Expression<'ast>> {
        match self.parse_expr() {
            Ok(expr) => Ok(expr),
            Err(e) => {
                if self.mode == ParseMode::Recovering {
                    self.errors.push(e);
                    Ok(self.missing_expr())
                } else {
                    Err(e)
                }
            }
        }
    }

    #[inline]
    fn err(&self, expected: &str) -> ParseError {
        ParseError::UnexpectedToken {
            range: self.current.range,
            expected: expected.to_string(),
            found: format!("{:?}", self.current.kind),
        }
    }

    fn skip_newline(&mut self) {
        while self.check(TokenKind::Newline) {
            self.advance();
        }
    }

    fn empty_params(&self) -> &'ast Parameters<'ast> {
        self.arena.alloc(Parameters {
            posonlyargs: self.empty_slice(),
            args: self.empty_slice(),
            vararg: None,
            kwonlyargs: self.empty_slice(),
            kw_defaults: self.empty_slice(),
            kwarg: None,
            defaults: self.empty_slice(),
            range: TextRange::default(),
        })
    }

    fn comma_list<T, F>(&mut self, close: TokenKind, mut parse: F) -> ParseResult<Vec<T>>
    where
        F: FnMut(&mut Self) -> ParseResult<T>,
    {
        let mut items = Vec::new();
        if !self.check(close) {
            items.push(parse(self)?);
            while self.consume(TokenKind::Comma) && !self.check(close) {
                items.push(parse(self)?);
            }
        }
        Ok(items)
    }

    // optional parse: if token matches, parse and return Some
    #[inline]
    fn opt<T, F>(&mut self, kind: TokenKind, parse: F) -> ParseResult<Option<T>>
    where
        F: FnOnce(&mut Self) -> ParseResult<T>,
    {
        if self.consume(kind) {
            Ok(Some(parse(self)?))
        } else {
            Ok(None)
        }
    }

    pub fn parse_module(&mut self) -> ParseResult<Module<'ast>> {
        let start = self.start();
        let mut body = Vec::with_capacity(32);

        while !self.at_end() {
            self.skip_newline();
            if self.at_end() {
                break;
            }

            let before = self.current.range.start();
            match self.try_parse_stmt() {
                RecoveredStatement::Ok(stmt) => body.push(stmt),
                RecoveredStatement::Error(e) => {
                    if self.mode == ParseMode::Strict {
                        return Err(e);
                    }
                    self.errors.push(e);
                    self.synchronize();
                }
                RecoveredStatement::Eof => break,
            }
            if self.current.range.start() == before && !self.at_end() {
                self.advance();
            }
        }

        Ok(Module {
            body: self.arena.alloc_slice(body),
            range: self.range(start),
        })
    }

    pub fn parse_module_recovering(&mut self) -> Module<'ast> {
        self.mode = ParseMode::Recovering;
        match self.parse_module() {
            Ok(module) => module,
            Err(e) => {
                self.errors.push(e);
                Module {
                    body: self.empty_slice(),
                    range: TextRange::new(
                        TextSize::new(0),
                        TextSize::new(self.source.len() as u32),
                    ),
                }
            }
        }
    }

    fn synchronize(&mut self) {
        let before = self.current.range.start();

        while !self.at_end() {
            // if we just passed a newline and are at a statement start, recover here
            if self.previous.kind == TokenKind::Newline {
                if self.is_stmt_start() {
                    return;
                }
                // also recover at dedent
                if self.check(TokenKind::Dedent) {
                    return;
                }
            }

            // recover at block-starting keywords
            if self.is_sync_point() {
                return;
            }

            // skip past dedents to exit broken blocks
            if self.check(TokenKind::Dedent) {
                self.advance();
                if self.is_stmt_start() || self.at_end() {
                    return;
                }
                continue;
            }

            self.advance();
        }

        // force advance if we didn't move
        if !self.at_end() && self.current.range.start() == before {
            self.advance();
        }
    }

    fn is_sync_point(&self) -> bool {
        matches!(
            self.current.kind,
            TokenKind::Def
                | TokenKind::Async
                | TokenKind::Class
                | TokenKind::If
                | TokenKind::For
                | TokenKind::While
                | TokenKind::Try
                | TokenKind::With
                | TokenKind::Return
                | TokenKind::Raise
                | TokenKind::Assert
                | TokenKind::Import
                | TokenKind::From
                | TokenKind::Global
                | TokenKind::Nonlocal
                | TokenKind::Del
                | TokenKind::Pass
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::At
        )
    }

    fn is_stmt_start(&self) -> bool {
        self.is_sync_point()
            || matches!(
                self.current.kind,
                TokenKind::Name
                    | TokenKind::Int
                    | TokenKind::Float
                    | TokenKind::String
                    | TokenKind::FString
                    | TokenKind::LParen
                    | TokenKind::LBracket
                    | TokenKind::LBrace
                    | TokenKind::Star
                    | TokenKind::Lambda
                    | TokenKind::Yield
            )
    }

    fn try_parse_stmt(&mut self) -> RecoveredStatement<'ast> {
        if self.at_end() {
            return RecoveredStatement::Eof;
        }

        match self.parse_stmt() {
            Ok(stmt) => RecoveredStatement::Ok(stmt),
            Err(e) => RecoveredStatement::Error(e),
        }
    }

    fn parse_stmt(&mut self) -> ParseResult<Statement<'ast>> {
        match self.current.kind {
            TokenKind::At => self.parse_decorated(),
            TokenKind::Def => self.parse_func_def(false, self.start(), self.empty_slice()),
            TokenKind::Async => self.parse_async_stmt(),
            TokenKind::Class => self.parse_class_def(self.empty_slice()),
            TokenKind::If => self.parse_if(),
            TokenKind::For => self.parse_for(false, self.start()),
            TokenKind::While => self.parse_while(),
            TokenKind::Try => self.parse_try(),
            TokenKind::With => self.parse_with(false, self.start()),
            TokenKind::Return => self.parse_return(),
            TokenKind::Raise => self.parse_raise(),
            TokenKind::Assert => self.parse_assert(),
            TokenKind::Import => self.parse_import(),
            TokenKind::From => self.parse_import_from(),
            TokenKind::Global => self.parse_global_nonlocal(true),
            TokenKind::Nonlocal => self.parse_global_nonlocal(false),
            TokenKind::Del => self.parse_del(),
            TokenKind::Pass => self.simple_stmt::<PassStmt>(TokenKind::Pass, Statement::Pass),
            TokenKind::Break => self.simple_stmt::<BreakStmt>(TokenKind::Break, Statement::Break),
            TokenKind::Continue => {
                self.simple_stmt::<ContinueStmt>(TokenKind::Continue, Statement::Continue)
            }
            _ => self.parse_expr_stmt(),
        }
    }

    fn simple_stmt<T: HasRange>(
        &mut self,
        kw: TokenKind,
        wrap: fn(&'ast T) -> Statement<'ast>,
    ) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(kw)?;
        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(wrap(self.arena.alloc(T::new(TextRange::new(start, end)))))
    }

    fn parse_block(&mut self) -> ParseResult<&'ast [Statement<'ast>]> {
        self.expect(TokenKind::Newline)?;
        self.expect(TokenKind::Indent)?;
        let mut stmts = Vec::new();
        while !self.check(TokenKind::Dedent) && !self.at_end() {
            self.skip_newline();
            if self.check(TokenKind::Dedent) || self.at_end() {
                break;
            }
            stmts.push(self.parse_stmt()?);
        }
        self.consume(TokenKind::Dedent);
        Ok(self.arena.alloc_slice(stmts))
    }

    fn parse_else(&mut self) -> ParseResult<&'ast [Statement<'ast>]> {
        if self.consume(TokenKind::Else) {
            self.expect_recover(TokenKind::Colon);
            self.parse_block()
        } else {
            Ok(self.empty_slice())
        }
    }

    fn parse_decorated(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        let mut decs = Vec::new();
        while self.consume(TokenKind::At) {
            decs.push(self.parse_expr()?);
            self.expect_newline()?;
        }
        let decorators = self.arena.alloc_slice(decs);
        match self.current.kind {
            TokenKind::Def => self.parse_func_def(false, start, decorators),
            TokenKind::Async => {
                self.advance();
                self.parse_func_def(true, start, decorators)
            }
            TokenKind::Class => self.parse_class_def(decorators),
            _ => Err(self.err("def or class")),
        }
    }

    fn parse_async_stmt(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Async)?;
        match self.current.kind {
            TokenKind::Def => self.parse_func_def(true, start, self.empty_slice()),
            TokenKind::For => self.parse_for(true, start),
            TokenKind::With => self.parse_with(true, start),
            _ => Err(self.err("def, for, or with")),
        }
    }

    fn parse_func_def(
        &mut self,
        is_async: bool,
        start: TextSize,
        decorators: &'ast [Expression<'ast>],
    ) -> ParseResult<Statement<'ast>> {
        self.expect(TokenKind::Def)?;
        let (name, params, returns, body) = self.func_sig_and_body()?;
        Ok(Statement::FunctionDef(self.arena.alloc(FunctionDef {
            name,
            params,
            body,
            decorators,
            returns,
            is_async,
            range: self.range(start),
        })))
    }

    fn func_sig_and_body(
        &mut self,
    ) -> ParseResult<(
        Identifier<'ast>,
        &'ast Parameters<'ast>,
        Option<Expression<'ast>>,
        &'ast [Statement<'ast>],
    )> {
        let name = self.parse_identifier()?;
        self.expect(TokenKind::LParen)?;
        let params = self.parse_params()?;
        self.expect_close(TokenKind::RParen)?;
        let returns = self.opt(TokenKind::Arrow, Self::parse_expr)?;
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        Ok((name, params, returns, body))
    }

    fn parse_params(&mut self) -> ParseResult<&'ast Parameters<'ast>> {
        let start = self.start();
        if self.check(TokenKind::RParen) {
            return Ok(self.empty_params());
        }

        let (mut posonlyargs, mut args, mut kwonlyargs) = (Vec::new(), Vec::new(), Vec::new());
        let (mut vararg, mut kwarg): (Option<&'ast Parameter>, Option<&'ast Parameter>) =
            (None, None);
        let (mut defaults, mut kw_defaults) = (Vec::new(), Vec::new());
        let mut seen_star = false;

        loop {
            if self.check(TokenKind::RParen) {
                break;
            }

            if self.consume(TokenKind::Slash) {
                if !seen_star {
                    posonlyargs.append(&mut args);
                }
                if !self.consume(TokenKind::Comma) {
                    break;
                }
                continue;
            }

            if self.consume(TokenKind::Star) {
                seen_star = true;
                if self.check(TokenKind::Comma) || self.check(TokenKind::RParen) {
                    if !self.consume(TokenKind::Comma) {
                        break;
                    }
                    continue;
                }
                vararg = Some(self.arena.alloc(self.parse_param()?));
                if !self.consume(TokenKind::Comma) {
                    break;
                }
                continue;
            }

            if self.consume(TokenKind::DoubleStar) {
                kwarg = Some(self.arena.alloc(self.parse_param()?));
                self.consume(TokenKind::Comma);
                break;
            }

            let p = self.parse_param()?;
            if seen_star {
                kw_defaults.push(p.default);
                kwonlyargs.push(Parameter {
                    name: p.name,
                    annotation: p.annotation,
                    default: None,
                    range: p.range,
                });
            } else {
                if let Some(d) = p.default {
                    defaults.push(d);
                }
                args.push(p);
            }
            if !self.consume(TokenKind::Comma) {
                break;
            }
        }

        Ok(self.arena.alloc(Parameters {
            posonlyargs: self.arena.alloc_slice(posonlyargs),
            args: self.arena.alloc_slice(args),
            vararg,
            kwonlyargs: self.arena.alloc_slice(kwonlyargs),
            kw_defaults: self.arena.alloc_slice(kw_defaults),
            kwarg,
            defaults: self.arena.alloc_slice(defaults),
            range: self.range(start),
        }))
    }

    fn parse_param(&mut self) -> ParseResult<Parameter<'ast>> {
        let start = self.start();
        let name = self.parse_identifier()?;
        let annotation = self.opt(TokenKind::Colon, Self::parse_expr)?;
        let default = self.opt(TokenKind::Eq, Self::parse_expr)?;
        Ok(Parameter {
            name,
            annotation,
            default,
            range: self.range(start),
        })
    }

    fn parse_class_def(
        &mut self,
        decorators: &'ast [Expression<'ast>],
    ) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Class)?;
        let name = self.parse_identifier()?;
        let (bases, keywords) = if self.consume(TokenKind::LParen) {
            let r = self.parse_class_args()?;
            self.expect_close(TokenKind::RParen)?;
            r
        } else {
            (self.empty_slice(), self.empty_slice())
        };
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        Ok(Statement::ClassDef(self.arena.alloc(ClassDef {
            name,
            bases,
            keywords,
            body,
            decorators,
            range: self.range(start),
        })))
    }

    fn parse_class_args(
        &mut self,
    ) -> ParseResult<(&'ast [Expression<'ast>], &'ast [Keyword<'ast>])> {
        let (mut bases, mut kws) = (Vec::new(), Vec::new());
        if self.check(TokenKind::RParen) {
            return Ok((self.empty_slice(), self.empty_slice()));
        }

        loop {
            let s = self.start();
            if self.consume(TokenKind::DoubleStar) {
                let v = self.parse_expr()?;
                kws.push(Keyword {
                    arg: None,
                    value: v,
                    range: self.range(s),
                });
            } else if self.check(TokenKind::Name) {
                let name = self.parse_identifier()?;
                if self.consume(TokenKind::Eq) {
                    let v = self.parse_expr()?;
                    kws.push(Keyword {
                        arg: Some(name),
                        value: v,
                        range: self.range(s),
                    });
                } else {
                    let mut e = Expression::Name(self.arena.alloc(NameExpr {
                        id: name,
                        range: self.range(s),
                    }));
                    while self.consume(TokenKind::Dot) {
                        let attr = self.parse_identifier()?;
                        e = Expression::Attribute(self.arena.alloc(AttributeExpr {
                            value: e,
                            attr,
                            range: self.range(s),
                        }));
                    }
                    bases.push(e);
                }
            } else {
                bases.push(self.parse_expr()?);
            }
            if !self.consume(TokenKind::Comma) || self.check(TokenKind::RParen) {
                break;
            }
        }
        Ok((self.arena.alloc_slice(bases), self.arena.alloc_slice(kws)))
    }

    fn parse_if(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::If)?;
        self.parse_if_inner(start)
    }

    fn parse_if_inner(&mut self, start: TextSize) -> ParseResult<Statement<'ast>> {
        let test = self.parse_expr_or_missing()?;
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        let orelse = if self.consume(TokenKind::Elif) {
            self.arena.alloc_slice([self.parse_if_inner(start)?])
        } else {
            self.parse_else()?
        };
        Ok(Statement::If(self.arena.alloc(IfStmt {
            test,
            body,
            orelse,
            range: self.range(start),
        })))
    }

    fn parse_for(&mut self, is_async: bool, start: TextSize) -> ParseResult<Statement<'ast>> {
        self.expect(TokenKind::For)?;
        let target = self.parse_expr_or_missing()?;
        self.expect(TokenKind::In)?;
        let iter = self.parse_expr_or_missing()?;
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        let orelse = self.parse_else()?;
        Ok(Statement::For(self.arena.alloc(ForStmt {
            target,
            iter,
            body,
            orelse,
            is_async,
            range: self.range(start),
        })))
    }

    fn parse_while(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::While)?;
        let test = self.parse_expr_or_missing()?;
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        let orelse = self.parse_else()?;
        Ok(Statement::While(self.arena.alloc(WhileStmt {
            test,
            body,
            orelse,
            range: self.range(start),
        })))
    }

    fn parse_try(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Try)?;
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        let mut handlers = Vec::new();
        while self.check(TokenKind::Except) {
            handlers.push(self.parse_except()?);
        }
        let orelse = if self.consume(TokenKind::Else) {
            self.expect_recover(TokenKind::Colon);
            self.parse_block()?
        } else {
            self.empty_slice()
        };
        let finalbody = if self.consume(TokenKind::Finally) {
            self.expect_recover(TokenKind::Colon);
            self.parse_block()?
        } else {
            self.empty_slice()
        };
        Ok(Statement::Try(self.arena.alloc(TryStmt {
            body,
            handlers: self.arena.alloc_slice(handlers),
            orelse,
            finalbody,
            range: self.range(start),
        })))
    }

    fn parse_except(&mut self) -> ParseResult<ExceptHandler<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Except)?;
        let (typ, name) = if self.check(TokenKind::Colon) {
            (None, None)
        } else {
            (
                Some(self.parse_expr()?),
                self.opt(TokenKind::As, Self::parse_identifier)?,
            )
        };
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        Ok(ExceptHandler {
            typ,
            name,
            body,
            range: self.range(start),
        })
    }

    fn parse_with(&mut self, is_async: bool, start: TextSize) -> ParseResult<Statement<'ast>> {
        self.expect(TokenKind::With)?;
        let items = self.comma_list(TokenKind::Colon, |p| {
            let s = p.start();
            let context_expr = p.parse_expr()?;
            let optional_vars = p.opt(TokenKind::As, Self::parse_expr)?;
            Ok(WithItem {
                context_expr,
                optional_vars,
                range: p.range(s),
            })
        })?;
        self.expect_recover(TokenKind::Colon);
        let body = self.parse_block()?;
        let items = self.arena.alloc_slice(items);
        Ok(Statement::With(self.arena.alloc(WithStmt {
            items,
            body,
            is_async,
            range: self.range(start),
        })))
    }

    fn parse_return(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Return)?;
        let value = if self.check(TokenKind::Newline) || self.at_end() {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(Statement::Return(self.arena.alloc(ReturnStmt {
            value,
            range: TextRange::new(start, end),
        })))
    }

    fn parse_raise(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Raise)?;
        let (exc, cause) = if self.check(TokenKind::Newline) || self.at_end() {
            (None, None)
        } else {
            (
                Some(self.parse_expr()?),
                self.opt(TokenKind::From, Self::parse_expr)?,
            )
        };
        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(Statement::Raise(self.arena.alloc(RaiseStmt {
            exc,
            cause,
            range: TextRange::new(start, end),
        })))
    }

    fn parse_assert(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Assert)?;
        let test = self.parse_expr()?;
        let msg = self.opt(TokenKind::Comma, Self::parse_expr)?;
        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(Statement::Assert(self.arena.alloc(AssertStmt {
            test,
            msg,
            range: TextRange::new(start, end),
        })))
    }

    fn parse_import(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Import)?;
        let names = self.parse_aliases()?;
        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(Statement::Import(self.arena.alloc(ImportStmt {
            names,
            range: TextRange::new(start, end),
        })))
    }

    fn parse_import_from(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::From)?;
        let mut level = 0u32;
        while self.consume(TokenKind::Dot) {
            level += 1;
        }
        let module = if self.check(TokenKind::Name) {
            Some(self.parse_dotted_name()?)
        } else {
            None
        };
        self.expect(TokenKind::Import)?;

        let names = if self.consume(TokenKind::LParen) {
            let aliases = self.comma_list(TokenKind::RParen, |p| {
                let s = p.start();
                let name = p.parse_dotted_name()?;
                let asname = p.opt(TokenKind::As, Self::parse_identifier)?;
                Ok(Alias {
                    name,
                    asname,
                    range: p.range(s),
                })
            })?;
            self.expect_close(TokenKind::RParen)?;
            self.arena.alloc_slice(aliases)
        } else {
            self.parse_aliases()?
        };

        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(Statement::ImportFrom(self.arena.alloc(ImportFromStmt {
            module,
            names,
            level,
            range: TextRange::new(start, end),
        })))
    }

    fn parse_aliases(&mut self) -> ParseResult<&'ast [Alias<'ast>]> {
        let aliases = self.comma_list(TokenKind::Newline, |p| {
            let s = p.start();
            let name = p.parse_dotted_name()?;
            let asname = p.opt(TokenKind::As, Self::parse_identifier)?;
            Ok(Alias {
                name,
                asname,
                range: p.range(s),
            })
        })?;
        Ok(self.arena.alloc_slice(aliases))
    }

    fn parse_dotted_name(&mut self) -> ParseResult<Identifier<'ast>> {
        let first = self.parse_identifier()?;

        if !self.check(TokenKind::Dot) {
            return Ok(first);
        }

        // build dotted name string
        let mut name = String::from(first);
        while self.consume(TokenKind::Dot) {
            if !self.check(TokenKind::Name) {
                break;
            }
            name.push('.');
            name.push_str(self.token_text());
            self.advance();
        }

        Ok(self.arena.alloc_str(&name))
    }

    fn parse_global_nonlocal(&mut self, is_global: bool) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.advance();
        let names = self.parse_ident_list()?;
        let end = self.previous.range.end();
        self.expect_newline()?;
        if is_global {
            Ok(Statement::Global(self.arena.alloc(GlobalStmt {
                names,
                range: TextRange::new(start, end),
            })))
        } else {
            Ok(Statement::Nonlocal(self.arena.alloc(NonlocalStmt {
                names,
                range: TextRange::new(start, end),
            })))
        }
    }

    fn parse_del(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Del)?;
        let targets = self.parse_expr_list()?;
        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(Statement::Delete(self.arena.alloc(DeleteStmt {
            targets,
            range: TextRange::new(start, end),
        })))
    }

    fn parse_expr_stmt(&mut self) -> ParseResult<Statement<'ast>> {
        let start = self.start();
        let expr = self.parse_expr()?;

        // annotated assignment: `x: int = 1`
        if self.consume(TokenKind::Colon) {
            let annotation = self.parse_expr()?;
            let value = self.opt(TokenKind::Eq, Self::parse_expr)?;
            let end = self.previous.range.end();
            self.expect_newline()?;
            return Ok(Statement::AnnAssign(self.arena.alloc(AnnAssignStmt {
                target: expr,
                annotation,
                value,
                simple: matches!(expr, Expression::Name(_)),
                range: TextRange::new(start, end),
            })));
        }

        // simple or chained assignment: `x = 1` or `a = b = 1`
        if self.check(TokenKind::Eq) {
            let mut targets = vec![expr];
            while self.consume(TokenKind::Eq) {
                targets.push(self.parse_expr()?);
            }

            // last item is the value, everything before is a target
            let value = targets.pop().unwrap();
            let end = self.previous.range.end();
            self.expect_newline()?;

            return Ok(Statement::Assign(self.arena.alloc(AssignStmt {
                targets: self.arena.alloc_slice(targets),
                value,
                range: TextRange::new(start, end),
            })));
        }

        // augmented assignment: `x += 1`
        if let Some(op) = self.aug_op() {
            self.advance();
            let value = self.parse_expr()?;
            let end = self.previous.range.end();
            self.expect_newline()?;
            return Ok(Statement::AugAssign(self.arena.alloc(AugAssignStmt {
                target: expr,
                op,
                value,
                range: TextRange::new(start, end),
            })));
        }

        // bare expression statement
        let end = self.previous.range.end();
        self.expect_newline()?;
        Ok(Statement::Expr(self.arena.alloc(ExprStmt {
            value: expr,
            range: TextRange::new(start, end),
        })))
    }

    fn aug_op(&self) -> Option<BinOp> {
        match self.current.kind {
            TokenKind::PlusEq => Some(BinOp::Add),
            TokenKind::MinusEq => Some(BinOp::Sub),
            TokenKind::StarEq => Some(BinOp::Mult),
            TokenKind::SlashEq => Some(BinOp::Div),
            TokenKind::PercentEq => Some(BinOp::Mod),
            _ => None,
        }
    }

    fn parse_identifier(&mut self) -> ParseResult<Identifier<'ast>> {
        if self.check(TokenKind::Name) {
            let id = self.arena.alloc_str(self.token_text());
            self.advance();
            Ok(id)
        } else {
            Err(self.err("identifier"))
        }
    }

    fn parse_ident_list(&mut self) -> ParseResult<&'ast [Identifier<'ast>]> {
        let ids = self.comma_list(TokenKind::Newline, Self::parse_identifier)?;
        Ok(self.arena.alloc_slice(ids))
    }

    fn parse_expr_list(&mut self) -> ParseResult<&'ast [Expression<'ast>]> {
        let exprs = self.comma_list(TokenKind::Newline, Self::parse_expr)?;
        Ok(self.arena.alloc_slice(exprs))
    }

    fn parse_expr(&mut self) -> ParseResult<Expression<'ast>> {
        if self.check(TokenKind::Yield) {
            self.parse_yield()
        } else {
            self.parse_or()
        }
    }

    fn parse_named(&mut self) -> ParseResult<Expression<'ast>> {
        let e = self.parse_ternary()?;
        if self.consume(TokenKind::ColonEq) {
            let v = self.parse_named()?;
            let range = TextRange::new(e.range().start(), v.range().end());
            return Ok(Expression::Named(self.arena.alloc(NamedExpr {
                target: e,
                value: v,
                range,
            })));
        }
        Ok(e)
    }

    fn parse_ternary(&mut self) -> ParseResult<Expression<'ast>> {
        let e = self.parse_or()?;
        if self.consume(TokenKind::If) {
            let test = self.parse_or()?;
            self.expect(TokenKind::Else)?;
            let orelse = self.parse_ternary()?;
            let range = TextRange::new(e.range().start(), orelse.range().end());
            return Ok(Expression::IfExp(self.arena.alloc(IfExpr {
                test,
                body: e,
                orelse,
                range,
            })));
        }
        Ok(e)
    }

    fn parse_or(&mut self) -> ParseResult<Expression<'ast>> {
        self.parse_bool_op(TokenKind::Or, BoolOp::Or, Self::parse_and)
    }
    fn parse_and(&mut self) -> ParseResult<Expression<'ast>> {
        self.parse_bool_op(TokenKind::And, BoolOp::And, Self::parse_not)
    }

    fn parse_bool_op<F>(
        &mut self,
        tok: TokenKind,
        op: BoolOp,
        next: F,
    ) -> ParseResult<Expression<'ast>>
    where
        F: Fn(&mut Self) -> ParseResult<Expression<'ast>>,
    {
        let mut left = next(self)?;
        while self.consume(tok) {
            let right = next(self)?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BoolOp(self.arena.alloc(BoolOpExpr {
                op,
                values: self.arena.alloc_slice([left, right]),
                range,
            }));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> ParseResult<Expression<'ast>> {
        if self.consume(TokenKind::Not) {
            let start = self.previous.range.start();
            let operand = self.parse_not()?;
            return Ok(Expression::UnaryOp(self.arena.alloc(UnaryOpExpr {
                op: UnaryOp::Not,
                operand,
                range: self.range(start),
            })));
        }
        self.parse_cmp()
    }

    fn parse_cmp(&mut self) -> ParseResult<Expression<'ast>> {
        let left = self.parse_binop(PREC_BITOR)?;

        let op = if self.consume(TokenKind::Is) {
            if self.consume(TokenKind::Not) {
                CompareOp::IsNot
            } else {
                CompareOp::Is
            }
        } else if self.consume(TokenKind::Not) {
            self.expect(TokenKind::In)?;
            CompareOp::NotIn
        } else if let Some(op) = self.cmp_op() {
            self.advance();
            op
        } else {
            return Ok(left);
        };

        let right = self.parse_binop(PREC_BITOR)?;
        let range = TextRange::new(left.range().start(), right.range().end());
        Ok(Expression::Compare(self.arena.alloc(CompareExpr {
            left,
            op: self.arena.alloc_slice([op]),
            comparators: self.arena.alloc_slice([right]),
            range,
        })))
    }

    fn cmp_op(&self) -> Option<CompareOp> {
        match self.current.kind {
            TokenKind::EqEq => Some(CompareOp::Eq),
            TokenKind::NotEq => Some(CompareOp::NotEq),
            TokenKind::Lt => Some(CompareOp::Lt),
            TokenKind::LtEq => Some(CompareOp::LtE),
            TokenKind::Gt => Some(CompareOp::Gt),
            TokenKind::GtEq => Some(CompareOp::GtE),
            TokenKind::In => Some(CompareOp::In),
            _ => None,
        }
    }

    fn parse_binop(&mut self, min_prec: u8) -> ParseResult<Expression<'ast>> {
        let mut left = self.parse_unary()?;
        loop {
            let Some((op, prec, right_assoc)) = binop_prec(self.current.kind) else {
                break;
            };
            if prec < min_prec {
                break;
            }
            self.advance();
            let right = self.parse_binop(if right_assoc { prec } else { prec + 1 })?;
            let range = TextRange::new(left.range().start(), right.range().end());
            left = Expression::BinOp(self.arena.alloc(BinOpExpr {
                left,
                op,
                right,
                range,
            }));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> ParseResult<Expression<'ast>> {
        if self.check(TokenKind::Await) {
            return self.parse_await();
        }
        let op = match self.current.kind {
            TokenKind::Minus => UnaryOp::USub,
            TokenKind::Plus => UnaryOp::UAdd,
            TokenKind::Tilde => UnaryOp::Invert,
            _ => return self.parse_postfix(),
        };
        let start = self.start();
        self.advance();
        let operand = self.parse_unary()?;
        Ok(Expression::UnaryOp(self.arena.alloc(UnaryOpExpr {
            op,
            operand,
            range: self.range(start),
        })))
    }

    fn parse_await(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Await)?;
        let value = self.parse_unary()?;
        Ok(Expression::Await(self.arena.alloc(AwaitExpr {
            value,
            range: self.range(start),
        })))
    }

    fn parse_postfix(&mut self) -> ParseResult<Expression<'ast>> {
        let mut e = self.parse_atom()?;
        loop {
            if self.consume(TokenKind::LParen) {
                let (args, keywords) = self.parse_call_args()?;
                self.expect_close(TokenKind::RParen)?;
                e = Expression::Call(self.arena.alloc(CallExpr {
                    func: e,
                    args,
                    keywords,
                    range: TextRange::new(e.range().start(), self.previous.range.end()),
                }));
            } else if self.consume(TokenKind::Dot) {
                let attr = self.parse_identifier()?;
                e = Expression::Attribute(self.arena.alloc(AttributeExpr {
                    value: e,
                    attr,
                    range: TextRange::new(e.range().start(), self.previous.range.end()),
                }));
            } else if self.consume(TokenKind::LBracket) {
                let slice = self.parse_slice()?;
                self.expect_close(TokenKind::RBracket)?;
                e = Expression::Subscript(self.arena.alloc(SubscriptExpr {
                    value: e,
                    slice,
                    range: TextRange::new(e.range().start(), self.previous.range.end()),
                }));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_atom(&mut self) -> ParseResult<Expression<'ast>> {
        let range = self.current.range;
        match self.current.kind {
            TokenKind::Lambda => self.parse_lambda(),
            TokenKind::Star => {
                let s = self.start();
                self.advance();
                let value = self.parse_expr()?;
                Ok(Expression::Starred(self.arena.alloc(StarredExpr {
                    value,
                    range: self.range(s),
                })))
            }
            TokenKind::Ellipsis => {
                self.advance();
                Ok(Expression::Ellipsis(
                    self.arena.alloc(EllipsisExpr { range }),
                ))
            }
            TokenKind::Int => {
                let value = self.token_text().parse().unwrap_or(0);
                self.advance();
                Ok(Expression::Int(self.arena.alloc(IntExpr { value, range })))
            }
            TokenKind::Float => {
                let value = self.token_text().parse().unwrap_or(0.0);
                self.advance();
                Ok(Expression::Float(
                    self.arena.alloc(FloatExpr { value, range }),
                ))
            }
            TokenKind::String => {
                let t = self.token_text();
                let value = self.arena.alloc_str(strip_string_quotes(t));
                self.advance();
                Ok(Expression::String(
                    self.arena.alloc(StringExpr { value, range }),
                ))
            }
            TokenKind::FString => {
                let t = self.token_text();
                let inner = strip_string_quotes(t);
                let literal_part = FStringPart::Literal(self.arena.alloc_str(inner));
                let values = self.arena.alloc_slice([literal_part]);
                self.advance();
                Ok(Expression::FString(
                    self.arena.alloc(FStringExpr { values, range }),
                ))
            }
            TokenKind::FStringStart => self.parse_fstring(),
            TokenKind::True => {
                self.advance();
                Ok(Expression::Bool(
                    self.arena.alloc(BoolExpr { value: true, range }),
                ))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expression::Bool(self.arena.alloc(BoolExpr {
                    value: false,
                    range,
                })))
            }
            TokenKind::None => {
                self.advance();
                Ok(Expression::None(self.arena.alloc(NoneExpr { range })))
            }
            TokenKind::Name => {
                let id = self.arena.alloc_str(self.token_text());
                self.advance();
                Ok(Expression::Name(self.arena.alloc(NameExpr { id, range })))
            }
            TokenKind::LParen => self.parse_paren(),
            TokenKind::LBracket => self.parse_list(),
            TokenKind::LBrace => self.parse_dict_or_set(),
            _ => Err(self.err("expression")),
        }
    }

    fn parse_fstring(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        self.advance();

        let mut parts = Vec::new();

        loop {
            match self.current.kind {
                TokenKind::FStringEnd => {
                    self.advance();
                    break;
                }
                TokenKind::FStringLiteral => {
                    let text = self.token_text();
                    let literal = self.arena.alloc_str(text);
                    parts.push(FStringPart::Literal(literal));
                    self.advance();
                }
                TokenKind::LBrace => {
                    let fv = self.parse_fstring_value()?;
                    parts.push(FStringPart::FormattedValue(self.arena.alloc(fv)));
                }
                TokenKind::Eof => {
                    if self.mode == ParseMode::Recovering {
                        self.errors.push(ParseError::UnexpectedEof);
                        break;
                    }
                    return Err(ParseError::UnexpectedEof);
                }
                _ => {
                    if self.mode == ParseMode::Recovering {
                        self.errors.push(self.err("f-string content"));
                        self.advance();
                    } else {
                        return Err(self.err("f-string content"));
                    }
                }
            }
        }

        let values = self.arena.alloc_slice(parts);
        Ok(Expression::FString(self.arena.alloc(FStringExpr {
            values,
            range: self.range(start),
        })))
    }

    fn parse_fstring_value(&mut self) -> ParseResult<FormattedValue<'ast>> {
        let start = self.start();
        self.advance();

        let value = self.parse_expr()?;

        // optional conversion: !s, !r, !a
        let conversion = if self.check(TokenKind::Not) {
            self.advance();
            if self.check(TokenKind::Name) {
                let conv_text = self.token_text();
                let conv = conv_text.chars().next();
                self.advance();
                conv
            } else {
                None
            }
        } else {
            None
        };

        // optional format spec after `:`
        // for now, consume everything until `}` as opaque text
        let format_spec = if self.check(TokenKind::Colon) {
            self.advance();
            let spec_start = self.start();
            let mut depth = 0u32;
            while !self.at_end() {
                if self.check(TokenKind::RBrace) && depth == 0 {
                    break;
                }
                if self.check(TokenKind::LBrace) {
                    depth += 1;
                }
                if self.check(TokenKind::RBrace) {
                    depth = depth.saturating_sub(1);
                }
                self.advance();
            }
            let spec_end = self.start();
            let spec_text = &self.source[spec_start.to_usize()..spec_end.to_usize()];
            if !spec_text.is_empty() {
                let literal = FStringPart::Literal(self.arena.alloc_str(spec_text));
                Some(self.arena.alloc_slice([literal]) as &[FStringPart<'ast>])
            } else {
                None
            }
        } else {
            None
        };

        self.expect_close(TokenKind::RBrace)?;

        Ok(FormattedValue {
            value,
            conversion,
            format_spec,
            range: self.range(start),
        })
    }

    fn parse_paren(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        self.advance();
        if self.check(TokenKind::RParen) {
            self.advance();
            return Ok(Expression::Tuple(self.arena.alloc(TupleExpr {
                elts: self.empty_slice(),
                range: self.range(start),
            })));
        }
        if self.check(TokenKind::Yield) {
            let e = self.parse_yield()?;
            self.expect_close(TokenKind::RParen)?;
            return Ok(e);
        }
        let first = self.parse_named()?;
        if self.check(TokenKind::For) || self.check(TokenKind::Async) {
            let gens = self.parse_comp_clauses()?;
            self.expect_close(TokenKind::RParen)?;
            return Ok(Expression::GeneratorExp(self.arena.alloc(GeneratorExpr {
                elt: first,
                generators: gens,
                range: self.range(start),
            })));
        }
        if self.consume(TokenKind::Comma) {
            let mut elts = vec![first];
            if !self.check(TokenKind::RParen) {
                elts.extend(self.comma_list(TokenKind::RParen, Self::parse_expr)?);
            }
            self.expect_close(TokenKind::RParen)?;
            return Ok(Expression::Tuple(self.arena.alloc(TupleExpr {
                elts: self.arena.alloc_slice(elts),
                range: self.range(start),
            })));
        }
        self.expect(TokenKind::RParen)?;
        Ok(first)
    }

    fn parse_list(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        self.advance();
        if self.check(TokenKind::RBracket) {
            self.advance();
            return Ok(Expression::List(self.arena.alloc(ListExpr {
                elts: self.empty_slice(),
                range: self.range(start),
            })));
        }
        let first = self.parse_named()?;
        if self.check(TokenKind::For) || self.check(TokenKind::Async) {
            let gens = self.parse_comp_clauses()?;
            self.expect_close(TokenKind::RBracket)?;
            return Ok(Expression::ListComp(self.arena.alloc(ListCompExpr {
                elt: first,
                generators: gens,
                range: self.range(start),
            })));
        }
        let mut elts = vec![first];
        if self.consume(TokenKind::Comma) && !self.check(TokenKind::RBracket) {
            elts.extend(self.comma_list(TokenKind::RBracket, Self::parse_named)?);
        }
        self.expect_close(TokenKind::RBracket)?;
        Ok(Expression::List(self.arena.alloc(ListExpr {
            elts: self.arena.alloc_slice(elts),
            range: self.range(start),
        })))
    }

    fn parse_dict_or_set(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        self.advance();
        if self.check(TokenKind::RBrace) {
            self.advance();
            return Ok(Expression::Dict(self.arena.alloc(DictExpr {
                keys: self.empty_slice(),
                values: self.empty_slice(),
                range: self.range(start),
            })));
        }
        if self.check(TokenKind::DoubleStar) {
            return self.parse_dict_unpack(start);
        }

        let first = self.parse_named()?;
        if self.consume(TokenKind::Colon) {
            let fv = self.parse_named()?;
            if self.check(TokenKind::For) || self.check(TokenKind::Async) {
                let gens = self.parse_comp_clauses()?;
                self.expect_close(TokenKind::RBrace)?;
                return Ok(Expression::DictComp(self.arena.alloc(DictCompExpr {
                    key: first,
                    value: fv,
                    generators: gens,
                    range: self.range(start),
                })));
            }
            let (mut keys, mut vals) = (vec![Some(first)], vec![fv]);
            while self.consume(TokenKind::Comma) && !self.check(TokenKind::RBrace) {
                if self.consume(TokenKind::DoubleStar) {
                    keys.push(None);
                    vals.push(self.parse_named()?);
                } else {
                    keys.push(Some(self.parse_named()?));
                    self.expect(TokenKind::Colon)?;
                    vals.push(self.parse_named()?);
                }
            }
            self.expect_close(TokenKind::RBrace)?;
            return Ok(Expression::Dict(self.arena.alloc(DictExpr {
                keys: self.arena.alloc_slice(keys),
                values: self.arena.alloc_slice(vals),
                range: self.range(start),
            })));
        }
        if self.check(TokenKind::For) || self.check(TokenKind::Async) {
            let gens = self.parse_comp_clauses()?;
            self.expect_close(TokenKind::RBrace)?;
            return Ok(Expression::SetComp(self.arena.alloc(SetCompExpr {
                elt: first,
                generators: gens,
                range: self.range(start),
            })));
        }
        let mut elts = vec![first];
        while self.consume(TokenKind::Comma) && !self.check(TokenKind::RBrace) {
            elts.push(self.parse_named()?);
        }
        self.expect_close(TokenKind::RBrace)?;
        Ok(Expression::Set(self.arena.alloc(SetExpr {
            elts: self.arena.alloc_slice(elts),
            range: self.range(start),
        })))
    }

    fn parse_dict_unpack(&mut self, start: TextSize) -> ParseResult<Expression<'ast>> {
        let (mut keys, mut vals): (Vec<Option<Expression>>, Vec<Expression>) =
            (Vec::new(), Vec::new());
        loop {
            if self.consume(TokenKind::DoubleStar) {
                keys.push(None);
                vals.push(self.parse_named()?);
            } else {
                keys.push(Some(self.parse_named()?));
                self.expect(TokenKind::Colon)?;
                vals.push(self.parse_named()?);
            }
            if !self.consume(TokenKind::Comma) || self.check(TokenKind::RBrace) {
                break;
            }
        }
        self.expect_close(TokenKind::RBrace)?;
        Ok(Expression::Dict(self.arena.alloc(DictExpr {
            keys: self.arena.alloc_slice(keys),
            values: self.arena.alloc_slice(vals),
            range: self.range(start),
        })))
    }

    fn parse_comp_clauses(&mut self) -> ParseResult<&'ast [Comprehension<'ast>]> {
        let mut gens = Vec::new();
        loop {
            let s = self.start();
            let is_async = self.consume(TokenKind::Async);
            if !self.consume(TokenKind::For) {
                if is_async {
                    return Err(self.err("for"));
                }
                break;
            }
            let target = self.parse_expr()?;
            self.expect(TokenKind::In)?;
            let iter = self.parse_or()?;
            let mut ifs = Vec::new();
            while self.consume(TokenKind::If) {
                ifs.push(self.parse_or()?);
            }
            gens.push(Comprehension {
                target,
                iter,
                ifs: self.arena.alloc_slice(ifs),
                is_async,
                range: self.range(s),
            });
        }
        Ok(self.arena.alloc_slice(gens))
    }

    fn parse_call_args(
        &mut self,
    ) -> ParseResult<(&'ast [Expression<'ast>], &'ast [Keyword<'ast>])> {
        let (mut args, mut kws) = (Vec::new(), Vec::new());
        if self.check(TokenKind::RParen) {
            return Ok((self.empty_slice(), self.empty_slice()));
        }
        loop {
            let s = self.start();
            if self.consume(TokenKind::DoubleStar) {
                kws.push(Keyword {
                    arg: None,
                    value: self.parse_expr()?,
                    range: self.range(s),
                });
            } else if self.consume(TokenKind::Star) {
                args.push(Expression::Starred(self.arena.alloc(StarredExpr {
                    value: self.parse_expr()?,
                    range: self.range(s),
                })));
            } else {
                let e = self.parse_expr()?;
                if let Expression::Name(n) = e {
                    if self.consume(TokenKind::Eq) {
                        kws.push(Keyword {
                            arg: Some(n.id),
                            value: self.parse_expr()?,
                            range: self.range(s),
                        });
                    } else {
                        args.push(e);
                    }
                } else {
                    args.push(e);
                }
            }
            if !self.consume(TokenKind::Comma) || self.check(TokenKind::RParen) {
                break;
            }
        }
        Ok((self.arena.alloc_slice(args), self.arena.alloc_slice(kws)))
    }

    fn parse_slice(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        let lower = if self.check(TokenKind::Colon) {
            None
        } else {
            let e = self.parse_expr()?;
            if !self.check(TokenKind::Colon) {
                return Ok(e);
            }
            Some(e)
        };
        self.expect(TokenKind::Colon)?;
        let upper = if self.check(TokenKind::Colon)
            || self.check(TokenKind::RBracket)
            || self.check(TokenKind::Comma)
        {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let step = if self.consume(TokenKind::Colon) {
            if self.check(TokenKind::RBracket) || self.check(TokenKind::Comma) {
                None
            } else {
                Some(self.parse_expr()?)
            }
        } else {
            None
        };
        Ok(Expression::Slice(self.arena.alloc(SliceExpr {
            lower,
            upper,
            step,
            range: self.range(start),
        })))
    }

    fn parse_yield(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Yield)?;
        if self.consume(TokenKind::From) {
            let v = self.parse_expr()?;
            return Ok(Expression::YieldFrom(self.arena.alloc(YieldFromExpr {
                value: v,
                range: self.range(start),
            })));
        }
        let value = if self.check(TokenKind::Newline)
            || self.check(TokenKind::RParen)
            || self.check(TokenKind::RBracket)
            || self.check(TokenKind::Comma)
            || self.at_end()
        {
            None
        } else {
            Some(self.parse_expr()?)
        };
        Ok(Expression::Yield(self.arena.alloc(YieldExpr {
            value,
            range: self.range(start),
        })))
    }

    fn parse_lambda(&mut self) -> ParseResult<Expression<'ast>> {
        let start = self.start();
        self.expect(TokenKind::Lambda)?;
        let params = if self.check(TokenKind::Colon) {
            self.empty_params()
        } else {
            self.parse_lambda_params()?
        };
        self.expect(TokenKind::Colon)?;
        let body = self.parse_ternary()?;
        Ok(Expression::Lambda(self.arena.alloc(LambdaExpr {
            params,
            body,
            range: self.range(start),
        })))
    }

    fn parse_lambda_params(&mut self) -> ParseResult<&'ast Parameters<'ast>> {
        let start = self.start();
        let (mut args, mut kwonlyargs) = (Vec::new(), Vec::new());
        let (mut vararg, mut kwarg): (Option<&'ast Parameter>, Option<&'ast Parameter>) =
            (None, None);
        let (mut defaults, mut kw_defaults) = (Vec::new(), Vec::new());
        let mut seen_star = false;

        loop {
            if self.check(TokenKind::Colon) {
                break;
            }
            if self.consume(TokenKind::Star) {
                seen_star = true;
                if self.check(TokenKind::Comma) || self.check(TokenKind::Colon) {
                    self.consume(TokenKind::Comma);
                    continue;
                }
                let s = self.start();
                let name = self.parse_identifier()?;
                vararg = Some(self.arena.alloc(Parameter {
                    name,
                    annotation: None,
                    default: None,
                    range: self.range(s),
                }));
                if !self.consume(TokenKind::Comma) {
                    break;
                }
                continue;
            }
            if self.consume(TokenKind::DoubleStar) {
                let s = self.start();
                let name = self.parse_identifier()?;
                kwarg = Some(self.arena.alloc(Parameter {
                    name,
                    annotation: None,
                    default: None,
                    range: self.range(s),
                }));
                self.consume(TokenKind::Comma);
                break;
            }
            let s = self.start();
            let name = self.parse_identifier()?;
            let default = self.opt(TokenKind::Eq, Self::parse_expr)?;
            let p = Parameter {
                name,
                annotation: None,
                default: if seen_star { None } else { default },
                range: self.range(s),
            };
            if seen_star {
                kw_defaults.push(default);
                kwonlyargs.push(p);
            } else {
                if let Some(d) = default {
                    defaults.push(d);
                }
                args.push(p);
            }
            if !self.consume(TokenKind::Comma) {
                break;
            }
        }

        Ok(self.arena.alloc(Parameters {
            posonlyargs: self.empty_slice(),
            args: self.arena.alloc_slice(args),
            vararg,
            kwonlyargs: self.arena.alloc_slice(kwonlyargs),
            kw_defaults: self.arena.alloc_slice(kw_defaults),
            kwarg,
            defaults: self.arena.alloc_slice(defaults),
            range: self.range(start),
        }))
    }
}

fn strip_string_quotes(text: &str) -> &str {
    let bytes = text.as_bytes();
    let len = bytes.len();

    // skip prefix characters
    let mut start = 0;
    while start < len
        && matches!(
            bytes[start],
            b'r' | b'R' | b'b' | b'B' | b'f' | b'F' | b'u' | b'U'
        )
    {
        start += 1;
        // max 2 prefix chars (rb, fr)
        if start >= 2 {
            break;
        }
    }

    if start >= len {
        return "";
    }

    let quote = bytes[start];
    if quote != b'\'' && quote != b'"' {
        return &text[start..];
    }

    // handle triple quote
    let triple = start + 2 < len && bytes[start + 1] == quote && bytes[start + 2] == quote;

    let quote_len = if triple { 3 } else { 1 };
    let content_start = start + quote_len;

    // strip trailing quotes
    let mut content_end = len;
    if triple {
        if content_end >= 3
            && bytes[content_end - 1] == quote
            && bytes[content_end - 2] == quote
            && bytes[content_end - 3] == quote
            && content_end - 3 >= content_start
        {
            content_end -= 3;
        }
    } else if content_end >= 1
        && bytes[content_end - 1] == quote
        && content_end - 1 >= content_start
    {
        content_end -= 1;
    }

    &text[content_start..content_end]
}

trait HasRange {
    fn new(range: TextRange) -> Self;
}
impl HasRange for PassStmt {
    fn new(range: TextRange) -> Self {
        Self { range }
    }
}
impl HasRange for BreakStmt {
    fn new(range: TextRange) -> Self {
        Self { range }
    }
}
impl HasRange for ContinueStmt {
    fn new(range: TextRange) -> Self {
        Self { range }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_precedence() {
        let arena = AstArena::new();
        let module = Parser::new("1 + 2 * 3", &arena).parse_module().unwrap();
        assert_eq!(module.body.len(), 1);
    }

    #[test]
    fn function_with_params() {
        let arena = AstArena::new();
        let module = Parser::new("def add(a, b):\n    return a + b", &arena)
            .parse_module()
            .unwrap();
        assert!(matches!(module.body[0], Statement::FunctionDef(_)));
    }

    #[test]
    fn class_def() {
        let arena = AstArena::new();
        let module = Parser::new("class Foo:\n    pass", &arena)
            .parse_module()
            .unwrap();
        assert!(matches!(module.body[0], Statement::ClassDef(_)));
    }

    #[test]
    fn if_elif_else() {
        let arena = AstArena::new();
        let module = Parser::new("if x:\n    a\nelif y:\n    b\nelse:\n    c", &arena)
            .parse_module()
            .unwrap();
        assert!(matches!(module.body[0], Statement::If(_)));
    }

    #[test]
    fn list_literal() {
        let arena = AstArena::new();
        let module = Parser::new("[1, 2, 3]", &arena).parse_module().unwrap();
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
        let module = Parser::new("{a: 1, b: 2}", &arena).parse_module().unwrap();
        assert!(matches!(
            module.body[0],
            Statement::Expr(ExprStmt {
                value: Expression::Dict(_),
                ..
            })
        ));
    }

    #[test]
    fn range_is_exact_source_span() {
        let arena = AstArena::new();
        let source = "x = 123\n";
        let module = Parser::new(source, &arena).parse_module().unwrap();

        let stmt = &module.body[0];
        let range = stmt.range();
        let text = &source[range.start().to_usize()..range.end().to_usize()];

        // should be "x = 123", not "x = 123\n"
        assert_eq!(text, "x = 123");
    }

    #[test]
    fn simple_assignment_parses() {
        let arena = AstArena::new();
        let module = Parser::new("x = 1", &arena).parse_module().unwrap();
        assert_eq!(module.body.len(), 1);
        assert!(matches!(module.body[0], Statement::Assign(_)));
    }

    #[test]
    fn chained_assignment() {
        let arena = AstArena::new();
        let source = "a = b = 123\n";
        let module = Parser::new(source, &arena).parse_module().unwrap();

        assert_eq!(module.body.len(), 1);
        let Statement::Assign(assign) = &module.body[0] else {
            panic!("expected Assign statement");
        };
        assert_eq!(assign.targets.len(), 2);
    }

    mod strings {
        use super::*;

        #[test]
        fn empty() {
            let arena = AstArena::new();
            let module = Parser::new("''", &arena).parse_module().unwrap();
            let Statement::Expr(e) = module.body[0] else {
                panic!("expected Expr statement");
            };
            let Expression::String(s) = e.value else {
                panic!("expected String expression");
            };
            assert_eq!(s.value, "");
        }

        #[test]
        fn string_prefixes() {
            let arena = AstArena::new();
            let module = Parser::new("r'hello'", &arena).parse_module().unwrap();
            let Statement::Expr(e) = module.body[0] else {
                panic!("expected Expr statement");
            };
            let Expression::String(s) = e.value else {
                panic!("expected String expression");
            };
            assert_eq!(s.value, "hello");
        }

        #[test]
        fn triple_quoted() {
            let arena = AstArena::new();
            let module = Parser::new("\"\"\"hello\nworld\"\"\"", &arena)
                .parse_module()
                .unwrap();
            let Statement::Expr(e) = module.body[0] else {
                panic!("expected Expr statement");
            };
            let Expression::String(s) = e.value else {
                panic!("expected String expression");
            };
            assert_eq!(s.value, "hello\nworld");
        }

        #[test]
        fn byte() {
            let arena = AstArena::new();
            let module = Parser::new("b'data'", &arena).parse_module().unwrap();
            let Statement::Expr(e) = module.body[0] else {
                panic!("expected Expr statement");
            };
            let Expression::String(s) = e.value else {
                panic!("expected String expression");
            };
            assert_eq!(s.value, "data");
        }
    }
}
