mod input;
mod queries;
mod types;

pub use input::{File, ModuleEntry, ModuleGraph, ModuleName};
pub use queries::{
    ExportedSymbol, ParseErrorKind, SerializedParseError, file_exported_symbols, file_index,
    file_line_index, file_parse_errors, file_tokens, resolve_module,
};
pub use types::{Ty, infer_symbol_type};

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
