use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub expanded: bool,
    pub children: Vec<FileNode>,
    pub depth: usize,
}

impl FileNode {
    pub fn from_path(path: PathBuf, depth: usize) -> Self {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let is_dir = path.is_dir();
        Self {
            path,
            name,
            is_dir,
            expanded: false,
            children: Vec::new(),
            depth,
        }
    }

    pub fn toggle_expand(&mut self) {
        if self.is_dir {
            if self.expanded {
                self.expanded = false;
                self.children.clear();
            } else {
                self.expanded = true;
                self.load_children();
            }
        }
    }

    pub fn load_children(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.path) {
            let mut files: Vec<FileNode> = entries
                .filter_map(|res| res.ok())
                .map(|e| FileNode::from_path(e.path(), self.depth + 1))
                .filter(|node| !node.name.starts_with('.'))
                .collect();
            
            files.sort_by(|a, b| {
                match (a.is_dir, b.is_dir) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });
            
            self.children = files;
        }
    }
}

pub struct VisibleItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

pub fn flatten_node(node: &FileNode, visible_items: &mut Vec<VisibleItem>) {
    visible_items.push(VisibleItem {
        name: node.name.clone(),
        path: node.path.clone(),
        is_dir: node.is_dir,
        depth: node.depth,
        expanded: node.expanded,
    });

    if node.expanded {
        for child in &node.children {
            flatten_node(child, visible_items);
        }
    }
}

pub fn toggle_node_recursive(nodes: &mut Vec<FileNode>, target: &PathBuf) -> bool {
    for node in nodes.iter_mut() {
        if &node.path == target {
            node.toggle_expand();
            return true;
        }
        if node.expanded {
            if toggle_node_recursive(&mut node.children, target) {
                return true;
            }
        }
    }
    false
}
