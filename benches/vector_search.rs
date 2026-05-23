use agentdb::{AgentDB, DistanceMetric, SearchOptions, VectorEntry};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn make_vec(seed: f32, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|i| (seed + i as f32 * 0.001).sin().abs())
        .collect()
}

fn bench_upsert(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_upsert");
    for count in [100usize, 1_000, 10_000] {
        group.bench_with_input(BenchmarkId::new("upsert_n", count), &count, |b, &count| {
            b.iter(|| {
                let db = AgentDB::open(":memory:").unwrap();
                let col = db.vectors().collection("bench", 128).unwrap();
                for i in 0..count {
                    col.upsert(VectorEntry {
                        id: format!("v{}", i),
                        vector: make_vec(i as f32, 128),
                        metadata: None,
                    })
                    .unwrap();
                }
                black_box(col.count().unwrap());
            });
        });
    }
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_search");
    for count in [1_000usize, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::new("ann_search_n", count),
            &count,
            |b, &count| {
                let db = AgentDB::open(":memory:").unwrap();
                let col = db.vectors().collection("bench", 128).unwrap();
                for i in 0..count {
                    col.upsert(VectorEntry {
                        id: format!("v{}", i),
                        vector: make_vec(i as f32, 128),
                        metadata: None,
                    })
                    .unwrap();
                }
                col.reindex().unwrap();
                let query = make_vec(42.0, 128);
                b.iter(|| {
                    let results = col
                        .search(
                            black_box(&query),
                            SearchOptions {
                                top_k: 10,
                                metric: DistanceMetric::Cosine,
                                filter: None,
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

fn bench_reindex(c: &mut Criterion) {
    let mut group = c.benchmark_group("vector_reindex");
    for count in [1_000usize, 10_000] {
        group.bench_with_input(BenchmarkId::new("reindex_n", count), &count, |b, &count| {
            let db = AgentDB::open(":memory:").unwrap();
            let col = db.vectors().collection("bench", 128).unwrap();
            for i in 0..count {
                col.upsert(VectorEntry {
                    id: format!("v{}", i),
                    vector: make_vec(i as f32, 128),
                    metadata: None,
                })
                .unwrap();
            }
            b.iter(|| {
                black_box(col.reindex().unwrap());
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_upsert, bench_search, bench_reindex);
criterion_main!(benches);
