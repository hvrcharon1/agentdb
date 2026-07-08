use agentdb::{AgentDB, TraversalOptions};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;

fn build_graph(db: &AgentDB, node_count: usize, edge_factor: usize) {
    let graph = db.memory();
    for i in 0..node_count {
        graph
            .add_node(
                &format!("node_{}", i),
                if i % 3 == 0 { "session" } else { "concept" },
                None,
            )
            .unwrap();
    }
    for i in 0..node_count {
        for k in 1..=edge_factor {
            let dst = (i + k) % node_count;
            if dst != i {
                graph
                    .add_edge(
                        &format!("node_{}", i),
                        &format!("node_{}", dst),
                        "relates",
                        0.8,
                    )
                    .unwrap();
            }
        }
    }
}

fn bench_add_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_add_nodes");
    for count in [100usize, 1_000, 5_000] {
        group.bench_with_input(
            BenchmarkId::new("add_n_nodes", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let db = AgentDB::open(":memory:").unwrap();
                    let graph = db.memory();
                    for i in 0..count {
                        graph.add_node(&format!("n{}", i), "concept", None).unwrap();
                    }
                    black_box(graph.stats().unwrap());
                });
            },
        );
    }
    group.finish();
}

fn bench_traversal_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_traversal");
    for depth in [1usize, 2, 3, 4] {
        group.bench_with_input(
            BenchmarkId::new("traverse_depth", depth),
            &depth,
            |b, &depth| {
                let db = AgentDB::open(":memory:").unwrap();
                build_graph(&db, 1_000, 4);
                b.iter(|| {
                    let graph = db.memory();
                    let results = graph
                        .neighbors(
                            black_box("node_0"),
                            TraversalOptions {
                                relation: None,
                                max_depth: depth,
                                min_weight: None,
                            },
                        )
                        .unwrap();
                    black_box(results);
                });
            },
        );
    }
    group.finish();
}

fn bench_traversal_with_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_traversal_filtered");
    group.bench_function("traverse_weight_filter", |b| {
        let db = AgentDB::open(":memory:").unwrap();
        build_graph(&db, 1_000, 4);
        b.iter(|| {
            let graph = db.memory();
            let results = graph
                .neighbors(
                    black_box("node_0"),
                    TraversalOptions {
                        relation: None,
                        max_depth: 3,
                        min_weight: Some(0.75),
                    },
                )
                .unwrap();
            black_box(results);
        });
    });
    group.bench_function("traverse_relation_filter", |b| {
        let db = AgentDB::open(":memory:").unwrap();
        build_graph(&db, 1_000, 4);
        b.iter(|| {
            let graph = db.memory();
            let results = graph
                .neighbors(
                    black_box("node_0"),
                    TraversalOptions {
                        relation: Some("relates".into()),
                        max_depth: 3,
                        min_weight: None,
                    },
                )
                .unwrap();
            black_box(results);
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_add_nodes,
    bench_traversal_depth,
    bench_traversal_with_filter
);
criterion_main!(benches);
