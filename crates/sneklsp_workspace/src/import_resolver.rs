use crate::Workspace;
use sneklsp_vfs::FileId;

#[derive(Debug, Clone)]
pub struct ResolvedImport {
    pub file_id: FileId,
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
        Some(ResolvedImport { file_id })
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
