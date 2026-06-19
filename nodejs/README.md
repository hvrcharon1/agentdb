# AgentDB Node.js SDK

Single-file embedded database for AI agents. SQL + Vector Search + Full-Text Search + Hybrid Queries + Memory Graphs — all in one `.agentdb` file.

## Installation

```bash
npm install @datacules/agentdb
```

Requires Node.js 18+. No external database server needed. Ships with prebuilt native addons for Linux, macOS, and Windows (x64 + arm64).

## Quick Start

```ts
import { AgentDB } from '@datacules/agentdb';

// Open or create a database (use ':memory:' for in-process only)
const db = AgentDB.open('agent.agentdb');

// SQL — full SQLite syntax
db.execute('CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, ts TEXT)');
db.execute("INSERT INTO sessions VALUES ('s1', '2026-06-19')");
const rows = db.query('SELECT * FROM sessions');
console.log(rows); // [{ id: 's1', ts: '2026-06-19' }]

// Vector search — HNSW index, cosine similarity
const col = db.collection('thoughts', 4);
col.upsert('t1', [0.9, 0.1, 0.0, 0.0], { topic: 'memory' });
col.upsert('t2', [0.1, 0.9, 0.0, 0.0], { topic: 'reasoning' });
const results = col.search([0.85, 0.15, 0.0, 0.0], { topK: 1 });
console.log(results[0].id); // 't1'

// Full-text search
db.ftsIndex('docs', 'd1', 's1', 'AgentDB stores agent memory efficiently');
const hits = db.ftsSearch('docs', 'memory', 5);

// Memory graph — associative knowledge store
db.addNode('s1', 'session');
db.addNode('t1', 'thought');
db.addEdge('s1', 't1', 'recalled', 0.9);
const neighbors = db.neighbors('s1', 2);

// Hybrid graph + vector query
const hybrid = db.hybridQuery('s1', [0.9, 0.1, 0.0, 0.0], 'thoughts', { topK: 5 });

// Stats
console.log(db.stats());
// { collections: 1, vectors: 2, nodes: 2, edges: 1 }
```

## TypeScript Support

Full TypeScript types are included. Import types directly:

```ts
import type { SearchResult, FtsResult, HybridResult, DbStats } from '@datacules/agentdb';
```

## API Reference

See the [full documentation](https://github.com/hvrcharon1/agentdb#api-reference) in the main README.

## License

Unlicense — public domain. See [LICENSE](https://github.com/hvrcharon1/agentdb/blob/main/LICENSE).
