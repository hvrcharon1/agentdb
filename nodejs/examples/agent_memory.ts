/**
 * agent_memory.ts — AgentDB quick-start example (Node.js / TypeScript)
 *
 * Demonstrates:
 *   - SQL layer         — execute statements and query rows
 *   - Vector store      — upsert embeddings and run ANN search
 *   - Memory graph      — add nodes, directed edges, and traverse
 *   - Full-text search  — index and search text content
 *   - Hybrid query      — blend graph and vector results
 *   - Database stats    — inspect counts across all layers
 *
 * Run (requires ts-node or tsx):
 *   cd nodejs
 *   npm install
 *   npm run build
 *   npx ts-node examples/agent_memory.ts
 */

import { AgentDB } from '..';

// ---------------------------------------------------------------------------
// 1. Open a database
// ---------------------------------------------------------------------------
const db = AgentDB.open(':memory:');
console.log('AgentDB opened\n');

// ---------------------------------------------------------------------------
// 2. SQL layer
// ---------------------------------------------------------------------------
db.execute('CREATE TABLE sessions (id TEXT PRIMARY KEY, label TEXT)');
db.execute("INSERT INTO sessions VALUES ('s1', 'planning session')");
db.execute("INSERT INTO sessions VALUES ('s2', 'review session')");

const rows = db.query('SELECT * FROM sessions ORDER BY id');
console.log('Sessions:', rows);

// ---------------------------------------------------------------------------
// 3. Vector store — HNSW ANN search
// ---------------------------------------------------------------------------
const DIM = 4;
const col = db.collection('thoughts', DIM);

// In production these would be real model embeddings.
col.upsert('t1', [0.9, 0.1, 0.0, 0.0], { label: 'agent planning phase' });
col.upsert('t2', [0.1, 0.9, 0.0, 0.0], { label: 'memory consolidation' });
col.upsert('t3', [0.0, 0.0, 0.9, 0.1], { label: 'tool selection step' });
col.upsert('t4', [0.0, 0.1, 0.1, 0.8], { label: 'final review loop' });

console.log(`\nInserted ${col.count()} vectors into 'thoughts'`);

// Nearest-neighbour search
const searchResults = col.search([0.85, 0.15, 0.0, 0.0], { topK: 3 });
console.log('\nVector search (top 3):');
for (const r of searchResults) {
  console.log(`  id=${r.id}  score=${r.score.toFixed(4)}  metadata=${JSON.stringify(r.metadata)}`);
}

// ---------------------------------------------------------------------------
// 4. Memory graph — typed nodes + directed weighted edges
// ---------------------------------------------------------------------------
db.addNode('s1', 'session', { label: 'planning session' });
db.addNode('t1', 'thought');
db.addNode('t2', 'thought');
db.addNode('t3', 'thought');

db.addEdge('s1', 't1', 'recalled', 0.9);
db.addEdge('s1', 't2', 'recalled', 0.7);
db.addEdge('t1', 't3', 'leads_to', 0.8);

const neighbors = db.neighbors('s1', 2);
console.log('\nGraph neighbours of s1 (depth \u2264 2):');
for (const n of neighbors) {
  console.log(`  id=${n.id}  kind=${n.kind}  depth=${n.depth}  weight=${n.weight}`);
}

// ---------------------------------------------------------------------------
// 5. Full-text search
// ---------------------------------------------------------------------------
db.ftsIndex('thoughts_text', 't1', 't1', 'agent planning phase');
db.ftsIndex('thoughts_text', 't2', 't2', 'memory consolidation loop');
db.ftsIndex('thoughts_text', 't3', 't3', 'tool selection step review');

const ftsResults = db.ftsSearch('thoughts_text', 'planning review', 3);
console.log("\nFull-text search for 'planning review':");
for (const f of ftsResults) {
  console.log(`  id=${f.id}  rank=${f.rank.toFixed(4)}`);
}

// ---------------------------------------------------------------------------
// 6. Hybrid query — blend graph traversal and vector ANN
// ---------------------------------------------------------------------------
const hybridResults = db.hybridQuery(
  's1',                        // anchor node
  [0.85, 0.15, 0.0, 0.0],      // query embedding
  'thoughts',                   // vector collection
  { graphDepth: 2, topK: 3, alpha: 0.6 },
);
console.log('\nHybrid query results (alpha=0.6):');
for (const h of hybridResults) {
  console.log(
    `  id=${h.id}  rank=${h.rankScore.toFixed(4)}` +
    `  vec=${h.vectorScore.toFixed(4)}  graph=${h.graphWeight.toFixed(4)}`,
  );
}

// ---------------------------------------------------------------------------
// 7. Database statistics
// ---------------------------------------------------------------------------
const stats = db.stats();
console.log('\nDatabase stats:', stats);
// Expected: { collections: 2, vectors: 4, nodes: 4, edges: 3 }
