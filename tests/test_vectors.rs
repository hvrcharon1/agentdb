#[cfg(test)]
mod tests {
    use agentdb::{AgentDB, BatchEntry, DistanceMetric, SearchOptions, VectorEntry};
    use serde_json::json;

    fn open() -> AgentDB {
        AgentDB::open(":memory:").expect("failed to open in-memory db")
    }

    fn make_vec(val: f32, dim: usize) -> Vec<f32> {
        vec![val; dim]
    }

    #[test]
    fn test_create_collection() {
        let db = open();
        let col = db.vectors().collection("test", 4).unwrap();
        assert_eq!(col.name, "test");
        assert_eq!(col.dim, 4);
        assert_eq!(col.count().unwrap(), 0);
    }

    #[test]
    fn test_upsert_and_count() {
        let db = open();
        let col = db.vectors().collection("thoughts", 4).unwrap();
        col.upsert(VectorEntry {
            id: "v1".into(),
            vector: vec![0.1, 0.2, 0.3, 0.4],
            metadata: None,
        })
        .unwrap();
        col.upsert(VectorEntry {
            id: "v2".into(),
            vector: vec![0.9, 0.8, 0.7, 0.6],
            metadata: None,
        })
        .unwrap();
        assert_eq!(col.count().unwrap(), 2);
    }

    #[test]
    fn test_dimension_mismatch_returns_error() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        let result = col.upsert(VectorEntry {
            id: "bad".into(),
            vector: vec![0.1, 0.2],
            metadata: None,
        });
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("mismatch") || err.contains("Dimension"));
    }

    #[test]
    fn test_search_returns_top_k() {
        let db = open();
        let col = db.vectors().collection("mem", 4).unwrap();
        for i in 0..10u32 {
            col.upsert(VectorEntry {
                id: format!("v{}", i),
                vector: vec![i as f32 / 10.0, 0.0, 0.0, 0.0],
                metadata: None,
            })
            .unwrap();
        }
        let results = col
            .search(
                &[0.9, 0.0, 0.0, 0.0],
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
    fn test_search_top1_is_closest() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        col.upsert(VectorEntry {
            id: "far".into(),
            vector: vec![0.0, 0.0, 0.0, 1.0],
            metadata: None,
        })
        .unwrap();
        col.upsert(VectorEntry {
            id: "close".into(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: None,
        })
        .unwrap();
        col.upsert(VectorEntry {
            id: "mid".into(),
            vector: vec![0.5, 0.5, 0.0, 0.0],
            metadata: None,
        })
        .unwrap();
        let results = col
            .search(
                &[1.0, 0.0, 0.0, 0.0],
                SearchOptions {
                    top_k: 1,
                    metric: DistanceMetric::Cosine,
                    filter: None,
                },
            )
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "close");
    }

    #[test]
    fn test_upsert_updates_existing() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        col.upsert(VectorEntry {
            id: "v1".into(),
            vector: vec![0.1, 0.0, 0.0, 0.0],
            metadata: Some(json!({ "version": 1 })),
        })
        .unwrap();
        col.upsert(VectorEntry {
            id: "v1".into(),
            vector: vec![0.9, 0.0, 0.0, 0.0],
            metadata: Some(json!({ "version": 2 })),
        })
        .unwrap();
        // Re-upserting the same ID must not increment the count.
        assert_eq!(col.count().unwrap(), 1, "re-upsert must not inflate count");
        let results = col
            .search(
                &[0.9, 0.0, 0.0, 0.0],
                SearchOptions {
                    top_k: 1,
                    metric: DistanceMetric::Cosine,
                    filter: None,
                },
            )
            .unwrap();
        assert_eq!(results[0].id, "v1");
    }

    #[test]
    fn test_upsert_batch_count_no_overcount() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        let entries = vec![
            BatchEntry {
                id: "b1".into(),
                vector: vec![1.0, 0.0, 0.0, 0.0],
                metadata: None,
            },
            BatchEntry {
                id: "b2".into(),
                vector: vec![0.0, 1.0, 0.0, 0.0],
                metadata: None,
            },
        ];
        col.upsert_batch(entries.clone()).unwrap();
        assert_eq!(col.count().unwrap(), 2, "initial batch must count 2");
        // Re-insert the same IDs — count must stay at 2, not grow to 4.
        col.upsert_batch(entries).unwrap();
        assert_eq!(
            col.count().unwrap(),
            2,
            "re-upsert batch must not inflate count"
        );
    }

    #[test]
    fn test_delete_vector() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        col.upsert(VectorEntry {
            id: "v1".into(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: None,
        })
        .unwrap();
        col.upsert(VectorEntry {
            id: "v2".into(),
            vector: vec![0.0, 1.0, 0.0, 0.0],
            metadata: None,
        })
        .unwrap();
        col.delete("v1").unwrap();
        col.reindex().unwrap();
        let results = col
            .search(
                &[1.0, 0.0, 0.0, 0.0],
                SearchOptions {
                    top_k: 5,
                    metric: DistanceMetric::Cosine,
                    filter: None,
                },
            )
            .unwrap();
        assert!(results.iter().all(|r| r.id != "v1"));
    }

    #[test]
    fn test_metadata_stored_and_returned() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        col.upsert(VectorEntry {
            id: "v1".into(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: Some(json!({ "text": "hello", "role": "user" })),
        })
        .unwrap();
        let results = col
            .search(
                &[1.0, 0.0, 0.0, 0.0],
                SearchOptions {
                    top_k: 1,
                    metric: DistanceMetric::Cosine,
                    filter: None,
                },
            )
            .unwrap();
        assert!(results[0].metadata.is_some());
        assert_eq!(results[0].metadata.as_ref().unwrap()["role"], "user");
    }

    #[test]
    fn test_metadata_filter() {
        let db = open();
        let col = db.vectors().collection("col", 4).unwrap();
        col.upsert(VectorEntry {
            id: "user_msg".into(),
            vector: vec![1.0, 0.0, 0.0, 0.0],
            metadata: Some(json!({ "role": "user" })),
        })
        .unwrap();
        col.upsert(VectorEntry {
            id: "agent_msg".into(),
            vector: vec![0.99, 0.0, 0.0, 0.0],
            metadata: Some(json!({ "role": "agent" })),
        })
        .unwrap();
        let results = col
            .search(
                &[1.0, 0.0, 0.0, 0.0],
                SearchOptions {
                    top_k: 5,
                    metric: DistanceMetric::Cosine,
                    filter: Some(json!({ "role": "user" })),
                },
            )
            .unwrap();
        assert!(results.iter().all(|r| r.id == "user_msg"));
    }

    #[test]
    fn test_list_collections() {
        let db = open();
        db.vectors().collection("alpha", 4).unwrap();
        db.vectors().collection("beta", 8).unwrap();
        let cols = db.vectors().list_collections().unwrap();
        assert_eq!(cols.len(), 2);
        assert!(cols.iter().any(|(name, _, _)| name == "alpha"));
        assert!(cols.iter().any(|(name, _, _)| name == "beta"));
    }

    #[test]
    fn test_drop_collection() {
        let db = open();
        db.vectors().collection("temp", 4).unwrap();
        db.vectors().drop_collection("temp").unwrap();
        let cols = db.vectors().list_collections().unwrap();
        assert!(cols.iter().all(|(name, _, _)| name != "temp"));
    }

    #[test]
    fn test_reindex_on_empty_collection() {
        let db = open();
        let col = db.vectors().collection("empty", 4).unwrap();
        col.reindex().unwrap();
        assert_eq!(col.count().unwrap(), 0);
    }

    #[test]
    fn test_search_empty_collection_returns_empty() {
        let db = open();
        let col = db.vectors().collection("empty", 4).unwrap();
        let results = col
            .search(
                &[1.0, 0.0, 0.0, 0.0],
                SearchOptions {
                    top_k: 5,
                    metric: DistanceMetric::Cosine,
                    filter: None,
                },
            )
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_multiple_collections_isolated() {
        let db = open();
        let col_a = db.vectors().collection("a", 4).unwrap();
        let col_b = db.vectors().collection("b", 4).unwrap();
        col_a
            .upsert(VectorEntry {
                id: "a1".into(),
                vector: make_vec(0.5, 4),
                metadata: None,
            })
            .unwrap();
        col_b
            .upsert(VectorEntry {
                id: "b1".into(),
                vector: make_vec(0.5, 4),
                metadata: None,
            })
            .unwrap();
        assert_eq!(col_a.count().unwrap(), 1);
        assert_eq!(col_b.count().unwrap(), 1);
        let results = col_a.search(&make_vec(0.5, 4), Default::default()).unwrap();
        assert!(results.iter().all(|r| r.id != "b1"));
    }

    #[test]
    fn test_batch_import_unused() {
        // Ensure BatchEntry is importable (used in test_v020)
        let _: Option<agentdb::BatchEntry> = None;
    }

    // ── T6: DotProduct vs Cosine distinctness ────────────────────────────

    #[test]
    fn test_dot_product_ranking_differs_from_cosine() {
        // Use unnormalized vectors where magnitude matters for dot product
        // but not for cosine.
        //
        // query = [4.0, 0.0]
        // v_long  = [4.0, 0.0]  — large magnitude, same direction as query
        // v_short = [0.5, 0.0]  — small magnitude, same direction as query
        // v_perp  = [0.0, 1.0]  — perpendicular
        //
        // Cosine: v_long and v_short are tied (both perfect cosine = 1.0)
        // DotProduct: v_long wins (4*4=16 > 4*0.5=2)
        let db = open();
        let dot_col = db
            .vectors()
            .collection_with_metric("dot_test", 2, DistanceMetric::DotProduct)
            .unwrap();
        dot_col
            .upsert(VectorEntry {
                id: "v_long".into(),
                vector: vec![4.0, 0.0],
                metadata: None,
            })
            .unwrap();
        dot_col
            .upsert(VectorEntry {
                id: "v_short".into(),
                vector: vec![0.5, 0.0],
                metadata: None,
            })
            .unwrap();
        dot_col
            .upsert(VectorEntry {
                id: "v_perp".into(),
                vector: vec![0.0, 1.0],
                metadata: None,
            })
            .unwrap();

        let dot_results = dot_col
            .search(
                &[4.0, 0.0],
                SearchOptions {
                    top_k: 3,
                    metric: DistanceMetric::DotProduct,
                    filter: None,
                },
            )
            .unwrap();
        assert_eq!(dot_results[0].id, "v_long", "dot product: long vector should rank first");

        let cos_col = db
            .vectors()
            .collection_with_metric("cos_test", 2, DistanceMetric::Cosine)
            .unwrap();
        cos_col
            .upsert(VectorEntry {
                id: "v_long".into(),
                vector: vec![4.0, 0.0],
                metadata: None,
            })
            .unwrap();
        cos_col
            .upsert(VectorEntry {
                id: "v_short".into(),
                vector: vec![0.5, 0.0],
                metadata: None,
            })
            .unwrap();

        let cos_results = cos_col
            .search(
                &[4.0, 0.0],
                SearchOptions {
                    top_k: 2,
                    metric: DistanceMetric::Cosine,
                    filter: None,
                },
            )
            .unwrap();
        // Cosine: both vectors point in the same direction — both score ≈ 1.0
        assert_eq!(cos_results.len(), 2, "both should appear in cosine results");
        let cos_scores: Vec<f32> = cos_results.iter().map(|r| r.score).collect();
        assert!(
            (cos_scores[0] - cos_scores[1]).abs() < 0.01,
            "cosine scores should be equal for same-direction vectors: {:?}",
            cos_scores
        );

        // Dot product scores must NOT be equal — magnitude matters
        let dot_scores: Vec<f32> = dot_results[..2].iter().map(|r| r.score).collect();
        assert!(
            (dot_scores[0] - dot_scores[1]).abs() > 0.5,
            "dot product scores should differ significantly: {:?}",
            dot_scores
        );
    }
}
