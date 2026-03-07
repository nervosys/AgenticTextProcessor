//! # Generic Tree Structure
//!
//! Recursive tree with typed payloads, DFS/BFS iterators, path queries,
//! diff, merge, prune, and serialisation to JSON / indented text.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Unique node identifier.
pub type NodeId = usize;

/// A node in the tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode<T: Clone> {
    pub id: NodeId,
    pub payload: T,
    pub children: Vec<TreeNode<T>>,
}

impl<T: Clone + fmt::Display> fmt::Display for TreeNode<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.payload)
    }
}

impl<T: Clone> TreeNode<T> {
    /// Create a leaf node.
    pub fn leaf(id: NodeId, payload: T) -> Self {
        Self {
            id,
            payload,
            children: Vec::new(),
        }
    }

    /// Create a branch node with children.
    pub fn branch(id: NodeId, payload: T, children: Vec<TreeNode<T>>) -> Self {
        Self {
            id,
            payload,
            children,
        }
    }

    /// Whether this is a leaf (no children).
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Total number of nodes in this subtree (including self).
    pub fn size(&self) -> usize {
        1 + self.children.iter().map(|c| c.size()).sum::<usize>()
    }

    /// Maximum depth of this subtree (1 for a leaf).
    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            1
        } else {
            1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
        }
    }

    /// Find a node by id (DFS).
    pub fn find(&self, target: NodeId) -> Option<&TreeNode<T>> {
        if self.id == target {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find(target) {
                return Some(found);
            }
        }
        None
    }

    /// Find a node by id (mutable, DFS).
    pub fn find_mut(&mut self, target: NodeId) -> Option<&mut TreeNode<T>> {
        if self.id == target {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(target) {
                return Some(found);
            }
        }
        None
    }

    /// Add a child to the given parent node.
    pub fn add_child(&mut self, parent_id: NodeId, child: TreeNode<T>) -> bool {
        if let Some(parent) = self.find_mut(parent_id) {
            parent.children.push(child);
            true
        } else {
            false
        }
    }

    /// Remove a node by id (and its subtree). Returns the removed subtree.
    pub fn remove(&mut self, target: NodeId) -> Option<TreeNode<T>> {
        let pos = self.children.iter().position(|c| c.id == target);
        if let Some(i) = pos {
            return Some(self.children.remove(i));
        }
        for child in &mut self.children {
            if let Some(removed) = child.remove(target) {
                return Some(removed);
            }
        }
        None
    }

    /// Collect the path from root to the node with given id.
    pub fn path_to(&self, target: NodeId) -> Option<Vec<NodeId>> {
        if self.id == target {
            return Some(vec![self.id]);
        }
        for child in &self.children {
            if let Some(mut path) = child.path_to(target) {
                path.insert(0, self.id);
                return Some(path);
            }
        }
        None
    }

    /// Collect all leaf payloads.
    pub fn leaves(&self) -> Vec<&T> {
        if self.is_leaf() {
            return vec![&self.payload];
        }
        self.children.iter().flat_map(|c| c.leaves()).collect()
    }

    /// Map a function over every payload, producing a new tree.
    pub fn map<U: Clone, F: Fn(&T) -> U + Copy>(&self, f: F) -> TreeNode<U> {
        TreeNode {
            id: self.id,
            payload: f(&self.payload),
            children: self.children.iter().map(|c| c.map(f)).collect(),
        }
    }

    /// Prune: remove all nodes at depth > max_depth.
    pub fn prune(&mut self, max_depth: usize) {
        if max_depth <= 1 {
            self.children.clear();
        } else {
            for child in &mut self.children {
                child.prune(max_depth - 1);
            }
        }
    }

    /// Retain only children (recursively) matching a predicate.
    pub fn filter<F: Fn(&T) -> bool + Copy>(&mut self, pred: F) {
        self.children.retain(|c| pred(&c.payload));
        for child in &mut self.children {
            child.filter(pred);
        }
    }
}

// ---------------------------------------------------------------------------
// Iterators
// ---------------------------------------------------------------------------

/// DFS pre-order iterator.
pub struct DfsIter<'a, T: Clone> {
    stack: Vec<&'a TreeNode<T>>,
}

impl<'a, T: Clone> Iterator for DfsIter<'a, T> {
    type Item = &'a TreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        // Push children in reverse so leftmost is visited first
        for child in node.children.iter().rev() {
            self.stack.push(child);
        }
        Some(node)
    }
}

/// BFS (level-order) iterator.
pub struct BfsIter<'a, T: Clone> {
    queue: VecDeque<&'a TreeNode<T>>,
}

impl<'a, T: Clone> Iterator for BfsIter<'a, T> {
    type Item = &'a TreeNode<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let node = self.queue.pop_front()?;
        for child in &node.children {
            self.queue.push_back(child);
        }
        Some(node)
    }
}

/// Get a DFS iterator.
pub fn dfs<T: Clone>(root: &TreeNode<T>) -> DfsIter<'_, T> {
    DfsIter {
        stack: vec![root],
    }
}

/// Get a BFS iterator.
pub fn bfs<T: Clone>(root: &TreeNode<T>) -> BfsIter<'_, T> {
    let mut queue = VecDeque::new();
    queue.push_back(root);
    BfsIter { queue }
}

// ---------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------

/// A difference between two trees.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeDiff {
    Added(NodeId),
    Removed(NodeId),
    Changed(NodeId),
}

/// Diff two trees by id. Payloads compared via Debug representation.
pub fn diff<T: Clone + fmt::Debug>(a: &TreeNode<T>, b: &TreeNode<T>) -> Vec<TreeDiff> {
    let mut diffs = Vec::new();
    diff_recursive(Some(a), Some(b), &mut diffs);
    diffs
}

fn diff_recursive<T: Clone + fmt::Debug>(
    a: Option<&TreeNode<T>>,
    b: Option<&TreeNode<T>>,
    out: &mut Vec<TreeDiff>,
) {
    match (a, b) {
        (Some(a_node), Some(b_node)) => {
            if format!("{:?}", a_node.payload) != format!("{:?}", b_node.payload) {
                out.push(TreeDiff::Changed(a_node.id));
            }
            // Match children by id
            let a_ids: Vec<NodeId> = a_node.children.iter().map(|c| c.id).collect();
            let b_ids: Vec<NodeId> = b_node.children.iter().map(|c| c.id).collect();

            for ac in &a_node.children {
                let bc = b_node.children.iter().find(|c| c.id == ac.id);
                if bc.is_none() {
                    out.push(TreeDiff::Removed(ac.id));
                } else {
                    diff_recursive(Some(ac), bc, out);
                }
            }
            for bc in &b_node.children {
                if !a_ids.contains(&bc.id) {
                    out.push(TreeDiff::Added(bc.id));
                }
            }
            let _ = b_ids; // suppress unused warning
        }
        (Some(a_node), None) => {
            out.push(TreeDiff::Removed(a_node.id));
        }
        (None, Some(b_node)) => {
            out.push(TreeDiff::Added(b_node.id));
        }
        (None, None) => {}
    }
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge tree `other` into `base`. Nodes with matching ids keep `other`'s
/// payload; new nodes in `other` are added.
pub fn merge<T: Clone>(base: &mut TreeNode<T>, other: &TreeNode<T>) {
    base.payload = other.payload.clone();
    for oc in &other.children {
        if let Some(bc) = base.children.iter_mut().find(|c| c.id == oc.id) {
            merge(bc, oc);
        } else {
            base.children.push(oc.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

/// Render tree as indented text.
pub fn to_indented<T: Clone + fmt::Display>(root: &TreeNode<T>, indent: usize) -> String {
    let mut out = String::new();
    render_indented(root, 0, indent, &mut out);
    out
}

fn render_indented<T: Clone + fmt::Display>(
    node: &TreeNode<T>,
    level: usize,
    indent: usize,
    out: &mut String,
) {
    for _ in 0..level * indent {
        out.push(' ');
    }
    out.push_str(&format!("{}\n", node.payload));
    for child in &node.children {
        render_indented(child, level + 1, indent, out);
    }
}

/// Serialise to JSON (requires Serialize on T).
pub fn to_json<T: Clone + Serialize>(root: &TreeNode<T>) -> String {
    serde_json::to_string_pretty(root).unwrap_or_default()
}

/// Deserialise from JSON (requires DeserializeOwned on T).
pub fn from_json<T: Clone + for<'de> Deserialize<'de>>(json: &str) -> Option<TreeNode<T>> {
    serde_json::from_str(json).ok()
}

// ---------------------------------------------------------------------------
// Builder helper
// ---------------------------------------------------------------------------

/// Simple auto-incrementing id allocator.
pub struct IdGen(NodeId);

impl IdGen {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn next_id(&mut self) -> NodeId {
        let id = self.0;
        self.0 += 1;
        id
    }
}

impl Default for IdGen {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tree() -> TreeNode<String> {
        TreeNode::branch(
            0,
            "root".into(),
            vec![
                TreeNode::branch(
                    1,
                    "a".into(),
                    vec![
                        TreeNode::leaf(3, "a1".into()),
                        TreeNode::leaf(4, "a2".into()),
                    ],
                ),
                TreeNode::branch(
                    2,
                    "b".into(),
                    vec![TreeNode::leaf(5, "b1".into())],
                ),
            ],
        )
    }

    #[test]
    fn test_size() {
        assert_eq!(sample_tree().size(), 6);
    }

    #[test]
    fn test_depth() {
        assert_eq!(sample_tree().depth(), 3);
        assert_eq!(TreeNode::leaf(0, "x".to_string()).depth(), 1);
    }

    #[test]
    fn test_find() {
        let tree = sample_tree();
        assert!(tree.find(3).is_some());
        assert_eq!(tree.find(3).unwrap().payload, "a1");
        assert!(tree.find(99).is_none());
    }

    #[test]
    fn test_path_to() {
        let tree = sample_tree();
        let path = tree.path_to(4).unwrap();
        assert_eq!(path, vec![0, 1, 4]);
    }

    #[test]
    fn test_leaves() {
        let tree = sample_tree();
        let leaves = tree.leaves();
        assert_eq!(leaves.len(), 3);
    }

    #[test]
    fn test_dfs_order() {
        let tree = sample_tree();
        let ids: Vec<NodeId> = dfs(&tree).map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 1, 3, 4, 2, 5]);
    }

    #[test]
    fn test_bfs_order() {
        let tree = sample_tree();
        let ids: Vec<NodeId> = bfs(&tree).map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_add_child() {
        let mut tree = sample_tree();
        let child = TreeNode::leaf(6, "new".into());
        assert!(tree.add_child(2, child));
        assert_eq!(tree.find(2).unwrap().children.len(), 2);
    }

    #[test]
    fn test_remove() {
        let mut tree = sample_tree();
        let removed = tree.remove(1);
        assert!(removed.is_some());
        assert_eq!(tree.size(), 3); // root + b + b1
    }

    #[test]
    fn test_map() {
        let tree = sample_tree();
        let upper = tree.map(|s| s.to_uppercase());
        assert_eq!(upper.payload, "ROOT");
        assert_eq!(upper.find(3).unwrap().payload, "A1");
    }

    #[test]
    fn test_prune() {
        let mut tree = sample_tree();
        tree.prune(2);
        assert_eq!(tree.depth(), 2);
        // Children of root are present but grandchildren are gone
        assert!(tree.find(1).unwrap().is_leaf());
    }

    #[test]
    fn test_diff() {
        let a = sample_tree();
        let mut b = sample_tree();
        b.find_mut(3).unwrap().payload = "changed".into();
        b.children[1].children.push(TreeNode::leaf(6, "new".into()));
        let diffs = diff(&a, &b);
        assert!(diffs.contains(&TreeDiff::Changed(3)));
        assert!(diffs.contains(&TreeDiff::Added(6)));
    }

    #[test]
    fn test_merge() {
        let mut base = sample_tree();
        let mut other = sample_tree();
        other.find_mut(3).unwrap().payload = "merged".into();
        other.children[1].children.push(TreeNode::leaf(6, "extra".into()));
        merge(&mut base, &other);
        assert_eq!(base.find(3).unwrap().payload, "merged");
        assert!(base.find(6).is_some());
    }

    #[test]
    fn test_to_indented() {
        let tree = sample_tree();
        let text = to_indented(&tree, 2);
        assert!(text.contains("root\n"));
        assert!(text.contains("  a\n"));
        assert!(text.contains("    a1\n"));
    }

    #[test]
    fn test_json_roundtrip() {
        let tree = sample_tree();
        let json = to_json(&tree);
        let restored: TreeNode<String> = from_json(&json).unwrap();
        assert_eq!(restored.size(), tree.size());
        assert_eq!(restored.payload, "root");
    }

    #[test]
    fn test_id_gen() {
        let mut gen = IdGen::new();
        assert_eq!(gen.next_id(), 0);
        assert_eq!(gen.next_id(), 1);
        assert_eq!(gen.next_id(), 2);
    }
}
