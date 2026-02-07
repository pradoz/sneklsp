use std::path::{Path, PathBuf};

const IGNORED_DIRS: &[&'static str] = &[
    ".git",
    ".hg",
    ".svn",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".tox",
    ".nox",
    ".venv",
    "venv",
    "env",
    ".env",
    "node_modules",
    ".eggs",
    "*.egg-info",
    "build",
    "dist",
];

pub fn discover_python_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    discover_recursive(root, &mut files);
    files
}

fn discover_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if should_ignore_dir(name) {
                    continue;
                }
            }
            discover_recursive(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "py") {
            files.push(path);
        }
    }
}

fn should_ignore_dir(name: &str) -> bool {
    IGNORED_DIRS.iter().any(|&ignored| {
        if let Some(pattern) = ignored.strip_prefix('*') {
            name.ends_with(pattern)
        } else {
            name == ignored
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_common_dirs() {
        assert!(should_ignore_dir("__pycache__"));
        assert!(should_ignore_dir(".git"));
        assert!(should_ignore_dir("venv"));
        assert!(should_ignore_dir(".venv"));
        assert!(!should_ignore_dir("src"));
        assert!(!should_ignore_dir("mypackage"));
    }

    #[test]
    fn glob_ignore() {
        assert!(should_ignore_dir("foo.egg-info"));
        assert!(should_ignore_dir("bar.egg-info"));
    }
}
