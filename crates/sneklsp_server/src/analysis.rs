use rustc_hash::FxHashMap;
use salsa::Setter;

use sneklsp_db::{
    Database, ExportedSymbol, File, FileAnalysis, ModuleEntry, ModuleGraph, ModuleName,
    file_exported_symbols, file_line_index, parse_file_recovering, resolve_module,
};
use sneklsp_text::LineIndex;
use sneklsp_vfs::FileId;

pub struct AnalysisHost {
    db: Database,
    file_map: FxHashMap<FileId, File>,
    module_graph: Option<ModuleGraph>,
    module_entries: Vec<ModuleEntry>,
    // (file_id, module_name, path, content)
    pending_modules: Vec<(FileId, String, String, String)>,
}

impl AnalysisHost {
    pub fn new() -> Self {
        Self {
            db: Database::default(),
            file_map: FxHashMap::default(),
            module_graph: None,
            module_entries: Vec::new(),
            pending_modules: Vec::new(),
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
        }

        let entries = self.module_entries.clone();
        if let Some(graph) = self.module_graph {
            graph.set_entries(&mut self.db).to(entries);
        } else {
            self.module_graph = Some(ModuleGraph::new(&self.db, entries));
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

    pub fn analyze_file(&self, file_id: FileId) -> Option<&FileAnalysis> {
        let file = self.file_map.get(&file_id)?;
        Some(parse_file_recovering(&self.db, *file))
    }

    pub fn file_ids(&self) -> impl Iterator<Item = FileId> + '_ {
        self.file_map.keys().copied()
    }

    pub fn file_for_id(&self, file_id: FileId) -> Option<File> {
        self.file_map.get(&file_id).copied()
    }

    pub fn line_index(&self, file_id: FileId) -> Option<&LineIndex> {
        let file = self.file_map.get(&file_id)?;
        Some(file_line_index(&self.db, *file))
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
