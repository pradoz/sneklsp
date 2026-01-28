mod parser;

use sneklsp_ast::Module;
use sneklsp_text::TextSize;
use thiserror::Error;

pub use parser::Parser;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token at offset {offset}: expected {expected}, found {found}")]
    UnexpectedToken {
        offset: TextSize,
        expected: String,
        found: String,
    },

    #[error("unexpected end of file")]
    UnexpectedEof,

    #[error("invalid syntax at offset {0}")]
    InvalidSyntax(TextSize),
}

pub type ParseResult<T> = Result<T, ParseError>;

pub fn parse(source: &str) -> ParseResult<Module> {
    Parser::new(source).parse_module()
}

pub fn parse_with_errors(source: &str) -> (Option<Module>, Vec<ParseError>) {
    match parse(source) {
        Ok(module) => (Some(module), vec![]),
        Err(e) => (None, vec![e]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_assignment() {
        let module = parse("x = 1").unwrap();
        assert_eq!(module.body.len(), 1);
    }

    #[test]
    fn parse_function() {
        let module = parse("def foo():\n    pass").unwrap();
        assert_eq!(module.body.len(), 1);
    }

    #[test]
    fn parse_expression() {
        let module = parse("1 + 2 * 3").unwrap();
        assert_eq!(module.body.len(), 1);
    }
}
