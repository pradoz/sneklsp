use crate::{File, ModuleGraph};
use sneklsp_index::OwnedIndex;
use sneklsp_lexer::Token;
use sneklsp_text::LineIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParseOutput {
    pub error_count: u32,
    pub stmt_count: u32,
    pub has_errors: bool,
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

#[derive(Debug)]
pub struct FileAnalysis {
    pub index: Option<OwnedIndex>,
    pub line_index: LineIndex,
    pub tokens: Vec<Token>,
    pub errors: Vec<SerializedParseError>,
}

impl PartialEq for FileAnalysis {
    fn eq(&self, _other: &Self) -> bool {
        false
    }
}

impl Eq for FileAnalysis {}

impl std::hash::Hash for FileAnalysis {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.errors.len().hash(state);
        self.tokens.len().hash(state);
    }
}

#[salsa::tracked(returns(ref))]
pub fn file_tokens(db: &dyn salsa::Database, file: File) -> Vec<Token> {
    let content = file.content(db);
    tracing::debug!(path = %file.path(db), "tokenizing");
    sneklsp_lexer::tokenize(content)
}

#[salsa::tracked]
pub fn parse_file(db: &dyn salsa::Database, file: File) -> ParseOutput {
    let content = file.content(db);
    tracing::debug!(path = %file.path(db), "parsing");

    let arena = sneklsp_ast::AstArena::with_capacity((content.len() * 50).max(4096));
    let output = sneklsp_parser::parse_recovering(content, &arena);

    ParseOutput {
        error_count: output.errors.len() as u32,
        stmt_count: output.module.body.len() as u32,
        has_errors: !output.errors.is_empty(),
    }
}

#[salsa::tracked(returns(ref))]
pub fn parse_file_recovering(db: &dyn salsa::Database, file: File) -> FileAnalysis {
    let content = file.content(db);
    let path = file.path(db);
    tracing::debug!(path = %path, "full analysis");

    let analyzed = sneklsp_index::analyze_source(content);
    let errors: Vec<SerializedParseError> = analyzed
        .errors
        .iter()
        .map(SerializedParseError::from)
        .collect();

    FileAnalysis {
        index: analyzed.index,
        line_index: analyzed.line_index,
        tokens: analyzed.tokens,
        errors,
    }
}

#[salsa::tracked(returns(ref))]
pub fn file_line_index(db: &dyn salsa::Database, file: File) -> LineIndex {
    let content = file.content(db);
    LineIndex::new(content)
}

pub fn file_index(db: &dyn salsa::Database, file: File) -> Option<&OwnedIndex> {
    let analysis = parse_file_recovering(db, file);
    analysis.index.as_ref()
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
    let analysis = parse_file_recovering(db, file);
    let Some(ref index) = analysis.index else {
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
