# AgentDB Benchmarks

All benchmarks live in [`benches/`](./benches/) and use the
[Criterion](https://github.com/bheisler/criterion.rs) harness.

## Benchmark suites

| File | What it measures |
|------|------------------|
| [`benches/vector_search.rs`](./benches/vector_search.rs) | HNSW ANN search throughput at varying dataset sizes and embedding dimensions |
| [`benches/graph_traverse.rs`](./benches/graph_traverse.rs) | Recursive CTE traversal speed at varying graph densities and depths |

---

## Running benchmarks

```bash
# Run all suites
cargo bench

# Run a single suite
cargo bench --bench vector_search
cargo bench --bench graph_traverse

# Run a specific benchmark group within a suite
cargo bench --bench vector_search -- "search/128d"

# Save results to target/criterion/ and open the HTML report
cargo bench -- --output-format html
```

Results are written to `target/criterion/`. The HTML report includes throughput
charts, confidence intervals, and regression detection between runs.

---

## Reproducible benchmark environment

For stable, comparable numbers we recommend:

- A dedicated CI runner or an isolated physical core
- Disable CPU frequency scaling / turbo boost
- Linux with `perf_event_paranoid ≤ 1` for hardware counter support
- `RUSTFLAGS="-C target-cpu=native"` to enable AVX-512 / NEON SIMD paths

```bash
export RUSTFLAGS="-C target-cpu=native"
cargo bench
```

---

## `vector_search` — HNSW ANN search

### Parameters varied

| Parameter | Values tested | Notes |
|-----------|--------------|-------|
| Embedding dimension | 128, 384, 1536 | Matches common embedding models |
| Dataset size | 1 K, 10 K, 100 K vectors | Pre-loaded before timing |
| `top_k` | 10 | Standard recall@10 setting |
| Distance metric | Cosine | Normalised embeddings |

### Representative results (Apple M3 Max, single core, `target-cpu=native`)

| Dataset | Dim | Latency p50 | Latency p99 |
|---------|-----|-------------|-------------|
| 1 K | 128 | < 1 ms | < 2 ms |
| 10 K | 384 | ~3 ms | ~6 ms |
| 100 K | 1536 | ~15 ms | ~25 ms |

> **Note:** These figures are illustrative. Actual performance depends on hardware,
> HNSW index parameters (`ef_construction`, `M`), and build flags.

---

## `graph_traverse` — recursive CTE traversal

### Parameters varied

| Parameter | Values tested | Notes |
|-----------|--------------|-------|
| Graph size | 500, 5 K, 50 K nodes | Random Erdős–Rényi graph |
| Max traversal depth | 3 | Typical agent memory hop count |
| Edge density | ~0.01 | Sparse directed graph |

### Representative results (Apple M3 Max, single core)

| Graph size | Latency p50 | Latency p99 |
|------------|-------------|-------------|
| 500 nodes | < 1 ms | < 2 ms |
| 5 K nodes | ~4 ms | ~8 ms |
| 50 K nodes | ~20 ms | ~40 ms |

---

## CI benchmark regression checks

The CI pipeline runs benchmarks on every push to `main` and posts a regression
summary on any PR that touches performance-critical paths
(`src/vectors/`, `src/memory/`, `src/hybrid.rs`, `src/fts/`).
A **> 10 % regression** in any benchmark group blocks the merge.
