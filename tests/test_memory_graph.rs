#[cfg(test)]
mod tests {
    use agentdb::{AgentDB, TraversalOptions};
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    #[test]
    fn test_add_and_get_node() {
        let db = open();
        let graph = db.memory();
        graph.add_node("n1", "concept", Some(json!({ "label": "Rust" }))).unwrap();
        let node = graph.get_node("n1").unwrap();
        assert_eq!(node.id, "n1");
        assert_eq!(node.kind, "concept");
        assert_eq!(node.data.unwrap()["label"], "Rust");
    }

    #[test]
    fn test_get_nonexistent_node_errors() {
        let db = open();
        let graph = db.memory();
        let result = graph.get_node("ghost");
        assert!(result.is_err());
    }

    #[test]
    fn test_add_node_upserts() {
        let db = open();
        let graph = db.memory();
        graph.add_node("n1", "concept", Some(json!({ "v": 1 }))).unwrap();
        graph.add_node("n1", "concept", Some(json!({ "v": 2 }))).unwrap();
        let node = graph.get_node("n1").unwrap();
        assert_eq!(node.data.unwrap()["v"], 2);
    }

    #[test]
    fn test_delete_node() {
        let db = open();
        let graph = db.memory();
        graph.add_node("n1", "concept", None).unwrap();
        graph.delete_node("n1").unwrap();
        assert!(graph.get_node("n1").is_err());
    }

    #[test]
    fn test_add_edge_requires_existing_nodes() {
        let db = open();
        let graph = db.memory();
        // Neither node exists — should error
        let result = graph.add_edge("missing_src", "missing_dst", "rel", 1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_and_traverse_edge() {
        let db = open();
        let graph = db.memory();
        graph.add_node("a", "session", None).unwrap();
        graph.add_node("b", "concept", None).unwrap();
        graph.add_edge("a", "b", "discussed", 0.9).unwrap();

        let neighbors = graph.neighbors("a", TraversalOptions {
            relation:   Some("discussed".into()),
            max_depth:  1,
            min_weight: None,
        }).unwrap();

        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].node.id, "b");
        assert_eq!(neighbors[0].depth, 1);
    }

    #[test]
    fn test_traversal_depth_2() {
        let db = open();
        let graph = db.memory();
        graph.add_node("root", "session", None).unwrap();
        graph.add_node("mid",  "concept", None).unwrap();
        graph.add_node("leaf", "concept", None).unwrap();
        graph.add_edge("root", "mid",  "relates", 1.0).unwrap();
        graph.add_edge("mid",  "leaf", "relates", 1.0).unwrap();

        let results = graph.neighbors("root", TraversalOptions {
            relation:   None,
            max_depth:  2,
            min_weight: None,
        }).unwrap();

        let ids: Vec<&str> = results.iter().map(|r| r.node.id.as_str()).collect();
        assert!(ids.contains(&"mid"));
        assert!(ids.contains(&"leaf"));
    }

    #[test]
    fn test_traversal_depth_limit() {
        let db = open();
        let graph = db.memory();
        graph.add_node("a", "n", None).unwrap();
        graph.add_node("b", "n", None).unwrap();
        graph.add_node("c", "n", None).unwrap();
        graph.add_edge("a", "b", "r", 1.0).unwrap();
        graph.add_edge("b", "c", "r", 1.0).unwrap();

        // depth=1 should return only b, not c
        let results = graph.neighbors("a", TraversalOptions {
            relation:   None,
            max_depth:  1,
            min_weight: None,
        }).unwrap();

        let ids: Vec<&str> = results.iter().map(|r| r.node.id.as_str()).collect();
        assert!(ids.contains(&"b"));
        assert!(!ids.contains(&"c"));
    }

    #[test]
    fn test_traversal_weight_filter() {
        let db = open();
        let graph = db.memory();
        graph.add_node("root",  "session", None).unwrap();
        graph.add_node("strong","concept", None).unwrap();
        graph.add_node("weak",  "concept", None).unwrap();
        graph.add_edge("root", "strong", "rel", 0.9).unwrap();
        graph.add_edge("root", "weak",   "rel", 0.2).unwrap();

        let results = graph.neighbors("root", TraversalOptions {
            relation:   None,
            max_depth:  1,
            min_weight: Some(0.5),
        }).unwrap();

        let ids: Vec<&str> = results.iter().map(|r| r.node.id.as_str()).collect();
        assert!(ids.contains(&"strong"));
        assert!(!ids.contains(&"weak"));
    }

    #[test]
    fn test_traversal_relation_filter() {
        let db = open();
        let graph = db.memory();
        graph.add_node("root",    "session", None).unwrap();
        graph.add_node("topic_a", "concept", None).unwrap();
        graph.add_node("topic_b", "concept", None).unwrap();
        graph.add_edge("root", "topic_a", "discussed", 1.0).unwrap();
        graph.add_edge("root", "topic_b", "mentioned", 1.0).unwrap();

        let results = graph.neighbors("root", TraversalOptions {
            relation:   Some("discussed".into()),
            max_depth:  1,
            min_weight: None,
        }).unwrap();

        let ids: Vec<&str> = results.iter().map(|r| r.node.id.as_str()).collect();
        assert!(ids.contains(&"topic_a"));
        assert!(!ids.contains(&"topic_b"));
    }

    #[test]
    fn test_nodes_by_kind() {
        let db = open();
        let graph = db.memory();
        graph.add_node("s1", "session", None).unwrap();
        graph.add_node("s2", "session", None).unwrap();
        graph.add_node("c1", "concept", None).unwrap();

        let sessions = graph.nodes_by_kind("session").unwrap();
        assert_eq!(sessions.len(), 2);

        let concepts = graph.nodes_by_kind("concept").unwrap();
        assert_eq!(concepts.len(), 1);
    }

    #[test]
    fn test_graph_stats() {
        let db = open();
        let graph = db.memory();
        graph.add_node("a", "n", None).unwrap();
        graph.add_node("b", "n", None).unwrap();
        graph.add_edge("a", "b", "rel", 1.0).unwrap();

        let (nodes, edges) = graph.stats().unwrap();
        assert_eq!(nodes, 2);
        assert_eq!(edges, 1);
    }

    #[test]
    fn test_delete_node_cascades_edges() {
        let db = open();
        let graph = db.memory();
        graph.add_node("a", "n", None).unwrap();
        graph.add_node("b", "n", None).unwrap();
        graph.add_edge("a", "b", "rel", 1.0).unwrap();

        graph.delete_node("a").unwrap();

        // Edge should be gone (cascade), traversal from b should find nothing
        let (_, edges) = graph.stats().unwrap();
        assert_eq!(edges, 0);
    }

    #[test]
    fn test_no_self_loop_traversal_explosion() {
        let db = open();
        let graph = db.memory();
        graph.add_node("a", "n", None).unwrap();
        graph.add_node("b", "n", None).unwrap();
        graph.add_edge("a", "b", "rel", 1.0).unwrap();
        graph.add_edge("b", "a", "rel", 1.0).unwrap(); // cycle

        // Should terminate, not loop forever
        let results = graph.neighbors("a", TraversalOptions {
            relation:   None,
            max_depth:  5,
            min_weight: None,
        });
        assert!(results.is_ok());
    }
}
