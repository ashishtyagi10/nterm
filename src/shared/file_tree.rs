// File tree data structures and operations

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_file_node_from_path() {
        let path = PathBuf::from("/tmp/test_dir");
        let node = FileNode::from_path(path.clone(), 0);

        assert_eq!(node.path, path);
        assert_eq!(node.name, "test_dir");
        assert_eq!(node.depth, 0);
        assert_eq!(node.expanded, false);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_flatten_node() {
        let mut root = FileNode {
            path: PathBuf::from("root"),
            name: "root".to_string(),
            is_dir: true,
            expanded: true,
            children: vec![],
            depth: 0,
        };

        let child1 = FileNode {
            path: PathBuf::from("root/child1"),
            name: "child1".to_string(),
            is_dir: false,
            expanded: false,
            children: vec![],
            depth: 1,
        };

        let child2 = FileNode {
            path: PathBuf::from("root/child2"),
            name: "child2".to_string(),
            is_dir: true,
            expanded: false,
            children: vec![],
            depth: 1,
        };

        root.children.push(child1);
        root.children.push(child2);

        let mut visible = Vec::new();
        flatten_node(&root, &mut visible);

        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].name, "root");
        assert_eq!(visible[1].name, "child1");
        assert_eq!(visible[2].name, "child2");
    }

    #[test]
    fn test_flatten_node_collapsed() {
         let mut root = FileNode {
            path: PathBuf::from("root"),
            name: "root".to_string(),
            is_dir: true,
            expanded: false, // Collapsed
            children: vec![],
            depth: 0,
        };

        let child1 = FileNode {
            path: PathBuf::from("root/child1"),
            name: "child1".to_string(),
            is_dir: false,
            expanded: false,
            children: vec![],
            depth: 1,
        };

        root.children.push(child1);

        let mut visible = Vec::new();
        flatten_node(&root, &mut visible);

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].name, "root");
    }
}
