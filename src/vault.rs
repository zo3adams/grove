// GROVE — Vault loader and file tree builder.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents a loaded vault of markdown notes.
#[derive(Debug, Default)]
pub struct Vault {
    pub root: Option<PathBuf>,
    /// Map from file path to its contents.
    pub files: HashMap<PathBuf, String>,
}

impl Vault {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load all .md files from the given directory recursively.
    pub fn load_from_directory(path: &Path) -> std::io::Result<Self> {
        let mut vault = Vault {
            root: Some(path.to_path_buf()),
            files: HashMap::new(),
        };
        vault.scan_directory(path)?;
        Ok(vault)
    }

    fn scan_directory(&mut self, dir: &Path) -> std::io::Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.scan_directory(&path)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                match std::fs::read_to_string(&path) {
                    Ok(contents) => {
                        self.files.insert(path, contents);
                    }
                    Err(e) => {
                        log::warn!("Failed to read {}: {}", path.display(), e);
                    }
                }
            }
        }
        Ok(())
    }

    /// Reload a single file (for incremental updates via file watcher).
    pub fn reload_file(&mut self, path: &Path) -> std::io::Result<()> {
        if path.exists() {
            let contents = std::fs::read_to_string(path)?;
            self.files.insert(path.to_path_buf(), contents);
        } else {
            self.files.remove(path);
        }
        Ok(())
    }

    /// Get sorted list of file paths relative to vault root.
    pub fn file_list(&self) -> Vec<PathBuf> {
        let mut paths: Vec<_> = self.files.keys().cloned().collect();
        paths.sort();
        paths
    }

    /// Build a tree structure for the file browser.
    pub fn file_tree(&self) -> FileTreeNode {
        let root_path = self.root.clone().unwrap_or_default();
        let mut root_node = FileTreeNode {
            name: root_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("vault")
                .to_string(),
            path: root_path.clone(),
            is_dir: true,
            children: Vec::new(),
        };

        for file_path in self.file_list() {
            let rel = file_path
                .strip_prefix(&root_path)
                .unwrap_or(&file_path);
            root_node.insert_path(rel, &file_path);
        }

        root_node.sort_children();
        root_node
    }
}

/// A node in the file tree for the UI browser.
#[derive(Debug, Clone)]
pub struct FileTreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<FileTreeNode>,
}

impl FileTreeNode {
    fn insert_path(&mut self, rel_path: &Path, full_path: &Path) {
        let components: Vec<_> = rel_path.components().collect();
        if components.is_empty() {
            return;
        }

        let first = components[0]
            .as_os_str()
            .to_str()
            .unwrap_or("")
            .to_string();

        if components.len() == 1 {
            // It's a file
            self.children.push(FileTreeNode {
                name: first,
                path: full_path.to_path_buf(),
                is_dir: false,
                children: Vec::new(),
            });
        } else {
            // Find or create directory node
            let dir_node = self
                .children
                .iter_mut()
                .find(|c| c.is_dir && c.name == first);

            if let Some(dir_node) = dir_node {
                let rest: PathBuf = components[1..].iter().collect();
                dir_node.insert_path(&rest, full_path);
            } else {
                let mut new_dir = FileTreeNode {
                    name: first,
                    path: full_path.parent().unwrap_or(full_path).to_path_buf(),
                    is_dir: true,
                    children: Vec::new(),
                };
                let rest: PathBuf = components[1..].iter().collect();
                new_dir.insert_path(&rest, full_path);
                self.children.push(new_dir);
            }
        }
    }

    fn sort_children(&mut self) {
        self.children.sort_by(|a, b| {
            // Directories first, then alphabetical
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            }
        });
        for child in &mut self.children {
            child.sort_children();
        }
    }
}
