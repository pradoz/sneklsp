use rustc_hash::FxHashMap;
use salsa::Setter;

use sneklsp_db::{
    Database, ExportedSymbol, File, ModuleEntry, ModuleGraph, ModuleName, file_exported_symbols,
    file_index, file_line_index, file_parsed_data, file_tokens, resolve_module,
};
use sneklsp_index::OwnedIndex;
use sneklsp_lexer::Token;
use sneklsp_text::LineIndex;
use sneklsp_vfs::FileId;

pub struct AnalysisHost {
    db: Database,
    file_map: FxHashMap<FileId, File>,
    module_graph: Option<ModuleGraph>,
    module_entries: Vec<ModuleEntry>,
    // (file_id, module_name, path, content)
    pending_modules: Vec<(FileId, String, String, String)>,
    modules_dirty: bool,
}

impl AnalysisHost {
    pub fn new() -> Self {
        Self {
            db: Database::default(),
            file_map: FxHashMap::default(),
            module_graph: None,
            module_entries: Vec::new(),
            pending_modules: Vec::new(),
            modules_dirty: false,
        }
    }

    pub fn set_file_content(&mut self, file_id: FileId, path: &str, content: String) {
        if let Some(&file) = self.file_map.get(&file_id) {
            file.set_content(&mut self.db).to(content);
        } else {
            let file = File::new(&self.db, path.to_string(), content);
            self.file_map.insert(file_id, file);
        }
    }

    pub fn queue_module(
        &mut self,
        file_id: FileId,
        module_name: String,
        path: String,
        content: String,
    ) {
        self.pending_modules
            .push((file_id, module_name, path, content));
    }

    pub fn flush_modules(&mut self) {
        if self.pending_modules.is_empty() {
            return;
        }

        let pending = std::mem::take(&mut self.pending_modules);
        for (file_id, module_name, path, content) in pending {
            let file = if let Some(&existing) = self.file_map.get(&file_id) {
                existing.set_content(&mut self.db).to(content);
                existing
            } else {
                let file = File::new(&self.db, path, content);
                self.file_map.insert(file_id, file);
                file
            };

            self.module_entries.retain(|e| e.name != module_name);
            self.module_entries.push(ModuleEntry {
                name: module_name,
                file,
            });
            self.modules_dirty = true;
        }

        if self.modules_dirty {
            let entries = self.module_entries.clone();
            if let Some(graph) = self.module_graph {
                graph.set_entries(&mut self.db).to(entries);
            } else {
                self.module_graph = Some(ModuleGraph::new(&self.db, entries));
            }
            self.modules_dirty = false;
        }
    }

    pub fn resolve_module_file(&self, module_name: &str) -> Option<File> {
        let graph = self.module_graph?;
        let interned = ModuleName::new(&self.db, module_name.to_string());
        resolve_module(&self.db, graph, interned)
    }

    pub fn exported_symbols(&self, file_id: FileId) -> Option<&[ExportedSymbol]> {
        let file = self.file_map.get(&file_id)?;
        Some(file_exported_symbols(&self.db, *file))
    }

    pub fn file_index(&self, file_id: FileId) -> Option<&OwnedIndex> {
        let file = self.file_map.get(&file_id)?;
        file_index(&self.db, *file).as_ref()
    }

    pub fn file_tokens(&self, file_id: FileId) -> Option<&[Token]> {
        let file = self.file_map.get(&file_id)?;
        Some(file_tokens(&self.db, *file))
    }

    pub fn file_parse_errors(
        &self,
        file_id: FileId,
    ) -> Option<&[sneklsp_db::SerializedParseError]> {
        let file = self.file_map.get(&file_id)?;
        Some(&file_parsed_data(&self.db, *file).errors)
    }

    pub fn file_line_index(&self, file_id: FileId) -> Option<&LineIndex> {
        let file = self.file_map.get(&file_id)?;
        Some(file_line_index(&self.db, *file))
    }

    pub fn find_exported_symbol(&self, name: &str) -> Vec<(FileId, u32)> {
        let mut results = Vec::new();
        for (&file_id, &file) in &self.file_map {
            let exports = file_exported_symbols(&self.db, file);
            for export in exports {
                if export.name == name {
                    results.push((file_id, export.symbol_id));
                }
            }
        }
        results
    }

    pub fn file_ids(&self) -> impl Iterator<Item = FileId> + '_ {
        self.file_map.keys().copied()
    }

    pub fn file_for_id(&self, file_id: FileId) -> Option<File> {
        self.file_map.get(&file_id).copied()
    }

    #[inline]
    pub fn db(&self) -> &Database {
        &self.db
    }
}

impl Default for AnalysisHost {
    fn default() -> Self {
        Self::new()
    }
}
