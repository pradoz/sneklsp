use crate::{Token, TokenKind};
use sneklsp_text::{TextRange, TextSize};

struct FStringState {
    quote: u8,
    triple: bool,
    brace_depth: u32,
}

pub struct Lexer<'src> {
    source: &'src str,
    bytes: &'src [u8],
    position: usize,
    indent_stack: Vec<usize>,
    pending_tokens: Vec<Token>,
    at_line_start: bool,
    done: bool,
    bracket_depth: u32,
    fstring_stack: Vec<FStringState>,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            position: 0,
            indent_stack: {
                let mut v = Vec::with_capacity(16);
                v.push(0);
                v
            },
            pending_tokens: Vec::with_capacity(8),
            at_line_start: true,
            done: false,
            bracket_depth: 0,
            fstring_stack: Vec::new(),
        }
    }

    #[inline]
    pub fn next_token(&mut self) -> Token {
        if let Some(token) = self.pending_tokens.pop() {
            return token;
        }

        if let Some(fstate) = self.fstring_stack.last() {
            if fstate.brace_depth == 0 {
                return self.scan_fstring_content();
            }
        }

        if self.at_line_start && !self.is_at_end() {
            self.at_line_start = false;
            if let Some(token) = self.handle_indentation() {
                return token;
            }
        }

        self.skip_whitespace();
        if let Some(comment) = self.scan_comment() {
            return comment;
        }

        if self.is_at_end() {
            if !self.done {
                self.done = true;

                while self.indent_stack.len() > 1 {
                    self.indent_stack.pop();
                    self.pending_tokens.push(self.make_token(
                        TokenKind::Dedent,
                        self.position,
                        self.position,
                    ));
                }
                if let Some(token) = self.pending_tokens.pop() {
                    return token;
                }
            }
            return self.make_token(TokenKind::Eof, self.position, self.position);
        }

        let start = self.position;
        let byte = self.advance();

        let kind = match byte {
            b'(' => {
                self.bracket_depth += 1;
                TokenKind::LParen
            }
            b')' => {
                self.decrement_bracket_depth();
                TokenKind::RParen
            }
            b'[' => {
                self.bracket_depth += 1;
                TokenKind::LBracket
            }
            b']' => {
                self.decrement_bracket_depth();
                TokenKind::RBracket
            }
            b'{' => {
                if let Some(fstate) = self.fstring_stack.last_mut() {
                    fstate.brace_depth += 1;
                }
                self.bracket_depth += 1;
                TokenKind::LBrace
            }
            b'}' => {
                if let Some(fstate) = self.fstring_stack.last_mut() {
                    if fstate.brace_depth > 0 {
                        fstate.brace_depth -= 1;
                        if fstate.brace_depth == 0 {
                            self.decrement_bracket_depth();
                            return self.make_token(TokenKind::RBrace, start, self.position);
                        }
                    }
                }
                self.decrement_bracket_depth();
                TokenKind::RBrace
            }
            b':' => {
                if self.match_byte(b'=') {
                    TokenKind::ColonEq
                } else {
                    TokenKind::Colon
                }
            }
            b',' => TokenKind::Comma,
            b';' => TokenKind::Semi,
            b'@' => TokenKind::At,
            b'~' => TokenKind::Tilde,

            b'.' => {
                if self.peek().is_ascii_digit() {
                    self.scan_number(start)
                } else if self.peek() == b'.' && self.peek_next() == b'.' {
                    self.advance(); // second .
                    self.advance(); // third .
                    TokenKind::Ellipsis
                } else {
                    TokenKind::Dot
                }
            }

            b'+' => {
                if self.match_byte(b'=') {
                    TokenKind::PlusEq
                } else {
                    TokenKind::Plus
                }
            }

            b'-' => {
                if self.match_byte(b'>') {
                    TokenKind::Arrow
                } else if self.match_byte(b'=') {
                    TokenKind::MinusEq
                } else {
                    TokenKind::Minus
                }
            }

            b'*' => {
                if self.match_byte(b'*') {
                    TokenKind::DoubleStar
                } else if self.match_byte(b'=') {
                    TokenKind::StarEq
                } else {
                    TokenKind::Star
                }
            }

            b'/' => {
                if self.match_byte(b'/') {
                    TokenKind::DoubleSlash
                } else if self.match_byte(b'=') {
                    TokenKind::SlashEq
                } else {
                    TokenKind::Slash
                }
            }

            b'%' => {
                if self.match_byte(b'=') {
                    TokenKind::PercentEq
                } else {
                    TokenKind::Percent
                }
            }

            b'&' => {
                if self.match_byte(b'=') {
                    TokenKind::AmpEq
                } else {
                    TokenKind::Amp
                }
            }

            b'|' => {
                if self.match_byte(b'=') {
                    TokenKind::PipeEq
                } else {
                    TokenKind::Pipe
                }
            }

            b'^' => {
                if self.match_byte(b'=') {
                    TokenKind::CaretEq
                } else {
                    TokenKind::Caret
                }
            }

            b'<' => {
                if self.match_byte(b'<') {
                    TokenKind::LtLt
                } else if self.match_byte(b'=') {
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }

            b'>' => {
                if self.match_byte(b'>') {
                    TokenKind::GtGt
                } else if self.match_byte(b'=') {
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }

            b'=' => {
                if self.match_byte(b'=') {
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }

            b'!' => {
                if self.match_byte(b'=') {
                    TokenKind::NotEq
                } else {
                    TokenKind::Error
                }
            }

            b'\n' => {
                // implicit line continuation
                if self.bracket_depth > 0 {
                    self.skip_whitespace();
                    return self.next_token();
                }
                self.at_line_start = true;
                TokenKind::Newline
            }

            b'"' | b'\'' => self.scan_string(byte),
            b'0'..=b'9' => self.scan_number(start),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.scan_identifier(start),

            _ => TokenKind::Error,
        };

        self.make_token(kind, start, self.position)
    }

    fn scan_fstring_content(&mut self) -> Token {
        let fstate = self.fstring_stack.last().unwrap();
        let quote = fstate.quote;
        let triple = fstate.triple;

        // check for end of f-string
        if self.is_at_end() {
            self.fstring_stack.pop();
            return self.make_token(TokenKind::Error, self.position, self.position);
        }

        // check for closing quote
        if self.peek() == quote {
            if triple {
                if self.position + 2 < self.bytes.len()
                    && self.bytes[self.position + 1] == quote
                    && self.bytes[self.position + 2] == quote
                {
                    let start = self.position;
                    self.position += 3;
                    self.fstring_stack.pop();
                    return self.make_token(TokenKind::FStringEnd, start, self.position);
                }
            } else {
                let start = self.position;
                self.position += 1;
                self.fstring_stack.pop();
                return self.make_token(TokenKind::FStringEnd, start, self.position);
            }
        }

        // check for expression hole
        if self.peek() == b'{' {
            // `{{` is an escaped brace, not an expression hole
            if self.peek_next() == b'{' {
                // scan as literal including the escaped brace
                return self.scan_fstring_literal(quote, triple);
            }
            let start = self.position;
            self.advance();
            self.fstring_stack.last_mut().unwrap().brace_depth = 1;
            self.bracket_depth += 1;
            return self.make_token(TokenKind::LBrace, start, self.position);
        }

        self.scan_fstring_literal(quote, triple)
    }

    fn scan_fstring_literal(&mut self, quote: u8, triple: bool) -> Token {
        let start = self.position;

        loop {
            if self.is_at_end() {
                break;
            }

            let byte = self.peek();

            // escaped brace `{{` or `}}`
            if byte == b'{' && self.peek_next() == b'{' {
                self.position += 2;
                continue;
            }
            if byte == b'}' && self.peek_next() == b'}' {
                self.position += 2;
                continue;
            }

            // expression hole start
            if byte == b'{' {
                break;
            }

            // closing quote
            if byte == quote {
                if triple {
                    if self.position + 2 < self.bytes.len()
                        && self.bytes[self.position + 1] == quote
                        && self.bytes[self.position + 2] == quote
                    {
                        break;
                    }
                } else {
                    break;
                }
            }

            if byte == b'\\' {
                self.advance();
                if !self.is_at_end() {
                    self.advance();
                }
                continue;
            }

            if byte == b'\n' && !triple {
                break;
            }

            self.advance();
        }

        // only emit if we consumed something
        if self.position > start {
            self.make_token(TokenKind::FStringLiteral, start, self.position)
        } else {
            self.scan_fstring_content()
        }
    }

    fn enter_fstring(&mut self, quote: u8) -> TokenKind {
        let triple = self.peek() == quote && self.peek_next() == quote;
        if triple {
            self.advance();
            self.advance();
        }

        self.fstring_stack.push(FStringState {
            quote,
            triple,
            brace_depth: 0,
        });

        TokenKind::FStringStart
    }

    pub fn decrement_bracket_depth(&mut self) {
        self.bracket_depth = self.bracket_depth.saturating_sub(1);
    }

    fn handle_indentation(&mut self) -> Option<Token> {
        if self.bracket_depth > 0 {
            return None;
        }

        loop {
            let mut indent = 0;
            let indent_start = self.position;

            while !self.is_at_end() {
                match self.peek() {
                    b' ' => {
                        indent += 1;
                        self.advance();
                    }
                    b'\t' => {
                        indent += 8 - (indent % 8);
                        self.advance();
                    }
                    _ => break,
                }
            }

            match self.peek() {
                b'\n' => {
                    self.advance();
                    continue;
                }
                b'#' => {
                    let comment = self.scan_comment().unwrap();
                    self.pending_tokens.push(comment);
                    if !self.is_at_end() && self.peek() == b'\n' {
                        self.advance();
                        continue;
                    }
                    if self.is_at_end() {
                        return self.pending_tokens.pop();
                    }
                }
                _ => {}
            }

            if self.is_at_end() {
                return None;
            }

            let current_indent = *self.indent_stack.last().unwrap();

            if indent > current_indent {
                self.indent_stack.push(indent);
                return Some(self.make_token(TokenKind::Indent, indent_start, self.position));
            }

            if indent < current_indent {
                while let Some(&top) = self.indent_stack.last() {
                    if top <= indent {
                        break;
                    }
                    self.indent_stack.pop();
                    self.pending_tokens.push(self.make_token(
                        TokenKind::Dedent,
                        indent_start,
                        self.position,
                    ));
                }
                return self.pending_tokens.pop();
            }

            return None;
        }
    }

    #[inline]
    fn scan_identifier(&mut self, start: usize) -> TokenKind {
        while !self.is_at_end() {
            match self.peek() {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' => {
                    self.advance();
                }
                _ => break,
            }
        }

        let text = &self.source[start..self.position];

        if self.peek() == b'"' || self.peek() == b'\'' {
            let lower = text.to_lowercase();
            let is_fstring = matches!(lower.as_str(), "f" | "fr" | "rf");
            if matches!(
                lower.as_str(),
                "f" | "r" | "b" | "fr" | "rf" | "br" | "rb" | "u"
            ) {
                let quote = self.advance();
                if is_fstring {
                    return self.enter_fstring(quote);
                }
                return self.scan_string(quote);
            }
        }

        TokenKind::from_keyword(text).unwrap_or(TokenKind::Name)
    }

    #[inline]
    fn scan_number(&mut self, _start: usize) -> TokenKind {
        self.skip_digits(); // scan integer

        let mut is_float = false;

        // check if float
        if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            self.advance(); // '.'
            self.skip_digits();
            is_float = true;
        }

        // check if exponent
        if self.peek() == b'e' || self.peek() == b'E' {
            self.advance();
            if self.peek() == b'+' || self.peek() == b'-' {
                self.advance();
            }
            self.skip_digits();
            is_float = true;
        }

        if is_float {
            TokenKind::Float
        } else {
            TokenKind::Int
        }
    }

    #[inline]
    fn skip_digits(&mut self) {
        while !self.is_at_end() && self.peek().is_ascii_digit() {
            self.advance();
        }
    }

    fn scan_string(&mut self, quote: u8) -> TokenKind {
        // docstring
        let triple = self.peek() == quote && self.peek_next() == quote;
        if triple {
            self.advance();
            self.advance();
        }

        loop {
            if self.is_at_end() {
                return TokenKind::Error;
            }

            let byte = self.peek();
            if byte == b'\\' {
                // escape sequence
                self.advance();
                if !self.is_at_end() {
                    self.advance();
                }
                continue;
            }

            if byte == quote {
                self.advance();
                if triple {
                    if self.peek() == quote && self.peek_next() == quote {
                        self.advance();
                        self.advance();
                        break;
                    }
                } else {
                    break;
                }
                continue;
            }

            if byte == b'\n' && !triple {
                return TokenKind::Error;
            }

            self.advance();
        }

        TokenKind::String
    }

    #[inline]
    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                b' ' | b'\t' | b'\r' => {
                    self.advance();
                }
                // line continuation
                b'\\' if self.peek_next() == b'\n' => {
                    self.advance();
                    self.advance();
                }
                _ => break,
            }
        }
    }

    #[inline]
    fn scan_comment(&mut self) -> Option<Token> {
        if !self.is_at_end() && self.peek() == b'#' {
            let start = self.position;
            while !self.is_at_end() && self.peek() != b'\n' {
                self.advance();
            }
            Some(self.make_token(TokenKind::Comment, start, self.position))
        } else {
            None
        }
    }

    #[inline]
    fn make_token(&self, kind: TokenKind, start: usize, end: usize) -> Token {
        Token::new(
            kind,
            TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32)),
        )
    }

    #[inline]
    fn is_at_end(&self) -> bool {
        self.position >= self.bytes.len()
    }

    #[inline]
    fn peek(&self) -> u8 {
        self.bytes.get(self.position).copied().unwrap_or(0)
    }

    #[inline]
    fn peek_next(&self) -> u8 {
        self.bytes.get(self.position + 1).copied().unwrap_or(0)
    }

    #[inline]
    fn advance(&mut self) -> u8 {
        let byte = self.peek();
        self.position += 1;
        byte
    }

    #[inline]
    fn match_byte(&mut self, expected: u8) -> bool {
        if self.peek() == expected {
            self.advance();
            true
        } else {
            false
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token();
        if token.kind == TokenKind::Eof {
            None
        } else {
            Some(token)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(source: &str) -> Vec<TokenKind> {
        Lexer::new(source).map(|t| t.kind).collect()
    }

    #[test]
    fn operators() {
        assert_eq!(
            lex("+ - * ** / // %"),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::DoubleStar,
                TokenKind::Slash,
                TokenKind::DoubleSlash,
                TokenKind::Percent,
            ]
        );
    }

    #[test]
    fn comparisons() {
        assert_eq!(
            lex("== != < <= > >="),
            vec![
                TokenKind::EqEq,
                TokenKind::NotEq,
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
            ]
        );
    }

    #[test]
    fn string_literals() {
        assert_eq!(lex("\"hello\""), vec![TokenKind::String]);
        assert_eq!(lex("'hello'"), vec![TokenKind::String]);
        assert_eq!(lex("\"\"\"hello\"\"\""), vec![TokenKind::String]);
    }

    #[test]
    fn numbers() {
        assert_eq!(lex("42"), vec![TokenKind::Int]);
        assert_eq!(lex("6.9"), vec![TokenKind::Float]);
        assert_eq!(lex("4e20"), vec![TokenKind::Float]);
        assert_eq!(lex("3.1e-4"), vec![TokenKind::Float]);
    }

    #[test]
    fn indentation() {
        let tokens = lex("if x:\n    y");
        assert!(tokens.contains(&TokenKind::Indent));
    }

    #[test]
    fn dedentation() {
        let tokens = lex("if x:\n    y\nz");
        assert!(tokens.contains(&TokenKind::Dedent));
    }

    #[test]
    fn keywords() {
        assert_eq!(
            lex("def if else return"),
            vec![
                TokenKind::Def,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::Return,
            ]
        );
    }

    #[test]
    fn implicit_line_continuation_parens() {
        let tokens = lex("x = (1 +\n    2)");
        // Should NOT contain Newline or Indent inside parens
        assert!(!tokens.contains(&TokenKind::Newline));
        assert!(!tokens.contains(&TokenKind::Indent));
    }

    #[test]
    fn implicit_line_continuation_brackets() {
        let tokens = lex("x = [1,\n    2,\n    3]");
        assert!(!tokens.contains(&TokenKind::Newline));
        assert!(!tokens.contains(&TokenKind::Indent));
    }

    #[test]
    fn implicit_line_continuation_braces() {
        let tokens = lex("x = {1: 2,\n    3: 4}");
        assert!(!tokens.contains(&TokenKind::Newline));
        assert!(!tokens.contains(&TokenKind::Indent));
    }

    #[test]
    fn multi_line_import() {
        let tokens = lex("from foo import (\n    bar,\n    baz\n    baz,\n)");
        assert!(!tokens.iter().any(|k| *k == TokenKind::Indent));
    }

    #[test]
    fn multi_line_function_call() {
        let tokens = lex("x = foo(\n    1,\n    2,\n)");
        assert!(!tokens.iter().any(|k| *k == TokenKind::Indent));
    }

    #[test]
    fn newline_outside_brackets_still_works() {
        let tokens = lex("x = 1\ny = 2");
        assert!(tokens.contains(&TokenKind::Newline));
    }

    #[test]
    fn comment_tokens() {
        let tokens = lex("x = 1  # comment");
        assert!(tokens.contains(&TokenKind::Comment));

        let tokens = lex("if x:\n    # comment\n    y");
        assert!(tokens.contains(&TokenKind::Comment));
        assert!(tokens.contains(&TokenKind::Indent));
    }

    mod fstring {
        use super::*;

        #[test]
        fn empty() {
            let kinds = lex("f''");
            assert_eq!(kinds, vec![TokenKind::FStringStart, TokenKind::FStringEnd]);
        }

        #[test]
        fn simple() {
            let kinds = lex("f'hello {x}'");
            assert_eq!(
                kinds,
                vec![
                    TokenKind::FStringStart,
                    TokenKind::FStringLiteral, // "hello "
                    TokenKind::LBrace,
                    TokenKind::Name, // x
                    TokenKind::RBrace,
                    TokenKind::FStringEnd,
                ]
            );
        }

        #[test]
        fn no_expr() {
            let kinds = lex("f'hello world'");
            assert_eq!(
                kinds,
                vec![
                    TokenKind::FStringStart,
                    TokenKind::FStringLiteral,
                    TokenKind::FStringEnd,
                ]
            );
        }

        #[test]
        fn multiple_exprs() {
            let kinds = lex("f'{a} and {b}'");
            assert_eq!(
                kinds,
                vec![
                    TokenKind::FStringStart,
                    TokenKind::LBrace,
                    TokenKind::Name, // a
                    TokenKind::RBrace,
                    TokenKind::FStringLiteral, // " and "
                    TokenKind::LBrace,
                    TokenKind::Name, // b
                    TokenKind::RBrace,
                    TokenKind::FStringEnd,
                ]
            );
        }

        #[test]
        fn escaped_braces() {
            let kinds = lex("f'{{literal}}'");
            assert_eq!(
                kinds,
                vec![
                    TokenKind::FStringStart,
                    TokenKind::FStringLiteral, // "{{literal}}"
                    TokenKind::FStringEnd,
                ]
            );
        }

        #[test]
        fn expression_with_call() {
            let kinds = lex("f'{foo(1, 2)}'");
            assert_eq!(
                kinds,
                vec![
                    TokenKind::FStringStart,
                    TokenKind::LBrace,
                    TokenKind::Name, // foo
                    TokenKind::LParen,
                    TokenKind::Int, // 1
                    TokenKind::Comma,
                    TokenKind::Int, // 2
                    TokenKind::RParen,
                    TokenKind::RBrace,
                    TokenKind::FStringEnd,
                ]
            );
        }

        #[test]
        fn nested() {
            let kinds = lex("f'{f\"{x}\"}'");
            assert_eq!(
                kinds,
                vec![
                    TokenKind::FStringStart, // f'
                    TokenKind::LBrace,
                    TokenKind::FStringStart, // f"
                    TokenKind::LBrace,
                    TokenKind::Name, // x
                    TokenKind::RBrace,
                    TokenKind::FStringEnd, // "
                    TokenKind::RBrace,
                    TokenKind::FStringEnd, // '
                ]
            );
        }

        #[test]
        fn with_dict() {
            let kinds = lex("f'{d[\"key\"]}'");
            assert_eq!(
                kinds,
                vec![
                    TokenKind::FStringStart,
                    TokenKind::LBrace,
                    TokenKind::Name, // d
                    TokenKind::LBracket,
                    TokenKind::String, // "key"
                    TokenKind::RBracket,
                    TokenKind::RBrace,
                    TokenKind::FStringEnd,
                ]
            );
        }
    }
}
