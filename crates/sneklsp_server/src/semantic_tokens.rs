use lsp_types::{
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens, SemanticTokensLegend,
    SemanticTokensResult,
};

use sneklsp_index::OwnedIndex;
use sneklsp_lexer::{Token, TokenKind};
use sneklsp_text::{LineIndex, TextRange, TextSize};

pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::NAMESPACE,
    SemanticTokenType::TYPE,
    SemanticTokenType::CLASS,
    SemanticTokenType::FUNCTION,
    SemanticTokenType::METHOD,
    SemanticTokenType::PROPERTY,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::PARAMETER,
    SemanticTokenType::DECORATOR,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::STRING,
    SemanticTokenType::NUMBER,
    SemanticTokenType::COMMENT,
    SemanticTokenType::new("builtin"),
    SemanticTokenType::new("selfKeyword"),
];

const TT_NAMESPACE: u32 = 0;
const TT_CLASS: u32 = 2;
const TT_FUNCTION: u32 = 3;
const TT_METHOD: u32 = 4;
const TT_PROPERTY: u32 = 5;
const TT_VARIABLE: u32 = 6;
const TT_PARAMETER: u32 = 7;
const TT_DECORATOR: u32 = 8;
const TT_KEYWORD: u32 = 9;
const TT_STRING: u32 = 10;
const TT_NUMBER: u32 = 11;
const TT_COMMENT: u32 = 12;
const TT_OPERATOR: u32 = 13;
const TT_BUILTIN: u32 = 14;
const TT_SELF: u32 = 15;

pub const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::DEFINITION, // bit 0
    SemanticTokenModifier::READONLY,   // bit 1
    SemanticTokenModifier::ASYNC,      // bit 2
];

const MOD_DEFINITION: u32 = 1 << 0;
const MOD_READONLY: u32 = 1 << 1;

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: TOKEN_MODIFIERS.to_vec(),
    }
}

#[derive(Clone, Copy)]
struct RawToken {
    line: u32,
    col: u32,
    len: u32,
    token_type: u32,
    modifiers: u32,
}

pub fn compute_semantic_tokens(index: &OwnedIndex, line_index: &LineIndex) -> SemanticTokensResult {
    let source = index.source();
    let tokens = sneklsp_lexer::tokenize(source);
    let raw = collect_raw_tokens(&tokens, Some(index), line_index, source, None);
    SemanticTokensResult::Tokens(encode(raw))
}

pub fn compute_semantic_tokens_range(
    index: &OwnedIndex,
    line_index: &LineIndex,
    range: lsp_types::Range,
) -> SemanticTokensResult {
    let source = index.source();
    let tokens = sneklsp_lexer::tokenize(source);
    let raw = collect_raw_tokens(&tokens, Some(index), line_index, source, Some(range));
    SemanticTokensResult::Tokens(encode(raw))
}

fn collect_raw_tokens(
    tokens: &[Token],
    index: Option<&OwnedIndex>,
    line_index: &LineIndex,
    source: &str,
    range_filter: Option<lsp_types::Range>,
) -> Vec<RawToken> {
    let capacity = tokens.len();
    let mut raw = Vec::with_capacity(capacity);

    collect_lexer_tokens(tokens, line_index, source, range_filter, &mut raw);

    if let Some(index) = index {
        collect_semantic_tokens(index, line_index, range_filter, &mut raw);
    }

    raw.sort_unstable_by(|a, b| a.line.cmp(&b.line).then(a.col.cmp(&b.col)));
    dedup_overlapping(&mut raw);

    raw
}

fn collect_lexer_tokens(
    tokens: &[Token],
    line_index: &LineIndex,
    source: &str,
    range_filter: Option<lsp_types::Range>,
    raw: &mut Vec<RawToken>,
) {
    for token in tokens {
        let Some(token_type) = lexer_token_type(token.kind) else {
            continue;
        };

        let pos = line_index.position(token.range.start());
        let len = token.range.len().to_u32();

        if len == 0 {
            continue;
        }

        if let Some(ref filter) = range_filter {
            if pos.line < filter.start.line || pos.line > filter.end.line {
                continue;
            }
        }

        let token_type = if token_type == TT_KEYWORD {
            let text = token_text(source, token.range);
            if text == "self" || text == "cls" {
                TT_SELF
            } else {
                TT_KEYWORD
            }
        } else {
            token_type
        };

        raw.push(RawToken {
            line: pos.line,
            col: pos.column,
            len,
            token_type,
            modifiers: 0,
        });
    }
    collect_comments(source, line_index, range_filter, raw);
}

fn lexer_token_type(kind: TokenKind) -> Option<u32> {
    match kind {
        TokenKind::And
        | TokenKind::As
        | TokenKind::Assert
        | TokenKind::Async
        | TokenKind::Await
        | TokenKind::Break
        | TokenKind::Class
        | TokenKind::Continue
        | TokenKind::Def
        | TokenKind::Del
        | TokenKind::Elif
        | TokenKind::Else
        | TokenKind::Except
        | TokenKind::Finally
        | TokenKind::For
        | TokenKind::From
        | TokenKind::Global
        | TokenKind::If
        | TokenKind::Import
        | TokenKind::In
        | TokenKind::Is
        | TokenKind::Lambda
        | TokenKind::Nonlocal
        | TokenKind::Not
        | TokenKind::Or
        | TokenKind::Pass
        | TokenKind::Raise
        | TokenKind::Return
        | TokenKind::Try
        | TokenKind::While
        | TokenKind::With
        | TokenKind::Yield => Some(TT_KEYWORD),

        TokenKind::True | TokenKind::False | TokenKind::None => Some(TT_KEYWORD),
        TokenKind::Int | TokenKind::Float => Some(TT_NUMBER),
        TokenKind::String | TokenKind::FString => Some(TT_STRING),

        TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::DoubleSlash
        | TokenKind::Percent
        | TokenKind::DoubleStar
        | TokenKind::Amp
        | TokenKind::Pipe
        | TokenKind::Caret
        | TokenKind::Tilde
        | TokenKind::LtLt
        | TokenKind::GtGt
        | TokenKind::EqEq
        | TokenKind::NotEq
        | TokenKind::Lt
        | TokenKind::LtEq
        | TokenKind::Gt
        | TokenKind::GtEq => Some(TT_OPERATOR),

        TokenKind::At => Some(TT_DECORATOR),

        TokenKind::Name => None,

        _ => None,
    }
}

fn collect_comments(
    source: &str,
    line_index: &LineIndex,
    range_filter: Option<lsp_types::Range>,
    raw: &mut Vec<RawToken>,
) {
    for (byte_offset, _) in source.match_indices('#') {
        let offset = TextSize::new(byte_offset as u32);
        let pos = line_index.position(offset);

        if let Some(ref filter) = range_filter {
            if pos.line < filter.start.line || pos.line > filter.end.line {
                continue;
            }
        }

        let line_end = source[byte_offset..]
            .find('\n')
            .map(|p| byte_offset + p)
            .unwrap_or(source.len());
        let len = (line_end - byte_offset) as u32;

        if len == 0 {
            continue;
        }

        if likely_inside_string(source, byte_offset) {
            continue;
        }

        raw.push(RawToken {
            line: pos.line,
            col: pos.column,
            len,
            token_type: TT_COMMENT,
            modifiers: 0,
        });
    }
}

fn likely_inside_string(source: &str, offset: usize) -> bool {
    let line_start = source[..offset].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let before = &source[line_start..offset];

    let mut single_quotes = 0u32;
    let mut double_quotes = 0u32;
    let bytes = before.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        match bytes[i] {
            b'\'' => single_quotes += 1,
            b'"' => double_quotes += 1,
            _ => {}
        }
        i += 1;
    }

    single_quotes % 2 != 0 || double_quotes % 2 != 0
}

fn collect_semantic_tokens(
    index: &OwnedIndex,
    line_index: &LineIndex,
    range_filter: Option<lsp_types::Range>,
    raw: &mut Vec<RawToken>,
) {
    for symbol in index.symbols() {
        let pos = line_index.position(symbol.selection_range.start());
        let len = symbol.selection_range.len().to_u32();

        if len == 0 {
            continue;
        }

        if let Some(ref filter) = range_filter {
            if pos.line < filter.start.line || pos.line > filter.end.line {
                continue;
            }
        }

        let name = index.symbol_name(symbol);
        let (token_type, modifiers) = symbol_token_info(symbol, name);

        raw.push(RawToken {
            line: pos.line,
            col: pos.column,
            len,
            token_type,
            modifiers: modifiers | MOD_DEFINITION,
        });
    }

    for reference in index.references() {
        let pos = line_index.position(reference.range.start());
        let len = reference.range.len().to_u32();

        if len == 0 {
            continue;
        }

        if let Some(ref filter) = range_filter {
            if pos.line < filter.start.line || pos.line > filter.end.line {
                continue;
            }
        }

        let (token_type, modifiers) = match reference.resolved {
            Some(sym_id) => match index.symbol(sym_id) {
                Some(sym) => symbol_token_info(sym, index.symbol_name(sym)),
                None => (TT_VARIABLE, 0),
            },
            None => {
                let name = index.reference_name(reference);
                if crate::builtins::lookup(name).is_some() {
                    (TT_BUILTIN, 0)
                } else {
                    (TT_VARIABLE, 0)
                }
            }
        };

        raw.push(RawToken {
            line: pos.line,
            col: pos.column,
            len,
            token_type,
            modifiers,
        });
    }
}

fn symbol_token_info(symbol: &sneklsp_index::SymbolData, name: &str) -> (u32, u32) {
    let mut modifiers = 0u32;

    if name.len() > 1 && name.bytes().all(|b| b.is_ascii_uppercase() || b == b'_') {
        modifiers |= MOD_READONLY;
    }

    let token_type = match symbol.kind {
        sneklsp_index::SymbolKind::Function => TT_FUNCTION,
        sneklsp_index::SymbolKind::Method => TT_METHOD,
        sneklsp_index::SymbolKind::Class => TT_CLASS,
        sneklsp_index::SymbolKind::Variable => TT_VARIABLE,
        sneklsp_index::SymbolKind::Parameter => {
            if name == "self" || name == "cls" {
                return (TT_SELF, modifiers);
            }
            TT_PARAMETER
        }
        sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol => {
            TT_NAMESPACE
        }
        sneklsp_index::SymbolKind::Property => TT_PROPERTY,
        sneklsp_index::SymbolKind::TypeAlias => TT_CLASS,
    };

    (token_type, modifiers)
}

fn dedup_overlapping(tokens: &mut Vec<RawToken>) {
    tokens.dedup_by(|b, a| {
        if a.line == b.line && a.col == b.col {
            a.token_type = b.token_type;
            a.modifiers = b.modifiers;
            a.len = b.len;
            true
        } else {
            false
        }
    });
}

fn encode(raw: Vec<RawToken>) -> SemanticTokens {
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;
    let mut data = Vec::with_capacity(raw.len());

    for token in &raw {
        let delta_line = token.line - prev_line;
        let delta_start = if delta_line == 0 {
            token.col - prev_col
        } else {
            token.col
        };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.len,
            token_type: token.token_type,
            token_modifiers_bitset: token.modifiers,
        });

        prev_line = token.line;
        prev_col = token.col;
    }

    SemanticTokens {
        result_id: None,
        data,
    }
}

#[inline]
fn token_text(source: &str, range: TextRange) -> &str {
    let start = range.start().to_usize();
    let end = range.end().to_usize();
    if end <= source.len() {
        &source[start..end]
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sneklsp_ast::AstArena;

    fn make_test_data(source: &str) -> (Vec<Token>, OwnedIndex, LineIndex) {
        let tokens = sneklsp_lexer::tokenize(source);
        let line_index = LineIndex::new(source);
        let arena = AstArena::new();
        let module = sneklsp_parser::parse(source, &arena).unwrap();
        let idx = sneklsp_index::index_module(source, &module);
        let owned = OwnedIndex::new(source.to_string(), &idx);
        (tokens, owned, line_index)
    }

    #[test]
    fn simple_assignment() {
        let source = "x = 42";
        let (_, index, line_index) = make_test_data(source);
        let result = compute_semantic_tokens(&index, &line_index);
        let SemanticTokensResult::Tokens(semantic) = result else {
            panic!("expected tokens");
        };

        assert!(!semantic.data.is_empty());
    }

    #[test]
    fn function_def_tokens() {
        let source = "def foo(x):\n    return x";
        let (_, index, line_index) = make_test_data(source);
        let result = compute_semantic_tokens(&index, &line_index);
        let SemanticTokensResult::Tokens(semantic) = result else {
            panic!("expected tokens");
        };

        assert!(semantic.data.len() >= 5);
        assert_eq!(semantic.data[0].token_type, TT_KEYWORD);
    }

    #[test]
    fn class_and_method() {
        let source = "class Foo:\n    def bar(self):\n        pass";
        let (_, index, line_index) = make_test_data(source);
        let result = compute_semantic_tokens(&index, &line_index);
        let SemanticTokensResult::Tokens(semantic) = result else {
            panic!("expected tokens");
        };

        let has_class = semantic.data.iter().any(|t| t.token_type == TT_CLASS);
        assert!(has_class, "should have class token");

        let has_method = semantic.data.iter().any(|t| t.token_type == TT_METHOD);
        assert!(has_method, "should have method token");

        let has_self = semantic.data.iter().any(|t| t.token_type == TT_SELF);
        assert!(has_self, "should have self token");
    }

    #[test]
    fn comment_detection() {
        let source = "x = 1  # a comment";
        let (_, index, line_index) = make_test_data(source);
        let result = compute_semantic_tokens(&index, &line_index);
        let SemanticTokensResult::Tokens(semantic) = result else {
            panic!("expected tokens");
        };

        let has_comment = semantic.data.iter().any(|t| t.token_type == TT_COMMENT);
        assert!(has_comment, "should detect comment");
    }

    #[test]
    fn hash_in_string_not_comment() {
        assert!(likely_inside_string("x = \"hello # world\"", 14));
        assert!(!likely_inside_string("x = 1  # comment", 7));
    }

    #[test]
    fn readonly_constant() {
        let source = "MAX_SIZE = 100";
        let (_, index, line_index) = make_test_data(source);
        let result = compute_semantic_tokens(&index, &line_index);
        let SemanticTokensResult::Tokens(semantic) = result else {
            panic!("expected tokens");
        };

        let has_readonly = semantic
            .data
            .iter()
            .any(|t| t.token_modifiers_bitset & MOD_READONLY != 0);
        assert!(has_readonly, "ALL_CAPS should get readonly modifier");
    }
}
