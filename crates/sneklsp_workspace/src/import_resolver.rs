use crate::Workspace;
use sneklsp_index::OwnedIndex;
use sneklsp_text::TextRange;
use sneklsp_vfs::FileId;

#[derive(Debug, Clone)]
pub struct ResolvedImport {
    pub file_id: FileId,
    pub symbol_range: Option<TextRange>,
    pub symbol_id: Option<u32>,
}

pub struct ImportResolver<'a> {
    workspace: &'a Workspace,
}

impl<'a> ImportResolver<'a> {
    pub fn new(workspace: &'a Workspace) -> Self {
        Self { workspace }
    }

    pub fn resolve_import(&self, module_name: &str) -> Option<ResolvedImport> {
        let file_id = self.workspace.resolve_module(module_name)?;
        Some(ResolvedImport {
            file_id,
            symbol_range: None,
            symbol_id: None,
        })
    }

    pub fn resolve_import_from(
        &self,
        module_name: &str,
        symbol_name: &str,
    ) -> Option<ResolvedImport> {
        let file_id = self.workspace.resolve_module(module_name)?;
        let state = self.workspace.file_state(file_id)?;
        let index = state.index.as_ref()?;

        find_exported_symbol(index, symbol_name).map(|(sid, range)| ResolvedImport {
            file_id,
            symbol_range: Some(range),
            symbol_id: Some(sid),
        })
    }

    pub fn resolve_relative_import(
        &self,
        from_file: FileId,
        module: Option<&str>,
        level: u32,
    ) -> Option<FileId> {
        if level == 0 {
            return module.and_then(|m| self.workspace.resolve_module(m));
        }

        // look for package path of importing file
        let from_path = self.workspace.vfs.file_path(from_file);
        let mut current = from_path.as_path().to_path_buf();

        for _ in 0..level {
            current.pop();
        }

        if let Some(mod_name) = module {
            for part in mod_name.split('.') {
                current.push(part);
            }
        }

        let mut py_path = current.clone();
        py_path.set_extension("py");
        let vfs_path = sneklsp_vfs::VfsPath::new(py_path);
        if let Some(id) = self.workspace.vfs.file_id(&vfs_path) {
            return Some(id);
        }

        current.push("__init__.py");
        let vfs_path = sneklsp_vfs::VfsPath::new(current);
        self.workspace.vfs.file_id(&vfs_path)
    }
}

fn find_exported_symbol(index: &OwnedIndex, name: &str) -> Option<(u32, TextRange)> {
    let root_scope = index.root_scope()?;
    for &sid in &root_scope.symbols {
        if let Some(symbol) = index.symbol(sid) {
            if index.symbol_name(symbol) == name {
                return Some((sid, symbol.selection_range));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_nonexistent_module() {
        let workspace = Workspace::new();
        let resolver = ImportResolver::new(&workspace);
        assert!(resolver.resolve_import("nonexistent").is_none());
    }
}
