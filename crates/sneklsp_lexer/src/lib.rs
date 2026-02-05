mod edit;
mod incremental;
mod lexer;
mod token;

pub use edit::TextEdit;
pub use incremental::{IncrementalLexResult, relex};
pub use lexer::Lexer;
pub use token::{Token, TokenKind};

pub fn tokenize(source: &str) -> Vec<Token> {
    // estimate ~10 chars per token on average
    let estimated_tokens = source.len() / 10;
    let mut tokens = Vec::with_capacity(estimated_tokens);
    tokens.extend(Lexer::new(source));
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple() {
        let tokens = tokenize("x = 1");
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::Name);
        assert_eq!(tokens[1].kind, TokenKind::Eq);
        assert_eq!(tokens[2].kind, TokenKind::Int);
        // TODO: decide if tokenize consumers need EOF
        // assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn tokenize_function() {
        let tokens = tokenize("def foo():");
        let kinds: Vec<_> = tokens.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Def,
                TokenKind::Name,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::Colon,
                // TODO: decide if tokenize consumers need EOF
                // TokenKind::Eof,
            ]
        );
    }

    mod snapshot {
        use super::*;

        fn tokenize_to_string(source: &str) -> String {
            let tokens = tokenize(source);
            tokens
                .iter()
                .map(|t| {
                    let start = t.range.start().to_usize();
                    let end = t.range.end().to_usize();
                    let text = &source[start..end];
                    format!("{:?} {:?}", t.kind, text)
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        #[test]
        fn tokenize_simple_expressions() {
            let source = include_str!("../../../testdata/simple/expressions.py");
            insta::assert_snapshot!(tokenize_to_string(source));
        }

        #[test]
        fn tokenize_simple_functions() {
            let source = include_str!("../../../testdata/simple/functions.py");
            insta::assert_snapshot!(tokenize_to_string(source));
        }
    }
}
