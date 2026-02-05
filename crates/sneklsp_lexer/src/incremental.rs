use crate::edit::TextEdit;
use crate::{Lexer, Token, TokenKind};
use sneklsp_text::{TextRange, TextSize};

pub struct IncrementalLexResult {
    pub tokens: Vec<Token>,
    pub fully_relexed: bool,
}

pub fn relex(
    old_tokens: &[Token],
    old_source: &str,
    new_source: &str,
    edit: TextEdit,
) -> IncrementalLexResult {
    if old_source.len() < 1000 || edit_is_large(&edit, old_source.len()) {
        return IncrementalLexResult {
            tokens: crate::tokenize(new_source),
            fully_relexed: true,
        };
    }

    let edit_start = edit.range.start();
    let edit_end = edit.range.end();
    let delta = edit.offset_delta();

    let first_affected = find_first_affected(old_tokens, edit_start);

    let relex_start = if first_affected > 0 {
        find_safe_relex_start(old_tokens, first_affected)
    } else {
        0
    };

    let prefix: Vec<Token> = old_tokens[..relex_start].to_vec();
    let resume_point = find_resume_point(old_tokens, edit_end, new_source, delta);

    let relex_start_offset = if relex_start > 0 {
        old_tokens[relex_start - 1].range.end()
    } else {
        TextSize::new(0)
    };
    let relex_end_offset = if let Some((idx, _)) = resume_point {
        adjust_offset(old_tokens[idx].range.start(), delta)
    } else {
        TextSize::new(new_source.len() as u32)
    };

    let region_to_lex = &new_source[relex_start_offset.to_usize()..relex_end_offset.to_usize()];
    let middle_tokens = lex_region(region_to_lex, relex_start_offset);

    let suffix: Vec<Token> = match resume_point {
        Some((idx, _)) => old_tokens[idx..]
            .iter()
            .map(|t| adjust_token(t, delta))
            .collect(),
        None => Vec::new(),
    };

    let mut tokens = prefix;
    tokens.extend(middle_tokens);
    tokens.extend(suffix);

    IncrementalLexResult {
        tokens,
        fully_relexed: false,
    }
}

fn edit_is_large(edit: &TextEdit, source_len: usize) -> bool {
    let edit_len = edit.range.len().to_usize();
    edit_len > source_len / 4 || edit.new_len.to_usize() > source_len / 4
}

fn find_first_affected(tokens: &[Token], edit_start: TextSize) -> usize {
    tokens.partition_point(|t| t.range.end() <= edit_start)
}

fn find_safe_relex_start(tokens: &[Token], first_affected: usize) -> usize {
    for i in (0..first_affected).rev() {
        if matches!(tokens[i].kind, TokenKind::Newline | TokenKind::Dedent) {
            return i + 1;
        }
    }
    0
}

fn find_resume_point(
    old_tokens: &[Token],
    edit_end: TextSize,
    new_source: &str,
    delta: i64,
) -> Option<(usize, TextSize)> {
    for (i, token) in old_tokens.iter().enumerate() {
        if token.range.start() <= edit_end {
            continue;
        }

        let new_start = adjust_offset(token.range.start(), delta);
        if new_start.to_usize() >= new_source.len() {
            return None;
        }

        if token.kind == TokenKind::Newline {
            return Some((i, new_start));
        }
    }
    None
}

fn adjust_offset(offset: TextSize, delta: i64) -> TextSize {
    if delta >= 0 {
        TextSize::new(offset.to_u32() + delta as u32)
    } else {
        TextSize::new(offset.to_u32().saturating_sub((-delta) as u32))
    }
}

fn adjust_token(token: &Token, delta: i64) -> Token {
    Token::new(
        token.kind,
        TextRange::new(
            adjust_offset(token.range.start(), delta),
            adjust_offset(token.range.end(), delta),
        ),
    )
}

fn lex_region(source: &str, offset: TextSize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut lexer = Lexer::new(source);

    loop {
        let mut token = lexer.next_token();

        if token.kind == TokenKind::Eof {
            break;
        }

        token = Token::new(
            token.kind,
            TextRange::new(
                TextSize::new(token.range.start().to_u32() + offset.to_u32()),
                TextSize::new(token.range.end().to_u32() + offset.to_u32()),
            ),
        );
        tokens.push(token);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(TextSize::new(start), TextSize::new(end))
    }

    #[test]
    fn small_file_full_relex() {
        let old = "x = 1";
        let new = "x = 2";
        let old_tokens = crate::tokenize(old);
        let edit = TextEdit::new(range(4, 5), TextSize::new(1));

        let result = relex(&old_tokens, old, new, edit);

        assert!(result.fully_relexed);
        assert_eq!(result.tokens.len(), 3);
    }

    #[test]
    fn preserves_token_count_simple_replace() {
        let old = "x = 1\ny = 2\nz = 3";
        let new = "x = 1\ny = 9\nz = 3";
        let old_tokens = crate::tokenize(old);

        let edit = TextEdit::new(range(10, 11), TextSize::new(1));
        let result = relex(&old_tokens, old, new, edit);

        let new_tokens = crate::tokenize(new);
        assert_eq!(result.tokens.len(), new_tokens.len());
    }
}
