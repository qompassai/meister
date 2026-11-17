//! SYS-01 — the shared graph validator.
//!
//! Pure functions over a directed graph given as node ids and
//! `(from, to)` edge pairs. No I/O, no store knowledge: the
//! scene, system, and level validators build their node/edge
//! lists and ask their structural questions here, so every kind
//! refuses the same shapes with the same typed codes.
//!
//! Bounds are contractual and checked before any traversal: a
//! 10,000-node input is refused on its counts alone. Traversal
//! is BFS/Kahn over adjacency built once, so the work is linear
//! in nodes + edges. Output is deterministic: every reported
//! set is sorted.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Contract bounds (PATH-TO-1.0, SYS-01, proposed at Rung 2 and
/// adopted by this implementation).
pub const MAX_NODES: usize = 512;
pub const MAX_EDGES: usize = 2_048;

/// A structural refusal, carrying the typed code the plan names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// A count crossed a contract bound, before any traversal.
    BoundExceeded(String),
    /// The graph must be acyclic and is not; payload names the
    /// nodes Kahn's algorithm could not order, sorted.
    Cycle(Vec<String>),
    /// Nodes no declared root can reach, sorted.
    Unreachable(Vec<String>),
    /// A root or edge endpoint that is not a declared node.
    DanglingRef(String),
}

impl GraphError {
    /// The typed code a caller surfaces.
    pub fn code(&self) -> &'static str {
        match self {
            GraphError::BoundExceeded(_) => "bound_exceeded",
            GraphError::Cycle(_) => "graph_cycle",
            GraphError::Unreachable(_) => "graph_unreachable",
            GraphError::DanglingRef(_) => "graph_dangling_ref",
        }
    }

    /// The human-readable refusal, naming the offending ids.
    pub fn message(&self) -> String {
        match self {
            GraphError::BoundExceeded(detail) => detail.clone(),
            GraphError::Cycle(nodes) => {
                format!("graph has a cycle through {}", nodes.join(", "))
            }
            GraphError::Unreachable(nodes) => {
                format!("unreachable from the declared roots: {}", nodes.join(", "))
            }
            GraphError::DanglingRef(detail) => detail.clone(),
        }
    }
}

/// Adjacency in declared order: node -> sorted successors.
fn adjacency<'a>(
    nodes: &'a [String],
    edges: &'a [(String, String)],
) -> BTreeMap<&'a str, Vec<&'a str>> {
    let mut adj: BTreeMap<&str, Vec<&str>> =
        nodes.iter().map(|n| (n.as_str(), Vec::new())).collect();
    for (from, to) in edges {
        if let Some(out) = adj.get_mut(from.as_str()) {
            out.push(to.as_str());
        }
    }
    for out in adj.values_mut() {
        out.sort_unstable();
        out.dedup();
    }
    adj
}

/// The set of nodes reachable from `roots` (roots included),
/// following edge direction. Assumes endpoints are declared;
/// `validate` checks that before calling.
pub fn reachable(
    nodes: &[String],
    edges: &[(String, String)],
    roots: &[String],
) -> BTreeSet<String> {
    let adj = adjacency(nodes, edges);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    for root in roots {
        if seen.insert(root.clone()) {
            queue.push_back(root.as_str());
        }
    }
    while let Some(node) = queue.pop_front() {
        if let Some(out) = adj.get(node) {
            for next in out {
                if seen.insert((*next).to_string()) {
                    queue.push_back(next);
                }
            }
        }
    }
    seen
}

/// Validate a directed graph against the contract: bounds first,
/// then dangling references, then (optionally) acyclicity and
/// reachability from the declared roots.
pub fn validate(
    nodes: &[String],
    edges: &[(String, String)],
    roots: &[String],
    require_acyclic: bool,
    require_reachable: bool,
) -> Result<(), GraphError> {
    if nodes.len() > MAX_NODES {
        return Err(GraphError::BoundExceeded(format!(
            "{} nodes exceeds the bound of {MAX_NODES}",
            nodes.len()
        )));
    }
    if edges.len() > MAX_EDGES {
        return Err(GraphError::BoundExceeded(format!(
            "{} edges exceeds the bound of {MAX_EDGES}",
            edges.len()
        )));
    }
    let declared: BTreeSet<&str> = nodes.iter().map(String::as_str).collect();
    for root in roots {
        if !declared.contains(root.as_str()) {
            return Err(GraphError::DanglingRef(format!(
                "root {root} is not a declared node"
            )));
        }
    }
    for (from, to) in edges {
        for end in [from, to] {
            if !declared.contains(end.as_str()) {
                return Err(GraphError::DanglingRef(format!(
                    "edge {from} -> {to} names undeclared node {end}"
                )));
            }
        }
    }
    if require_acyclic {
        // Kahn's algorithm: repeatedly remove indegree-0 nodes.
        // What remains is in, or downstream of, a cycle.
        let adj = adjacency(nodes, edges);
        let mut indegree: BTreeMap<&str, usize> =
            nodes.iter().map(|n| (n.as_str(), 0usize)).collect();
        for out in adj.values() {
            for next in out {
                *indegree.get_mut(next).expect("endpoints checked") += 1;
            }
        }
        let mut queue: VecDeque<&str> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut ordered = 0usize;
        while let Some(node) = queue.pop_front() {
            ordered += 1;
            if let Some(out) = adj.get(node) {
                for next in out {
                    let d = indegree.get_mut(next).expect("endpoints checked");
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
        if ordered < nodes.len() {
            let stuck: Vec<String> = indegree
                .iter()
                .filter(|(_, d)| **d > 0)
                .map(|(n, _)| (*n).to_string())
                .collect();
            return Err(GraphError::Cycle(stuck));
        }
    }
    if require_reachable {
        let seen = reachable(nodes, edges, roots);
        let lost: Vec<String> = nodes
            .iter()
            .filter(|n| !seen.contains(*n))
            .cloned()
            .collect();
        if !lost.is_empty() {
            return Err(GraphError::Unreachable(lost));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nodes(ids: &[&str]) -> Vec<String> {
        ids.iter().map(ToString::to_string).collect()
    }

    fn edges(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    // ---------- validation ----------

    #[test]
    fn a_chain_validates() {
        let n = nodes(&["a", "b", "c"]);
        let e = edges(&[("a", "b"), ("b", "c")]);
        assert_eq!(validate(&n, &e, &nodes(&["a"]), true, true), Ok(()));
    }

    #[test]
    fn a_diamond_validates() {
        let n = nodes(&["a", "b", "c", "d"]);
        let e = edges(&[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")]);
        assert_eq!(validate(&n, &e, &nodes(&["a"]), true, true), Ok(()));
    }

    #[test]
    fn a_forest_with_declared_roots_validates() {
        let n = nodes(&["a", "b", "x", "y"]);
        let e = edges(&[("a", "b"), ("x", "y")]);
        assert_eq!(validate(&n, &e, &nodes(&["a", "x"]), true, true), Ok(()));
    }

    #[test]
    fn reachability_is_not_demanded_when_not_asked() {
        let n = nodes(&["a", "b", "island"]);
        let e = edges(&[("a", "b")]);
        assert_eq!(validate(&n, &e, &nodes(&["a"]), true, false), Ok(()));
    }

    // ---------- adversarial ----------

    #[test]
    fn adv_a_self_loop_is_a_cycle() {
        let n = nodes(&["a"]);
        let e = edges(&[("a", "a")]);
        let err = validate(&n, &e, &nodes(&["a"]), true, false).unwrap_err();
        assert_eq!(err.code(), "graph_cycle");
    }

    #[test]
    fn adv_a_two_cycle_is_a_cycle() {
        let n = nodes(&["a", "b", "c"]);
        let e = edges(&[("a", "b"), ("b", "a"), ("b", "c")]);
        let err = validate(&n, &e, &nodes(&["a"]), true, false).unwrap_err();
        assert_eq!(err.code(), "graph_cycle");
        assert!(err.message().contains('a'), "message: {}", err.message());
    }

    #[test]
    fn adv_an_unreachable_island_is_named() {
        let n = nodes(&["a", "b", "isle"]);
        let e = edges(&[("a", "b")]);
        let err = validate(&n, &e, &nodes(&["a"]), false, true).unwrap_err();
        assert_eq!(err, GraphError::Unreachable(nodes(&["isle"])));
    }

    #[test]
    fn adv_a_dangling_edge_is_named() {
        let n = nodes(&["a"]);
        let e = edges(&[("a", "ghost")]);
        let err = validate(&n, &e, &nodes(&["a"]), false, false).unwrap_err();
        assert_eq!(err.code(), "graph_dangling_ref");
        assert!(err.message().contains("ghost"));
    }

    #[test]
    fn adv_an_undeclared_root_is_dangling() {
        let n = nodes(&["a"]);
        let err = validate(&n, &[], &nodes(&["nope"]), false, false).unwrap_err();
        assert_eq!(err.code(), "graph_dangling_ref");
    }

    #[test]
    fn adv_513_nodes_cross_the_bound() {
        let n: Vec<String> = (0..513).map(|i| format!("n{i}")).collect();
        let err = validate(&n, &[], &[], false, false).unwrap_err();
        assert_eq!(err.code(), "bound_exceeded");
    }

    #[test]
    fn adv_a_10000_node_input_is_refused_before_traversal() {
        // No edges and no roots: a traversal-first implementation
        // would happily accept this. The bound refuses it on the
        // node count alone.
        let n: Vec<String> = (0..10_000).map(|i| format!("n{i}")).collect();
        let err = validate(&n, &[], &[], true, true).unwrap_err();
        assert_eq!(err.code(), "bound_exceeded");
    }

    #[test]
    fn adv_2049_edges_cross_the_bound() {
        let n = nodes(&["a", "b"]);
        let e: Vec<(String, String)> = (0..2049)
            .map(|_| ("a".to_string(), "b".to_string()))
            .collect();
        let err = validate(&n, &e, &nodes(&["a"]), false, false).unwrap_err();
        assert_eq!(err.code(), "bound_exceeded");
    }
}
