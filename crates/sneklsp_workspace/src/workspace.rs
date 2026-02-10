use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

use sneklsp_ast::AstArena;
use sneklsp_index::OwnedIndex;
use sneklsp_lexer::Token;
use sneklsp_text::LineIndex;
use sneklsp_vfs::{FileId, Vfs, VfsPath};

use crate::discovery::discover_python_files;

pub struct FileState {
    pub index: Option<OwnedIndex>,
    pub line_index: LineIndex,
    pub tokens: Vec<Token>,
    pub version: Option<i32>,
}

pub struct Workspace {
    pub vfs: Vfs,
    files: FxHashMap<FileId, FileState>,
    module_map: FxHashMap<String, FileId>,
    roots: Vec<VfsPath>,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            vfs: Vfs::new(),
            files: FxHashMap::default(),
            module_map: FxHashMap::default(),
            roots: Vec::new(),
        }
    }

    pub fn add_root(&mut self, root: &Path) -> Vec<FileId> {
        let root_vfs = VfsPath::new(root.to_path_buf());
        self.roots.push(root_vfs);

        let py_files = discover_python_files(root);
        let mut file_ids = Vec::with_capacity(py_files.len());

        for path in py_files {
            let module_name = path_to_module_name(root, &path);
            let vfs_path = VfsPath::new(path);
            let file_id = self.vfs.intern_path(vfs_path);
            file_ids.push(file_id);

            if let Some(name) = module_name {
                self.module_map.insert(name, file_id);
            }
        }

        tracing::info!(
            root = %root.display(),
            file_count = file_ids.len(),
            "discovered workspace files"
        );

        file_ids
    }

    pub fn index_file(&mut self, file_id: FileId) -> bool {
        let content = match self.vfs.read(file_id) {
            Some(c) => c,
            None => return false,
        };

        let line_index = LineIndex::new(&content);
        let arena_size = (content.len() * 50).max(4096);
        let arena = AstArena::with_capacity(arena_size);

        let output = sneklsp_parser::parse_recovering(&content, &arena);

        let index = if !output.module.body.is_empty() || output.errors.is_empty() {
            let idx = sneklsp_index::index_module(&content, &output.module);
            Some(OwnedIndex::new(content.to_string(), &idx))
        } else {
            None
        };

        let tokens = sneklsp_lexer::tokenize(&content);

        self.files.insert(
            file_id,
            FileState {
                index,
                line_index,
                tokens,
                version: self.vfs.version(file_id),
            },
        );

        true
    }

    pub fn set_file_state(&mut self, id: FileId, state: FileState) {
        self.files.insert(id, state);
    }

    #[inline]
    pub fn get_file_state(&self, id: FileId) -> Option<&FileState> {
        self.files.get(&id)
    }

    #[inline]
    pub fn file_state_mut(&mut self, id: FileId) -> Option<&mut FileState> {
        self.files.get_mut(&id)
    }

    pub fn remove_file_state(&mut self, id: FileId) {
        self.files.remove(&id);
    }

    #[inline]
    pub fn resolve_module(&self, module_name: &str) -> Option<FileId> {
        self.module_map.get(module_name).copied()
    }

    pub fn resolve_module_name(&self, file_id: FileId) -> Option<String> {
        self.module_map
            .iter()
            .find(|(_, id)| **id == file_id)
            .map(|(name, _)| name.clone())
    }

    pub fn file_id_for_uri(&mut self, uri: &lsp_types::Uri) -> Option<FileId> {
        let path = VfsPath::from_uri(uri)?;
        Some(self.vfs.intern_path(path))
    }

    pub fn lookup_uri(&self, uri: &lsp_types::Uri) -> Option<FileId> {
        let path = VfsPath::from_uri(uri)?;
        self.vfs.file_id(&path)
    }

    #[inline]
    pub fn roots(&self) -> &[VfsPath] {
        &self.roots
    }

    pub fn indexed_files(&self) -> impl Iterator<Item = FileId> + '_ {
        self.files.keys().copied()
    }

    pub fn find_exported_symbol(&self, name: &str) -> Vec<(FileId, u32)> {
        let mut results = Vec::new();

        for (&file_id, state) in &self.files {
            if let Some(ref index) = state.index {
                if let Some(root_scope) = index.root_scope() {
                    for &sym_id in &root_scope.symbols {
                        if let Some(symbol) = index.symbol(sym_id) {
                            if index.symbol_name(symbol) == name
                                && symbol.visibility != sneklsp_index::Visibility::Private
                                && symbol.visibility != sneklsp_index::Visibility::DunderPrivate
                            {
                                results.push((file_id, sym_id));
                            }
                        }
                    }
                }
            }
        }

        results
    }
}

fn path_to_module_name(root: &Path, file: &PathBuf) -> Option<String> {
    let relative = file.strip_prefix(root).ok()?;
    let mut components: Vec<&str> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.is_empty() {
        return None;
    }

    // strip .py extension from last component
    let last = components.last_mut()?;
    *last = last.strip_suffix(".py").unwrap_or(last);

    // __init__ means the module is the parent package
    if *components.last()? == "__init__" {
        components.pop();
    }

    if components.is_empty() {
        return None;
    }

    Some(components.join("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_name_simple() {
        let root = PathBuf::from("/project");
        let file = PathBuf::from("/project/foo.py");
        assert_eq!(path_to_module_name(&root, &file), Some("foo".to_string()));
    }

    #[test]
    fn module_name_nested() {
        let root = PathBuf::from("/project");
        let file = PathBuf::from("/project/foo/bar.py");
        assert_eq!(
            path_to_module_name(&root, &file),
            Some("foo.bar".to_string())
        );
    }

    #[test]
    fn module_name_dunder_init() {
        let root = PathBuf::from("/project");
        let file = PathBuf::from("/project/foo/__init__.py");
        assert_eq!(path_to_module_name(&root, &file), Some("foo".to_string()));
    }
}
