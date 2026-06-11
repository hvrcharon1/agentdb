/**
 * Quick-start: agent memory management with AgentDB.
 *
 * Prerequisites:
 *   cd nodejs
 *   npm install
 *   npm run build          # compile the native addon
 *
 * Run:
 *   npx ts-node examples/agent_memory.ts
 *
 * This script demonstrates the four AgentDB layers:
 *   1. Relational SQL  (CREATE TABLE / INSERT / SELECT)
 *   2. Vector store    (upsert + ANN search)
 *   3. Memory graph    (nodes, edges, traversal)
 *   4. Statistics
 */

import { AgentDB } from 'agentdb';

// Open an in-memory database (replace ':memory:' with a file path for persistence)
const db = AgentDB.open(':memory:');

// ── 1. Relational SQL ─────────────────────────────────────────────────
db.execute('CREATE TABLE sessions (id TEXT PRIMARY KEY, name TEXT)');
db.execute("INSERT INTO sessions VALUES ('s1', 'Research Sprint')");
db.execute("INSERT INTO sessions VALUES ('s2', 'Planning Session')");

const rows = db.query('SELECT * FROM sessions ORDER BY id');
console.log('Sessions:');
for (const row of rows) {
  console.log(' ', row);
}

// ── 2. Vector store ───────────────────────────────────────────────────
// Get (or create) a collection of 4-dimensional embeddings.
const col = db.collection('thoughts', 4);

col.upsert('t1', [0.9, 0.1, 0.0, 0.0], { topic: 'RL',  score: 9 });
col.upsert('t2', [0.1, 0.9, 0.0, 0.0], { topic: 'CV',  score: 7 });
col.upsert('t3', [0.5, 0.5, 0.0, 0.0], { topic: 'NLP', score: 8 });

const results = col.search([0.85, 0.15, 0.0, 0.0], { topK: 2 });
console.log('\nNearest thoughts to [0.85, 0.15, ...]:');
for (const r of results) {
  console.log(`  id=${r.id}  score=${r.score.toFixed(4)}  meta=${JSON.stringify(r.metadata)}`);
}

// ── 3. Memory graph ───────────────────────────────────────────────────
db.addNode('s1', 'session', { name: 'Research Sprint' });
db.addNode('t1', 'thought', { topic: 'RL' });
db.addNode('t2', 'thought', { topic: 'CV' });
db.addNode('t3', 'thought', { topic: 'NLP' });

db.addEdge('s1', 't1', 'recalled', 0.9);
db.addEdge('s1', 't2', 'recalled', 0.7);
db.addEdge('s1', 't3', 'recalled', 0.5);

const neighbors = db.neighbors('s1', 1);
console.log('\nThoughts recalled by session s1:');
for (const n of neighbors) {
  console.log(`  id=${n.id}  kind=${n.kind}  depth=${n.depth}  weight=${n.weight.toFixed(2)}`);
}

// ── 4. Statistics ─────────────────────────────────────────────────────
const stats = db.stats();
console.log(
  `\nDB stats: collections=${stats.collections} ` +
  `vectors=${stats.vectors} ` +
  `nodes=${stats.nodes} ` +
  `edges=${stats.edges}`
);
