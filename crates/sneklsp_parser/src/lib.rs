mod parser;

use sneklsp_ast::{AstArena, Module};
use sneklsp_text::{TextRange, TextSize};
use thiserror::Error;

pub use parser::Parser;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected token at offset {range:?}: expected {expected}, found {found}")]
    UnexpectedToken {
        range: TextRange,
        expected: String,
        found: String,
    },

    #[error("unexpected end of file")]
    UnexpectedEof,

    #[error("invalid syntax at offset {0}")]
    InvalidSyntax(TextSize),
}

pub type ParseResult<T> = Result<T, ParseError>;

pub struct ParseOutput<'ast> {
    pub module: Module<'ast>,
    pub errors: Vec<ParseError>,
}

#[inline]
pub fn parse<'ast>(source: &str, arena: &'ast AstArena) -> ParseResult<Module<'ast>> {
    Parser::new(source, arena).parse_module()
}

#[inline]
pub fn parse_recovering<'ast>(source: &str, arena: &'ast AstArena) -> ParseOutput<'ast> {
    let mut parser = Parser::new(source, arena).with_recovery();
    let module = parser.parse_module_recovering();
    let errors = parser.take_errors();
    ParseOutput { module, errors }
}

pub fn parse_and_collect_errors(source: &str) -> Vec<ParseError> {
    // estimate ~50 bytes of arena per byte of source for python
    let arena_size = source.len() * 50;
    let arena = AstArena::with_capacity(arena_size.max(4096));
    parse_recovering(source, &arena).errors
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parse {
        use super::*;

        #[test]
        fn simple_assignment() {
            let arena = AstArena::new();
            let module = parse("x = 1", &arena).unwrap();
            assert_eq!(module.body.len(), 1);
        }

        #[test]
        fn function() {
            let arena = AstArena::new();
            let module = parse("def foo():\n    pass", &arena).unwrap();
            assert_eq!(module.body.len(), 1);
        }

        #[test]
        fn expression() {
            let arena = AstArena::new();
            let module = parse("1 + 2 * 3", &arena).unwrap();
            assert_eq!(module.body.len(), 1);
        }
    }

    mod collect_errors {
        use super::*;

        #[test]
        fn on_valid_syntax() {
            let errors = parse_and_collect_errors("x = 1");
            assert!(errors.is_empty());
        }

        #[test]
        fn on_invalid_syntax() {
            let errors = parse_and_collect_errors("def foo(");
            assert!(!errors.is_empty());
        }

        #[test]
        fn multiple_errors() {
            let source = "x = 1 +\ny = 2 +\nz = 3";
            let errors = parse_and_collect_errors(source);
            assert!(!errors.is_empty());
            assert!(
                errors.len() >= 2,
                "expected multiple errors, got {}",
                errors.len()
            );
        }
    }

    mod recovery {
        use super::*;

        #[test]
        fn valid_source_no_errors() {
            let arena = AstArena::new();
            let source = "x = 1\ny = 2";
            let output = parse_recovering(source, &arena);
            assert_eq!(output.module.body.len(), 2);
            assert!(output.errors.is_empty());
        }

        #[test]
        fn recovers_partial_module() {
            let arena = AstArena::new();
            let source = "x = 1\ndef foo(\nz = 3";
            let output = parse_recovering(source, &arena);
            assert!(!output.module.body.is_empty());
            assert!(!output.errors.is_empty());
        }
    }

    mod snapshot {
        use super::*;

        #[test]
        fn parse_simple_expressions() {
            let arena = AstArena::new();
            let source = include_str!("../../../testdata/simple/expressions.py");
            let result = parse(source, &arena);
            insta::assert_debug_snapshot!(result);
        }

        #[test]
        fn parse_simple_functions() {
            let arena = AstArena::new();
            let source = include_str!("../../../testdata/simple/functions.py");
            let result = parse(source, &arena);
            insta::assert_debug_snapshot!(result);
        }
    }
}
