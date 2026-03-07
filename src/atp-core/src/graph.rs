// ---------------------------------------------------------------------------
// graph.rs — Text graph analytics
// ---------------------------------------------------------------------------
//
// Word co-occurrence matrix, sentence similarity graph, TextRank/PageRank
// iteration, degree/closeness centrality, DOT export.
// ---------------------------------------------------------------------------

use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An edge in a text graph.
#[derive(Debug, Clone)]
pub struct Edge {
    /// Source node index.
    pub from: usize,
    /// Target node index.
    pub to: usize,
    /// Edge weight.
    pub weight: f64,
}

/// A text graph with named nodes and weighted edges.
#[derive(Debug, Clone)]
pub struct TextGraph {
    /// Node labels.
    pub nodes: Vec<String>,
    /// Adjacency list (node index → list of (neighbor, weight)).
    pub adjacency: Vec<Vec<(usize, f64)>>,
}

/// Centrality scores for all nodes.
#[derive(Debug, Clone)]
pub struct Centrality {
    /// Degree centrality per node.
    pub degree: Vec<f64>,
    /// Closeness centrality per node.
    pub closeness: Vec<f64>,
}

/// PageRank result.
#[derive(Debug, Clone)]
pub struct RankResult {
    /// Node labels.
    pub nodes: Vec<String>,
    /// PageRank scores (sum ≈ 1.0).
    pub scores: Vec<f64>,
}

// ---------------------------------------------------------------------------
// Co-occurrence
// ---------------------------------------------------------------------------

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

/// Build a word co-occurrence graph with the given window size.
///
/// Two words are connected if they appear within `window` tokens of each other.
pub fn cooccurrence(text: &str, window: usize) -> TextGraph {
    let tokens = tokenize(text);
    let mut word_index: HashMap<String, usize> = HashMap::new();
    let mut nodes: Vec<String> = Vec::new();
    for t in &tokens {
        if !word_index.contains_key(t) {
            word_index.insert(t.clone(), nodes.len());
            nodes.push(t.clone());
        }
    }
    let n = nodes.len();
    let mut weights = vec![vec![0.0f64; n]; n];
    for i in 0..tokens.len() {
        let a = word_index[&tokens[i]];
        for j in (i + 1)..=(i + window).min(tokens.len() - 1) {
            let b = word_index[&tokens[j]];
            if a != b {
                weights[a][b] += 1.0;
                weights[b][a] += 1.0;
            }
        }
    }
    let mut adjacency = vec![Vec::new(); n];
    for i in 0..n {
        for (j, w_j) in weights[i].iter().enumerate() {
            if *w_j > 0.0 {
                adjacency[i].push((j, *w_j));
            }
        }
    }
    TextGraph { nodes, adjacency }
}

// ---------------------------------------------------------------------------
// Sentence similarity graph
// ---------------------------------------------------------------------------

fn sentence_words(s: &str) -> HashSet<String> {
    tokenize(s).into_iter().collect()
}

/// Build a sentence similarity graph using word overlap (Jaccard).
pub fn sentence_graph(text: &str) -> TextGraph {
    let sentences: Vec<&str> = text
        .split(['.', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    let n = sentences.len();
    let word_sets: Vec<HashSet<String>> = sentences.iter().map(|s| sentence_words(s)).collect();
    let nodes: Vec<String> = sentences.iter().map(|s| s.to_string()).collect();
    let mut adjacency = vec![Vec::new(); n];
    for i in 0..n {
        for j in (i + 1)..n {
            let intersection = word_sets[i].intersection(&word_sets[j]).count();
            let union = word_sets[i].union(&word_sets[j]).count();
            if union > 0 {
                let sim = intersection as f64 / union as f64;
                if sim > 0.0 {
                    adjacency[i].push((j, sim));
                    adjacency[j].push((i, sim));
                }
            }
        }
    }
    TextGraph { nodes, adjacency }
}

// ---------------------------------------------------------------------------
// PageRank
// ---------------------------------------------------------------------------

/// Run PageRank on a `TextGraph`.
///
/// `damping` is typically 0.85, `iterations` typically 20–50.
pub fn pagerank(graph: &TextGraph, damping: f64, iterations: usize) -> RankResult {
    let n = graph.nodes.len();
    if n == 0 {
        return RankResult {
            nodes: Vec::new(),
            scores: Vec::new(),
        };
    }
    let mut scores = vec![1.0 / n as f64; n];
    // Build weighted out-degree
    let out_weights: Vec<f64> = graph
        .adjacency
        .iter()
        .map(|adj| adj.iter().map(|(_, w)| w).sum::<f64>())
        .collect();

    for _ in 0..iterations {
        let mut new_scores = vec![(1.0 - damping) / n as f64; n];
        for i in 0..n {
            if out_weights[i] > 0.0 {
                for &(j, w) in &graph.adjacency[i] {
                    new_scores[j] += damping * scores[i] * w / out_weights[i];
                }
            }
        }
        scores = new_scores;
    }
    RankResult {
        nodes: graph.nodes.clone(),
        scores,
    }
}

/// TextRank — alias for PageRank on a text graph.
pub fn textrank(text: &str, damping: f64, iterations: usize) -> RankResult {
    let g = sentence_graph(text);
    pagerank(&g, damping, iterations)
}

// ---------------------------------------------------------------------------
// Centrality
// ---------------------------------------------------------------------------

/// Compute degree and closeness centrality for all nodes.
pub fn centrality(graph: &TextGraph) -> Centrality {
    let n = graph.nodes.len();
    if n == 0 {
        return Centrality {
            degree: Vec::new(),
            closeness: Vec::new(),
        };
    }
    // Degree centrality: sum of weights / (n-1)
    let degree: Vec<f64> = graph
        .adjacency
        .iter()
        .map(|adj| {
            let sum: f64 = adj.iter().map(|(_, w)| w).sum();
            if n > 1 {
                sum / (n - 1) as f64
            } else {
                0.0
            }
        })
        .collect();

    // Closeness centrality via BFS-like shortest paths (unweighted for simplicity)
    let closeness: Vec<f64> = (0..n)
        .map(|start| {
            let mut dist = vec![usize::MAX; n];
            dist[start] = 0;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(start);
            while let Some(u) = queue.pop_front() {
                for &(v, _) in &graph.adjacency[u] {
                    if dist[v] == usize::MAX {
                        dist[v] = dist[u] + 1;
                        queue.push_back(v);
                    }
                }
            }
            let reachable: Vec<usize> = dist
                .iter()
                .copied()
                .filter(|&d| d != usize::MAX && d > 0)
                .collect();
            if reachable.is_empty() {
                0.0
            } else {
                let total_dist: usize = reachable.iter().sum();
                reachable.len() as f64 / total_dist as f64
            }
        })
        .collect();

    Centrality { degree, closeness }
}

// ---------------------------------------------------------------------------
// Edges list
// ---------------------------------------------------------------------------

/// Extract all edges from the graph.
pub fn edges(graph: &TextGraph) -> Vec<Edge> {
    let mut result = Vec::new();
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for (i, adj) in graph.adjacency.iter().enumerate() {
        for &(j, w) in adj {
            let key = (i.min(j), i.max(j));
            if seen.insert(key) {
                result.push(Edge {
                    from: i,
                    to: j,
                    weight: w,
                });
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// DOT export
// ---------------------------------------------------------------------------

/// Export graph as Graphviz DOT format.
pub fn to_dot(graph: &TextGraph, name: &str) -> String {
    let mut dot = format!("graph \"{name}\" {{\n");
    // Nodes
    for (i, label) in graph.nodes.iter().enumerate() {
        let short: String = label.chars().take(30).collect();
        dot.push_str(&format!("  n{i} [label=\"{short}\"];\n"));
    }
    // Edges (undirected, deduplicated)
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for (i, adj) in graph.adjacency.iter().enumerate() {
        for &(j, w) in adj {
            let key = (i.min(j), i.max(j));
            if seen.insert(key) {
                dot.push_str(&format!("  n{i} -- n{j} [weight={w:.2}];\n"));
            }
        }
    }
    dot.push_str("}\n");
    dot
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "The cat sat on the mat. The dog sat on the log. The cat chased the dog.";

    #[test]
    fn test_cooccurrence_nodes() {
        let g = cooccurrence(SAMPLE, 2);
        assert!(!g.nodes.is_empty());
    }

    #[test]
    fn test_cooccurrence_edges() {
        let g = cooccurrence(SAMPLE, 2);
        let e = edges(&g);
        assert!(!e.is_empty());
    }

    #[test]
    fn test_sentence_graph() {
        let g = sentence_graph(SAMPLE);
        assert_eq!(g.nodes.len(), 3);
    }

    #[test]
    fn test_sentence_graph_edges() {
        let g = sentence_graph(SAMPLE);
        let e = edges(&g);
        assert!(!e.is_empty());
    }

    #[test]
    fn test_pagerank() {
        let g = cooccurrence(SAMPLE, 2);
        let r = pagerank(&g, 0.85, 20);
        assert_eq!(r.scores.len(), g.nodes.len());
        let total: f64 = r.scores.iter().sum();
        assert!((total - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_pagerank_empty() {
        let g = TextGraph {
            nodes: Vec::new(),
            adjacency: Vec::new(),
        };
        let r = pagerank(&g, 0.85, 10);
        assert!(r.scores.is_empty());
    }

    #[test]
    fn test_textrank() {
        let r = textrank(SAMPLE, 0.85, 20);
        assert_eq!(r.scores.len(), 3);
    }

    #[test]
    fn test_centrality_degree() {
        let g = cooccurrence(SAMPLE, 2);
        let c = centrality(&g);
        assert_eq!(c.degree.len(), g.nodes.len());
    }

    #[test]
    fn test_centrality_closeness() {
        let g = cooccurrence(SAMPLE, 2);
        let c = centrality(&g);
        for &v in &c.closeness {
            assert!(v >= 0.0);
        }
    }

    #[test]
    fn test_to_dot() {
        let g = cooccurrence("hello world foo bar", 2);
        let dot = to_dot(&g, "test");
        assert!(dot.starts_with("graph \"test\""));
        assert!(dot.contains("--"));
    }

    #[test]
    fn test_edges_deduplicated() {
        let g = cooccurrence("a b a b", 1);
        let e = edges(&g);
        // Should have edges but no duplicates
        let mut pairs: Vec<(usize, usize)> = e
            .iter()
            .map(|e| (e.from.min(e.to), e.from.max(e.to)))
            .collect();
        pairs.sort();
        pairs.dedup();
        assert_eq!(pairs.len(), e.len());
    }

    #[test]
    fn test_cooccurrence_window_1() {
        let g = cooccurrence("a b c d", 1);
        // With window 1, only adjacent words connect
        assert!(!g.adjacency.is_empty());
    }

    #[test]
    fn test_centrality_empty() {
        let g = TextGraph {
            nodes: Vec::new(),
            adjacency: Vec::new(),
        };
        let c = centrality(&g);
        assert!(c.degree.is_empty());
    }

    #[test]
    fn test_pagerank_positive() {
        let g = cooccurrence(SAMPLE, 2);
        let r = pagerank(&g, 0.85, 50);
        for s in &r.scores {
            assert!(*s > 0.0);
        }
    }

    #[test]
    fn test_textrank_ordering() {
        let r = textrank(SAMPLE, 0.85, 20);
        // All scores should be positive
        assert!(r.scores.iter().all(|s| *s > 0.0));
    }
}
