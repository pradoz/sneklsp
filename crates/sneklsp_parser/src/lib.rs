mod parser;

use sneklsp_ast::{AstArena, Module};
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

pub fn parse<'ast>(source: &str, arena: &'ast AstArena) -> ParseResult<Module<'ast>> {
    Parser::new(source, arena).parse_module()
}

pub fn parse_and_collect_errors(source: &str) -> Vec<ParseError> {
    let arena = AstArena::new();
    let mut parser = Parser::new(source, &arena).parse_module();
    parser.parse_module_collecting_errors()
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

        #[test]
        fn parse_simple_hello() {
            let arena = AstArena::new();
            let source = include_str!("../../../testdata/simple/hello.py");
            let result = parse(source, &arena);
            insta::assert_debug_snapshot!(result);
        }
    }
}
