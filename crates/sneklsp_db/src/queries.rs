use crate::{File, ModuleGraph};
use sneklsp_index::OwnedIndex;
use sneklsp_lexer::Token;
use sneklsp_text::LineIndex;

#[derive(Debug, Clone)]
pub struct ParsedFileData {
    pub index: Option<OwnedIndex>,
    pub errors: Vec<SerializedParseError>,
}

impl PartialEq for ParsedFileData {
    fn eq(&self, other: &Self) -> bool {
        self.errors == other.errors && self.index == other.index
    }
}

impl Eq for ParsedFileData {}

impl std::hash::Hash for ParsedFileData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.errors.hash(state);
        self.index.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SerializedParseError {
    pub kind: ParseErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ParseErrorKind {
    UnexpectedToken { range: sneklsp_text::TextRange },
    UnexpectedEof,
    InvalidSyntax { offset: sneklsp_text::TextSize },
}

impl From<&sneklsp_parser::ParseError> for SerializedParseError {
    fn from(error: &sneklsp_parser::ParseError) -> Self {
        match error {
            sneklsp_parser::ParseError::UnexpectedToken {
                range,
                expected,
                found,
            } => SerializedParseError {
                kind: ParseErrorKind::UnexpectedToken { range: *range },
                message: format!("expected {expected}, found {found}"),
            },
            sneklsp_parser::ParseError::UnexpectedEof => SerializedParseError {
                kind: ParseErrorKind::UnexpectedEof,
                message: "unexpected end of file".to_string(),
            },
            sneklsp_parser::ParseError::InvalidSyntax(offset) => SerializedParseError {
                kind: ParseErrorKind::InvalidSyntax { offset: *offset },
                message: "invalid syntax".to_string(),
            },
        }
    }
}

#[salsa::tracked(returns(ref))]
pub fn file_tokens(db: &dyn salsa::Database, file: File) -> Vec<Token> {
    let content = file.content(db);
    tracing::debug!(path = %file.path(db), "tokenizing");
    sneklsp_lexer::tokenize(content)
}

#[salsa::tracked(returns(ref))]
pub fn file_parsed_data(db: &dyn salsa::Database, file: File) -> ParsedFileData {
    let content = file.content(db);
    let path = file.path(db);
    tracing::debug!(path = %path, "parsing + indexing");

    let arena = sneklsp_ast::AstArena::with_capacity((content.len() * 50).max(4096));
    let output = sneklsp_parser::parse_recovering(content, &arena);

    let errors: Vec<SerializedParseError> = output
        .errors
        .iter()
        .map(SerializedParseError::from)
        .collect();

    let index = if !output.module.body.is_empty() || output.errors.is_empty() {
        let idx = sneklsp_index::index_module(content, &output.module);
        Some(sneklsp_index::OwnedIndex::new(content.to_string(), &idx))
    } else {
        None
    };

    ParsedFileData { index, errors }
}

#[salsa::tracked(returns(ref))]
pub fn file_index(db: &dyn salsa::Database, file: File) -> Option<OwnedIndex> {
    file_parsed_data(db, file).index.clone()
}

#[salsa::tracked(returns(ref))]
pub fn file_line_index(db: &dyn salsa::Database, file: File) -> LineIndex {
    let content = file.content(db);
    LineIndex::new(content)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExportedSymbol {
    pub name: String,
    pub symbol_id: u32,
    pub kind: sneklsp_index::SymbolKind,
    pub range: sneklsp_text::TextRange,
}

#[salsa::tracked(returns(ref))]
pub fn file_exported_symbols(db: &dyn salsa::Database, file: File) -> Vec<ExportedSymbol> {
    let Some(ref index) = *file_index(db, file) else {
        return Vec::new();
    };

    let Some(root_scope) = index.root_scope() else {
        return Vec::new();
    };

    let mut exports = Vec::new();
    for &sym_id in &root_scope.symbols {
        if let Some(symbol) = index.symbol(sym_id) {
            if symbol.visibility == sneklsp_index::Visibility::Public {
                exports.push(ExportedSymbol {
                    name: index.symbol_name(symbol).to_string(),
                    symbol_id: sym_id,
                    kind: symbol.kind,
                    range: symbol.selection_range,
                });
            }
        }
    }
    exports
}

#[salsa::tracked]
pub fn resolve_module(
    db: &dyn salsa::Database,
    graph: ModuleGraph,
    name: crate::ModuleName<'_>,
) -> Option<File> {
    let target = name.name(db);
    graph
        .entries(db)
        .iter()
        .find(|m| m.name == *target)
        .map(|m| m.file)
}
