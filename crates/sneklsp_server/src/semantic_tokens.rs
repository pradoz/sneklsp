use lsp_types::{
    SemanticToken, SemanticTokenType, SemanticTokens, SemanticTokensLegend, SemanticTokensResult,
};

use sneklsp_index::OwnedIndex;
use sneklsp_text::LineIndex;

pub const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::NAMESPACE, // 0 - import/module
    SemanticTokenType::TYPE,      // 1 - class
    SemanticTokenType::CLASS,     // 2 - class (definition)
    SemanticTokenType::FUNCTION,  // 3 - function
    SemanticTokenType::METHOD,    // 4 - method
    SemanticTokenType::PROPERTY,  // 5 - property
    SemanticTokenType::VARIABLE,  // 6 - variable
    SemanticTokenType::PARAMETER, // 7 - parameter
    SemanticTokenType::DECORATOR, // 8 - decorator
    SemanticTokenType::KEYWORD,   // 9 - keyword
    SemanticTokenType::STRING,    // 10 - string
    SemanticTokenType::NUMBER,    // 11 - number
    SemanticTokenType::COMMENT,   // 12 - comment
];

pub fn legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: TOKEN_TYPES.to_vec(),
        token_modifiers: vec![],
    }
}

fn symbol_kind_to_token_type(kind: sneklsp_index::SymbolKind) -> u32 {
    match kind {
        sneklsp_index::SymbolKind::Import | sneklsp_index::SymbolKind::ImportedSymbol => 0,
        sneklsp_index::SymbolKind::Class => 2,
        sneklsp_index::SymbolKind::Function => 3,
        sneklsp_index::SymbolKind::Method => 4,
        sneklsp_index::SymbolKind::Property => 5,
        sneklsp_index::SymbolKind::Variable => 6,
        sneklsp_index::SymbolKind::Parameter => 7,
        sneklsp_index::SymbolKind::TypeAlias => 1,
    }
}

pub fn compute_semantic_tokens(index: &OwnedIndex, line_index: &LineIndex) -> SemanticTokensResult {
    let mut raw_tokens: Vec<(u32, u32, u32, u32)> = Vec::new(); // (line, col, len, type)

    // symbol definitions
    for symbol in index.symbols() {
        let pos = line_index.position(symbol.selection_range.start());
        let len = symbol.selection_range.len().to_u32();
        if len == 0 {
            continue;
        }
        let token_type = symbol_kind_to_token_type(symbol.kind);
        raw_tokens.push((pos.line, pos.column, len, token_type));
    }

    // references
    for reference in index.references() {
        let token_type = match reference.resolved {
            Some(sym_id) => match index.symbol(sym_id) {
                Some(sym) => symbol_kind_to_token_type(sym.kind),
                None => 6, // fallback to variable
            },
            None => 6, // unresolved → variable
        };

        let pos = line_index.position(reference.range.start());
        let len = reference.range.len().to_u32();
        if len == 0 {
            continue;
        }
        raw_tokens.push((pos.line, pos.column, len, token_type));
    }

    raw_tokens.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    raw_tokens.dedup_by(|b, a| a.0 == b.0 && a.1 == b.1); // deduplicate overlapping tokens

    // delta encode
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;
    let mut tokens = Vec::with_capacity(raw_tokens.len());

    for (line, col, len, token_type) in raw_tokens {
        let delta_line = line - prev_line;
        let delta_start = if delta_line == 0 { col - prev_col } else { col };

        tokens.push(SemanticToken {
            delta_line,
            delta_start,
            length: len,
            token_type,
            token_modifiers_bitset: 0,
        });

        prev_line = line;
        prev_col = col;
    }

    SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: tokens,
    })
}
