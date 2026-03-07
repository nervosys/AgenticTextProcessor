//! File browser panel — directory tree for the TUI sidebar.

use std::path::{Path, PathBuf};

/// A node in the file tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub expanded: bool,
    pub depth: usize,
    pub children: Vec<FileNode>,
}

impl FileNode {
    /// Create a node from a path.
    pub fn from_path(path: &Path, depth: usize) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        Self {
            name,
            path: path.to_path_buf(),
            is_dir: path.is_dir(),
            expanded: depth == 0, // Root starts expanded
            depth,
            children: Vec::new(),
        }
    }

    /// Load immediate children for a directory node.
    pub fn load_children(&mut self) {
        if !self.is_dir || !self.children.is_empty() {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(&self.path) {
            let mut dirs = Vec::new();
            let mut files = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                let node = FileNode::from_path(&path, self.depth + 1);
                if node.is_dir {
                    dirs.push(node);
                } else {
                    files.push(node);
                }
            }
            dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            self.children = dirs;
            self.children.extend(files);
        }
    }

    /// Flatten the visible tree into a list of (depth, node) for rendering.
    pub fn flatten_visible(&self) -> Vec<(usize, &FileNode)> {
        let mut out = Vec::new();
        self.flatten_into(&mut out);
        out
    }

    fn flatten_into<'a>(&'a self, out: &mut Vec<(usize, &'a FileNode)>) {
        out.push((self.depth, self));
        if self.is_dir && self.expanded {
            for child in &self.children {
                child.flatten_into(out);
            }
        }
    }
}

/// File browser state.
pub struct FileBrowser {
    pub root: FileNode,
    pub selected: usize,
    pub visible: bool,
}

impl FileBrowser {
    pub fn new(root_path: &Path) -> Self {
        let mut root = FileNode::from_path(root_path, 0);
        root.load_children();
        Self {
            root,
            selected: 0,
            visible: true,
        }
    }

    /// Get the flat list of visible nodes.
    pub fn visible_items(&self) -> Vec<(usize, &FileNode)> {
        self.root.flatten_visible()
    }

    /// Toggle expand/collapse on the selected directory.
    pub fn toggle_expand(&mut self) {
        let items = self.root.flatten_visible();
        if let Some((_, node)) = items.get(self.selected) {
            if node.is_dir {
                let path = node.path.clone();
                self.toggle_node(&path);
            }
        }
    }

    fn toggle_node(&mut self, target: &Path) {
        Self::toggle_recursive(&mut self.root, target);
    }

    fn toggle_recursive(node: &mut FileNode, target: &Path) {
        if node.path == target {
            node.expanded = !node.expanded;
            if node.expanded {
                node.load_children();
            }
            return;
        }
        for child in &mut node.children {
            Self::toggle_recursive(child, target);
        }
    }

    /// Get the selected file path (if it's a file).
    #[allow(dead_code)]
    pub fn selected_file(&self) -> Option<PathBuf> {
        let items = self.root.flatten_visible();
        items.get(self.selected).and_then(|(_, node)| {
            if !node.is_dir {
                Some(node.path.clone())
            } else {
                None
            }
        })
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let count = self.root.flatten_visible().len();
        if self.selected + 1 < count {
            self.selected += 1;
        }
    }
}
