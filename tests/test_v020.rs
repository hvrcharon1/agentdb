#[cfg(test)]
mod tests {
    use agentdb::filter_matches;
    use serde_json::json;

    #[test]
    fn test_exact_match_pass() {
        assert!(filter_matches(
            &json!({"role":"user"}),
            &json!({"role":"user"})
        ));
    }

    #[test]
    fn test_exact_match_fail() {
        assert!(!filter_matches(
            &json!({"role":"user"}),
            &json!({"role":"agent"})
        ));
    }

    #[test]
    fn test_gt_pass() {
        assert!(filter_matches(
            &json!({"score":8}),
            &json!({"score":{"$gt":5}})
        ));
    }

    #[test]
    fn test_gt_fail() {
        assert!(!filter_matches(
            &json!({"score":3}),
            &json!({"score":{"$gt":5}})
        ));
    }

    #[test]
    fn test_gte_boundary() {
        assert!(filter_matches(&json!({"v":7}), &json!({"v":{"$gte":7}})));
        assert!(!filter_matches(&json!({"v":6}), &json!({"v":{"$gte":7}})));
    }

    #[test]
    fn test_lt_lte() {
        assert!(filter_matches(&json!({"v":3}), &json!({"v":{"$lt":5}})));
        assert!(filter_matches(&json!({"v":5}), &json!({"v":{"$lte":5}})));
        assert!(!filter_matches(&json!({"v":6}), &json!({"v":{"$lte":5}})));
    }

    #[test]
    fn test_ne() {
        assert!(filter_matches(
            &json!({"role":"user"}),
            &json!({"role":{"$ne":"agent"}})
        ));
        assert!(!filter_matches(
            &json!({"role":"user"}),
            &json!({"role":{"$ne":"user"}})
        ));
    }

    #[test]
    fn test_in_pass() {
        assert!(filter_matches(
            &json!({"lang":"en"}),
            &json!({"lang":{"$in":["en","fr"]}})
        ));
    }

    #[test]
    fn test_in_fail() {
        assert!(!filter_matches(
            &json!({"lang":"de"}),
            &json!({"lang":{"$in":["en","fr"]}})
        ));
    }

    #[test]
    fn test_nin() {
        assert!(filter_matches(
            &json!({"lang":"de"}),
            &json!({"lang":{"$nin":["en","fr"]}})
        ));
        assert!(!filter_matches(
            &json!({"lang":"en"}),
            &json!({"lang":{"$nin":["en","fr"]}})
        ));
    }

    #[test]
    fn test_exists_true() {
        assert!(filter_matches(
            &json!({"score":5}),
            &json!({"score":{"$exists":true}})
        ));
        assert!(!filter_matches(
            &json!({"other":1}),
            &json!({"score":{"$exists":true}})
        ));
    }

    #[test]
    fn test_exists_false() {
        assert!(filter_matches(
            &json!({"other":1}),
            &json!({"score":{"$exists":false}})
        ));
    }

    #[test]
    fn test_multi_field_all_pass() {
        assert!(filter_matches(
            &json!({"lang":"en","score":9,"role":"user"}),
            &json!({"lang":"en","score":{"$gte":8},"role":{"$ne":"agent"}})
        ));
    }

    #[test]
    fn test_multi_field_one_fail() {
        assert!(!filter_matches(
            &json!({"lang":"en","score":6}),
            &json!({"lang":"en","score":{"$gte":8}})
        ));
    }
}

#[cfg(test)]
mod batch_tests {
    use agentdb::{AgentDB, BatchEntry, DistanceMetric, SearchOptions};
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").unwrap()
    }

    fn v(seed: f32) -> Vec<f32> {
        vec![seed, seed * 0.5, seed * 0.25, seed * 0.1]
    }

    #[test]
    fn test_batch_upsert_count() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        let batch = (0..50u32)
            .map(|i| BatchEntry {
                id: format!("v{}", i),
                vector: v(i as f32 / 50.0),
                metadata: Some(json!({"i": i})),
            })
            .collect();
        let n = col.upsert_batch(batch).unwrap();
        assert_eq!(n, 50);
        assert_eq!(col.count().unwrap(), 50);
    }

    #[test]
    fn test_batch_dim_mismatch_rolls_back() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        let batch = vec![
            BatchEntry {
                id: "good".into(),
                vector: vec![1.0, 0.0, 0.0, 0.0],
                metadata: None,
            },
            BatchEntry {
                id: "bad".into(),
                vector: vec![1.0, 0.0],
                metadata: None,
            },
        ];
        assert!(col.upsert_batch(batch).is_err());
        assert_eq!(col.count().unwrap(), 0);
    }

    #[test]
    fn test_batch_empty_ok() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        let n = col.upsert_batch(vec![]).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_batch_then_search() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        let batch = (0..20u32)
            .map(|i| BatchEntry {
                id: format!("v{}", i),
                vector: v(i as f32 / 20.0),
                metadata: Some(json!({"idx": i})),
            })
            .collect();
        col.upsert_batch(batch).unwrap();
        let results = col
            .search(
                &[0.95, 0.475, 0.2375, 0.095],
                SearchOptions {
                    top_k: 3,
                    metric: DistanceMetric::Cosine,
                    filter: None,
                },
            )
            .unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_filter_gt_after_batch() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        let batch = (0..10u32)
            .map(|i| BatchEntry {
                id: format!("v{}", i),
                vector: v(i as f32 / 10.0 + 0.1),
                metadata: Some(json!({"score": i})),
            })
            .collect();
        col.upsert_batch(batch).unwrap();
        let results = col
            .search(
                &[0.5, 0.25, 0.125, 0.05],
                SearchOptions {
                    top_k: 10,
                    metric: DistanceMetric::Cosine,
                    filter: Some(json!({"score": {"$gt": 5}})),
                },
            )
            .unwrap();
        assert!(results
            .iter()
            .all(|r| { r.metadata.as_ref().unwrap()["score"].as_u64().unwrap() > 5 }));
    }
}

#[cfg(test)]
mod hybrid_tests {
    use agentdb::{AgentDB, BatchEntry, HybridQuery};

    fn open() -> AgentDB {
        AgentDB::open(":memory:").unwrap()
    }

    fn v(seed: f32) -> Vec<f32> {
        (0..8)
            .map(|i| ((seed + i as f32) * 0.1).sin().abs())
            .collect()
    }

    #[test]
    fn test_hybrid_returns_top_k() {
        let db = open();
        let col = db.vectors().collection("docs", 8).unwrap();
        let batch = vec![
            BatchEntry {
                id: "a".into(),
                vector: v(1.0),
                metadata: None,
            },
            BatchEntry {
                id: "b".into(),
                vector: v(2.0),
                metadata: None,
            },
            BatchEntry {
                id: "c".into(),
                vector: v(3.0),
                metadata: None,
            },
        ];
        col.upsert_batch(batch).unwrap();
        let graph = db.memory();
        graph.add_node("root", "session", None).unwrap();
        graph.add_node("a", "doc", None).unwrap();
        graph.add_node("b", "doc", None).unwrap();
        graph.add_edge("root", "a", "read", 0.9).unwrap();
        graph.add_edge("root", "b", "read", 0.5).unwrap();
        let results = db
            .hybrid_query(HybridQuery {
                anchor_node: "root",
                embedding: &v(1.0),
                collection: "docs",
                graph_depth: 1,
                top_k: 2,
                alpha: 0.5,
                filter: None,
            })
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_hybrid_alpha_pure_vector() {
        let db = open();
        let col = db.vectors().collection("docs", 8).unwrap();
        col.upsert_batch(vec![BatchEntry {
            id: "x".into(),
            vector: v(1.0),
            metadata: None,
        }])
        .unwrap();
        let graph = db.memory();
        graph.add_node("root", "session", None).unwrap();
        let results = db
            .hybrid_query(HybridQuery {
                anchor_node: "root",
                embedding: &v(1.0),
                collection: "docs",
                graph_depth: 1,
                top_k: 1,
                alpha: 1.0,
                filter: None,
            })
            .unwrap();
        assert!(!results.is_empty());
        assert!(results[0].graph_weight == 0.0);
    }

    #[test]
    fn test_hybrid_graph_boosts_rank() {
        let db = open();
        let col = db.vectors().collection("docs", 8).unwrap();
        col.upsert_batch(vec![
            BatchEntry {
                id: "connected".into(),
                vector: v(2.0),
                metadata: None,
            },
            BatchEntry {
                id: "disconnected".into(),
                vector: v(2.1),
                metadata: None,
            },
        ])
        .unwrap();
        let graph = db.memory();
        graph.add_node("root", "session", None).unwrap();
        graph.add_node("connected", "doc", None).unwrap();
        graph.add_node("disconnected", "doc", None).unwrap();
        graph.add_edge("root", "connected", "read", 1.0).unwrap();
        let results = db
            .hybrid_query(HybridQuery {
                anchor_node: "root",
                embedding: &v(2.0),
                collection: "docs",
                graph_depth: 1,
                top_k: 2,
                alpha: 0.4,
                filter: None,
            })
            .unwrap();
        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids[0], "connected");
    }
}
