mod input;
mod queries;
mod types;

pub use input::{File, ModuleEntry, ModuleGraph, ModuleName, SourceProgram};
pub use queries::{
    ExportedSymbol, FileAnalysis, ParseErrorKind, ParseOutput, SerializedParseError,
    file_exported_symbols, file_index, file_line_index, file_tokens, parse_file,
    parse_file_recovering, resolve_module,
};

#[salsa::db]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for Database {}

impl Database {
    pub fn new() -> Self {
        Self::default()
    }
}
