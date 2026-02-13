use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

use sneklsp_vfs::{FileId, Vfs, VfsPath};

use crate::discovery::discover_python_files;

pub struct Workspace {
    pub vfs: Vfs,
    module_map: FxHashMap<String, FileId>,
    reverse_module_map: FxHashMap<FileId, String>,
    roots: Vec<VfsPath>,
}

impl Workspace {
    pub fn new() -> Self {
        Self {
            vfs: Vfs::new(),
            module_map: FxHashMap::default(),
            reverse_module_map: FxHashMap::default(),
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
                self.reverse_module_map.insert(file_id, name.clone());
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

    #[inline]
    pub fn resolve_module(&self, module_name: &str) -> Option<FileId> {
        self.module_map.get(module_name).copied()
    }

    #[inline]
    pub fn resolve_module_name(&self, file_id: FileId) -> Option<String> {
        self.reverse_module_map.get(&file_id).cloned()
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
