//! File/module dependency graph.
//!
//! Build import/dependency graphs from source files, detect cycles,
//! compute transitive closures, topological sort, and export as DOT
//! for visualization.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A node in the dependency graph (represents a file or module).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepNode {
    /// Unique identifier (typically a file path or module name).
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Tags / metadata (e.g. `["crate", "test"]`).
    pub tags: Vec<String>,
}

/// An edge in the dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepEdge {
    /// Source node id.
    pub from: String,
    /// Target node id.
    pub to: String,
    /// Edge label (e.g. "use", "import", "include").
    pub kind: String,
}

/// Statistics about the dependency graph.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub max_depth: usize,
    pub root_nodes: usize,
    pub leaf_nodes: usize,
    pub has_cycles: bool,
    pub cycle_count: usize,
}

/// Import kind detected in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportKind {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    CInclude,
    Unknown,
}

/// A detected import statement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportStatement {
    pub kind: ImportKind,
    pub source_file: String,
    pub imported_path: String,
    pub line_number: usize,
}

// ---------------------------------------------------------------------------
// Import extraction (regex-based)
// ---------------------------------------------------------------------------

/// Extract import/use statements from source text using regex patterns.
pub fn extract_imports(source: &str, file_path: &str) -> Vec<ImportStatement> {
    let mut imports = Vec::new();
    let kind = detect_import_kind(file_path);

    let patterns: Vec<(&str, ImportKind)> = vec![
        // Rust: use crate::foo, mod foo, use foo::bar
        (r"(?m)^\s*(?:pub\s+)?use\s+([\w:]+)", ImportKind::Rust),
        (r"(?m)^\s*(?:pub\s+)?mod\s+(\w+)", ImportKind::Rust),
        // Python: import foo, from foo import bar
        (r"(?m)^\s*import\s+([\w.]+)", ImportKind::Python),
        (r"(?m)^\s*from\s+([\w.]+)\s+import", ImportKind::Python),
        // JavaScript/TypeScript: import ... from 'path', require('path')
        (
            r#"(?m)(?:import\s+.*?\s+from\s+['"](.+?)['"])"#,
            ImportKind::JavaScript,
        ),
        (
            r#"(?m)require\(\s*['"](.+?)['"]\s*\)"#,
            ImportKind::JavaScript,
        ),
        // C/C++: #include "file" or #include <file>
        (r#"(?m)^\s*#include\s+["<](.+?)[">]"#, ImportKind::CInclude),
    ];

    for (pattern, pat_kind) in &patterns {
        if kind != ImportKind::Unknown && kind != *pat_kind {
            continue;
        }
        let Ok(re) = regex::Regex::new(pattern) else {
            continue;
        };
        for (line_idx, line) in source.lines().enumerate() {
            for caps in re.captures_iter(line) {
                if let Some(m) = caps.get(1) {
                    imports.push(ImportStatement {
                        kind: *pat_kind,
                        source_file: file_path.to_string(),
                        imported_path: m.as_str().to_string(),
                        line_number: line_idx + 1,
                    });
                }
            }
        }
    }

    imports
}

fn detect_import_kind(path: &str) -> ImportKind {
    if path.ends_with(".rs") {
        ImportKind::Rust
    } else if path.ends_with(".py") {
        ImportKind::Python
    } else if path.ends_with(".js") || path.ends_with(".jsx") || path.ends_with(".mjs") {
        ImportKind::JavaScript
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        ImportKind::TypeScript
    } else if path.ends_with(".c")
        || path.ends_with(".h")
        || path.ends_with(".cpp")
        || path.ends_with(".hpp")
    {
        ImportKind::CInclude
    } else {
        ImportKind::Unknown
    }
}

// ---------------------------------------------------------------------------
// DepGraph
// ---------------------------------------------------------------------------

/// A directed dependency graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepGraph {
    nodes: BTreeMap<String, DepNode>,
    edges: Vec<DepEdge>,
    /// Adjacency list: from → [to].
    adjacency: BTreeMap<String, BTreeSet<String>>,
    /// Reverse adjacency: to → [from].
    reverse_adj: BTreeMap<String, BTreeSet<String>>,
}

impl DepGraph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            adjacency: BTreeMap::new(),
            reverse_adj: BTreeMap::new(),
        }
    }

    /// Build a graph from import statements.
    pub fn from_imports(imports: &[ImportStatement]) -> Self {
        let mut g = Self::new();
        for imp in imports {
            g.add_node_if_missing(&imp.source_file, &[]);
            g.add_node_if_missing(&imp.imported_path, &[]);
            g.add_edge(&imp.source_file, &imp.imported_path, &format!("{:?}", imp.kind));
        }
        g
    }

    /// Add a node if it doesn't exist.
    pub fn add_node_if_missing(&mut self, id: &str, tags: &[&str]) {
        if !self.nodes.contains_key(id) {
            self.nodes.insert(
                id.to_string(),
                DepNode {
                    id: id.to_string(),
                    label: id.to_string(),
                    tags: tags.iter().map(|s| s.to_string()).collect(),
                },
            );
        }
    }

    /// Add a node.
    pub fn add_node(&mut self, node: DepNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// Add a directed edge from → to.
    pub fn add_edge(&mut self, from: &str, to: &str, kind: &str) {
        self.edges.push(DepEdge {
            from: from.to_string(),
            to: to.to_string(),
            kind: kind.to_string(),
        });
        self.adjacency
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
        self.reverse_adj
            .entry(to.to_string())
            .or_default()
            .insert(from.to_string());
    }

    /// Get all node IDs.
    pub fn node_ids(&self) -> Vec<&str> {
        self.nodes.keys().map(|s| s.as_str()).collect()
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: &str) -> Option<&DepNode> {
        self.nodes.get(id)
    }

    /// Get all direct dependencies of a node.
    pub fn dependencies(&self, id: &str) -> Vec<&str> {
        self.adjacency
            .get(id)
            .map(|s| s.iter().map(|x| x.as_str()).collect())
            .unwrap_or_default()
    }

    /// Get all direct dependents (reverse dependencies) of a node.
    pub fn dependents(&self, id: &str) -> Vec<&str> {
        self.reverse_adj
            .get(id)
            .map(|s| s.iter().map(|x| x.as_str()).collect())
            .unwrap_or_default()
    }

    /// Root nodes (no incoming edges).
    pub fn roots(&self) -> Vec<&str> {
        self.nodes
            .keys()
            .filter(|id| !self.reverse_adj.contains_key(id.as_str()))
            .map(|s| s.as_str())
            .collect()
    }

    /// Leaf nodes (no outgoing edges).
    pub fn leaves(&self) -> Vec<&str> {
        self.nodes
            .keys()
            .filter(|id| !self.adjacency.contains_key(id.as_str()))
            .map(|s| s.as_str())
            .collect()
    }

    /// Compute the transitive closure (all reachable nodes) from a source.
    pub fn transitive_closure(&self, from: &str) -> BTreeSet<String> {
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(from.to_string());

        while let Some(node) = queue.pop_front() {
            if !visited.insert(node.clone()) {
                continue;
            }
            if let Some(deps) = self.adjacency.get(&node) {
                for dep in deps {
                    if !visited.contains(dep) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
        visited.remove(from);
        visited
    }

    /// Detect all cycles in the graph. Returns sets of nodes forming cycles.
    pub fn detect_cycles(&self) -> Vec<Vec<String>> {
        let mut visited = BTreeSet::new();
        let mut on_stack = BTreeSet::new();
        let mut cycles = Vec::new();

        for node in self.nodes.keys() {
            if !visited.contains(node) {
                let mut path = Vec::new();
                self.dfs_cycle(node, &mut visited, &mut on_stack, &mut path, &mut cycles);
            }
        }
        cycles
    }

    fn dfs_cycle(
        &self,
        node: &str,
        visited: &mut BTreeSet<String>,
        on_stack: &mut BTreeSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        on_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(deps) = self.adjacency.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    self.dfs_cycle(dep, visited, on_stack, path, cycles);
                } else if on_stack.contains(dep) {
                    // Found a cycle — extract it from path
                    if let Some(start) = path.iter().position(|n| n == dep) {
                        let cycle: Vec<String> = path[start..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        on_stack.remove(node);
        path.pop();
    }

    /// Topological sort (returns None if there are cycles).
    pub fn topological_sort(&self) -> Option<Vec<String>> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        for id in self.nodes.keys() {
            in_degree.entry(id.as_str()).or_insert(0);
        }
        for edge in &self.edges {
            *in_degree.entry(edge.to.as_str()).or_insert(0) += 1;
        }

        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.to_string());
            if let Some(deps) = self.adjacency.get(node) {
                for dep in deps {
                    if let Some(d) = in_degree.get_mut(dep.as_str()) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push_back(dep.as_str());
                        }
                    }
                }
            }
        }

        if order.len() == self.nodes.len() {
            Some(order)
        } else {
            None
        }
    }

    /// Compute the depth (longest path from any root) for each node.
    pub fn compute_depths(&self) -> BTreeMap<String, usize> {
        let mut depths: BTreeMap<String, usize> = BTreeMap::new();
        let Some(order) = self.topological_sort() else {
            return depths;
        };
        for node in &order {
            let depth = self
                .reverse_adj
                .get(node)
                .map(|parents| {
                    parents
                        .iter()
                        .filter_map(|p| depths.get(p))
                        .max()
                        .map(|d| d + 1)
                        .unwrap_or(0)
                })
                .unwrap_or(0);
            depths.insert(node.clone(), depth);
        }
        depths
    }

    /// Compute graph statistics.
    pub fn stats(&self) -> GraphStats {
        let cycles = self.detect_cycles();
        let depths = self.compute_depths();
        GraphStats {
            total_nodes: self.nodes.len(),
            total_edges: self.edges.len(),
            max_depth: depths.values().copied().max().unwrap_or(0),
            root_nodes: self.roots().len(),
            leaf_nodes: self.leaves().len(),
            has_cycles: !cycles.is_empty(),
            cycle_count: cycles.len(),
        }
    }

    /// Export the graph in Graphviz DOT format.
    pub fn to_dot(&self) -> String {
        let mut out = String::from("digraph dependencies {\n");
        out.push_str("    rankdir=LR;\n");
        out.push_str("    node [shape=box, style=rounded];\n\n");

        for node in self.nodes.values() {
            let label = node.label.replace('"', "\\\"");
            out.push_str(&format!("    \"{}\" [label=\"{}\"];\n", node.id, label));
        }

        out.push('\n');
        for edge in &self.edges {
            out.push_str(&format!(
                "    \"{}\" -> \"{}\" [label=\"{}\"];\n",
                edge.from, edge.to, edge.kind
            ));
        }

        out.push_str("}\n");
        out
    }
}

impl Default for DepGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_graph() -> DepGraph {
        let mut g = DepGraph::new();
        g.add_node_if_missing("a", &[]);
        g.add_node_if_missing("b", &[]);
        g.add_node_if_missing("c", &[]);
        g.add_node_if_missing("d", &[]);
        g.add_edge("a", "b", "use");
        g.add_edge("a", "c", "use");
        g.add_edge("b", "d", "use");
        g.add_edge("c", "d", "use");
        g
    }

    #[test]
    fn test_dependencies() {
        let g = simple_graph();
        let deps = g.dependencies("a");
        assert!(deps.contains(&"b"));
        assert!(deps.contains(&"c"));
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_dependents() {
        let g = simple_graph();
        let deps = g.dependents("d");
        assert!(deps.contains(&"b"));
        assert!(deps.contains(&"c"));
    }

    #[test]
    fn test_roots_and_leaves() {
        let g = simple_graph();
        assert_eq!(g.roots(), vec!["a"]);
        assert_eq!(g.leaves(), vec!["d"]);
    }

    #[test]
    fn test_transitive_closure() {
        let g = simple_graph();
        let closure = g.transitive_closure("a");
        assert!(closure.contains("b"));
        assert!(closure.contains("c"));
        assert!(closure.contains("d"));
        assert_eq!(closure.len(), 3);
    }

    #[test]
    fn test_no_cycles() {
        let g = simple_graph();
        let cycles = g.detect_cycles();
        assert!(cycles.is_empty());
    }

    #[test]
    fn test_cycle_detection() {
        let mut g = DepGraph::new();
        g.add_node_if_missing("a", &[]);
        g.add_node_if_missing("b", &[]);
        g.add_node_if_missing("c", &[]);
        g.add_edge("a", "b", "use");
        g.add_edge("b", "c", "use");
        g.add_edge("c", "a", "use");
        let cycles = g.detect_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn test_topological_sort() {
        let g = simple_graph();
        let order = g.topological_sort().unwrap();
        assert_eq!(order[0], "a");
        let d_pos = order.iter().position(|x| x == "d").unwrap();
        let b_pos = order.iter().position(|x| x == "b").unwrap();
        assert!(b_pos < d_pos);
    }

    #[test]
    fn test_topological_sort_with_cycle() {
        let mut g = DepGraph::new();
        g.add_node_if_missing("x", &[]);
        g.add_node_if_missing("y", &[]);
        g.add_edge("x", "y", "use");
        g.add_edge("y", "x", "use");
        assert!(g.topological_sort().is_none());
    }

    #[test]
    fn test_compute_depths() {
        let g = simple_graph();
        let depths = g.compute_depths();
        assert_eq!(depths.get("a"), Some(&0));
        assert_eq!(depths.get("d"), Some(&2));
    }

    #[test]
    fn test_stats() {
        let g = simple_graph();
        let s = g.stats();
        assert_eq!(s.total_nodes, 4);
        assert_eq!(s.total_edges, 4);
        assert_eq!(s.root_nodes, 1);
        assert_eq!(s.leaf_nodes, 1);
        assert!(!s.has_cycles);
    }

    #[test]
    fn test_dot_export() {
        let g = simple_graph();
        let dot = g.to_dot();
        assert!(dot.contains("digraph dependencies"));
        assert!(dot.contains("\"a\" -> \"b\""));
        assert!(dot.contains("\"c\" -> \"d\""));
    }

    #[test]
    fn test_from_imports() {
        let imports = vec![
            ImportStatement {
                kind: ImportKind::Rust,
                source_file: "main.rs".into(),
                imported_path: "config".into(),
                line_number: 1,
            },
            ImportStatement {
                kind: ImportKind::Rust,
                source_file: "main.rs".into(),
                imported_path: "engine".into(),
                line_number: 2,
            },
            ImportStatement {
                kind: ImportKind::Rust,
                source_file: "engine".into(),
                imported_path: "config".into(),
                line_number: 1,
            },
        ];
        let g = DepGraph::from_imports(&imports);
        assert_eq!(g.node_ids().len(), 3);
        assert!(g.dependencies("main.rs").contains(&"config"));
        assert!(g.dependencies("main.rs").contains(&"engine"));
    }

    #[test]
    fn test_extract_rust_imports() {
        let src = "use std::io;\npub mod config;\nuse crate::engine::grep;\n";
        let imports = extract_imports(src, "lib.rs");
        assert!(imports.len() >= 3);
        assert!(imports.iter().any(|i| i.imported_path == "std::io"));
        assert!(imports.iter().any(|i| i.imported_path == "config"));
    }

    #[test]
    fn test_extract_python_imports() {
        let src = "import os\nfrom pathlib import Path\nimport json\n";
        let imports = extract_imports(src, "main.py");
        assert_eq!(imports.len(), 3);
        assert!(imports.iter().all(|i| i.kind == ImportKind::Python));
    }

    #[test]
    fn test_empty_graph() {
        let g = DepGraph::new();
        assert!(g.node_ids().is_empty());
        assert!(g.roots().is_empty());
        assert!(g.detect_cycles().is_empty());
        assert_eq!(g.topological_sort(), Some(vec![]));
    }
}
