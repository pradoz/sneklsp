use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(u32);

impl FileId {
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VfsPath(PathBuf);

impl VfsPath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    #[inline]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn from_uri(uri: &lsp_types::Uri) -> Option<Self> {
        let path_str = uri.path().as_str();
        if path_str.is_empty() {
            return None;
        }
        Some(Self(PathBuf::from(path_str)))
    }

    pub fn to_uri(&self) -> Option<lsp_types::Uri> {
        let url = format!("file://{}", self.0.display());
        url.parse().ok()
    }
}

struct FileEntry {
    overlay: Option<Arc<str>>,
    version: Option<i32>,
}

pub struct Vfs {
    path_to_id: HashMap<VfsPath, FileId>,
    id_to_path: Vec<VfsPath>,
    entries: Vec<FileEntry>,
}

impl Vfs {
    pub fn new() -> Self {
        Self {
            path_to_id: HashMap::new(),
            id_to_path: Vec::new(),
            entries: Vec::new(),
        }
    }

    pub fn intern_path(&mut self, path: VfsPath) -> FileId {
        if let Some(&id) = self.path_to_id.get(&path) {
            return id;
        }

        let id = FileId(self.id_to_path.len() as u32);
        self.path_to_id.insert(path.clone(), id);
        self.id_to_path.push(path);
        self.entries.push(FileEntry {
            overlay: None,
            version: None,
        });
        id
    }

    #[inline]
    pub fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        self.path_to_id.get(path).copied()
    }

    #[inline]
    pub fn file_path(&self, id: FileId) -> &VfsPath {
        &self.id_to_path[id.as_usize()]
    }

    pub fn set_overlay(&mut self, id: FileId, content: String, version: i32) {
        let entry = &mut self.entries[id.as_usize()];
        entry.overlay = Some(Arc::from(content));
        entry.version = Some(version);
    }

    pub fn remove_overlay(&mut self, id: FileId) {
        let entry = &mut self.entries[id.as_usize()];
        entry.overlay = None;
        entry.version = None;
    }

    pub fn read(&self, id: FileId) -> Option<Arc<str>> {
        let entry = &self.entries[id.as_usize()];

        if let Some(ref overlay) = entry.overlay {
            return Some(Arc::clone(overlay));
        }

        // fall back to disk
        let path = &self.id_to_path[id.as_usize()];
        match std::fs::read_to_string(path.as_path()) {
            Ok(content) => Some(Arc::from(content)),
            Err(_) => None,
        }
    }

    #[inline]
    pub fn has_overlay(&self, id: FileId) -> bool {
        self.entries[id.as_usize()].overlay.is_some()
    }

    #[inline]
    pub fn version(&self, id: FileId) -> Option<i32> {
        self.entries[id.as_usize()].version
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.id_to_path.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.id_to_path.is_empty()
    }

    pub fn file_ids(&self) -> impl Iterator<Item = FileId> {
        (0..self.id_to_path.len() as u32).map(FileId)
    }
}

impl Default for Vfs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_and_lookup() {
        let mut vfs = Vfs::new();
        let path = VfsPath::new(PathBuf::from("/tmp/test.py"));
        let id = vfs.intern_path(path.clone());
        assert_eq!(vfs.file_id(&path), Some(id));
        assert_eq!(vfs.file_path(id), &path);
    }

    #[test]
    fn overlay_priority() {
        let mut vfs = Vfs::new();
        let path = VfsPath::new(PathBuf::from("/tmp/test.py"));
        let id = vfs.intern_path(path.clone());

        vfs.set_overlay(id, "overlay content".to_string(), 1);
        let content = vfs.read(id).unwrap();
        assert_eq!(content, "overlay content".into());
    }

    #[test]
    fn stable_ids() {
        let mut vfs = Vfs::new();
        let p1 = VfsPath::new(PathBuf::from("/a.py"));
        let p2 = VfsPath::new(PathBuf::from("/b.py"));
        let id1 = vfs.intern_path(p1.clone());
        let id2 = vfs.intern_path(p2);

        let id1_again = vfs.intern_path(p1);
        assert_eq!(id1, id1_again);
        assert_ne!(id1, id2);
    }
}
