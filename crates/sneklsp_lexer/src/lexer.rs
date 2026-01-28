use crate::{Token, TokenKind};
use sneklsp_text::{TextRange, TextSize};

pub struct Lexer<'src> {
    source: &'src str,
    bytes: &'src [u8],
    position: usize,
    indent_stack: Vec<usize>,
    pending_tokens: Vec<Token>,
    at_line_start: bool,
    done: bool,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            position: 0,
            indent_stack: vec![0],
            pending_tokens: Vec::new(),
            at_line_start: true,
            done: false,
        }
    }

    pub fn next_token(&mut self) -> Token {
        if let Some(token) = self.pending_tokens.pop() {
            return token;
        }

        if self.at_line_start && !self.is_at_end() {
            self.at_line_start = false;
            if let Some(token) = self.handle_indentation() {
                return token;
            }
        }

        self.skip_whitespace();
        self.skip_comment();

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
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b':' => TokenKind::Colon,
            b',' => TokenKind::Comma,
            b';' => TokenKind::Semi,
            b'@' => TokenKind::At,
            b'~' => TokenKind::Tilde,

            b'.' => {
                if self.peek().is_ascii_digit() {
                    self.scan_number(start)
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

    fn handle_indentation(&mut self) -> Option<Token> {
        let mut indent = 0;
        let start = self.position;

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
                b'\n' => {
                    self.advance();
                    self.at_line_start = true;
                    indent = 0;
                }
                b'#' => {
                    self.skip_comment();
                    if !self.is_at_end() && self.peek() == b'\n' {
                        self.advance();
                        self.at_line_start = true;
                        indent = 0;
                    }
                }
                _ => break,
            }
        }

        if self.is_at_end() {
            return None;
        }

        let current_indent = *self.indent_stack.last().unwrap();

        if indent > current_indent {
            self.indent_stack.push(indent);
            return Some(self.make_token(TokenKind::Indent, start, self.position));
        }

        if indent < current_indent {
            while let Some(&top) = self.indent_stack.last() {
                if top <= indent {
                    break;
                }
                self.indent_stack.pop();
                self.pending_tokens
                    .push(self.make_token(TokenKind::Dedent, start, self.position));
            }
            return self.pending_tokens.pop();
        }

        None
    }

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
        TokenKind::from_keyword(text).unwrap_or(TokenKind::Name)
    }

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

    fn skip_comment(&mut self) {
        if !self.is_at_end() && self.peek() == b'#' {
            while !self.is_at_end() && self.peek() != b'\n' {
                self.advance();
            }
        }
    }

    fn make_token(&self, kind: TokenKind, start: usize, end: usize) -> Token {
        Token::new(
            kind,
            TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32)),
        )
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.bytes.len()
    }

    fn peek(&self) -> u8 {
        self.bytes.get(self.position).copied().unwrap_or(0)
    }

    fn peek_next(&self) -> u8 {
        self.bytes.get(self.position + 1).copied().unwrap_or(0)
    }

    fn advance(&mut self) -> u8 {
        let byte = self.peek();
        self.position += 1;
        byte
    }

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
}
