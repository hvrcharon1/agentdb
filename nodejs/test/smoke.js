'use strict';
// Smoke test for the AgentDB Node.js native addon.
// Requires the .node binary — run `npm run build` first, then `npm test`.

const { AgentDB } = require('..');

const db = AgentDB.open(':memory:');

// ── SQL ───────────────────────────────────────────────────────────────
db.execute('CREATE TABLE t (id TEXT PRIMARY KEY, n INTEGER)');
db.execute("INSERT INTO t VALUES ('a', 42)");
const rows = db.query('SELECT * FROM t');
console.assert(rows.length === 1,   `SQL row count: expected 1, got ${rows.length}`);
console.assert(rows[0].id === 'a',  `SQL id: expected 'a', got '${rows[0].id}'`);
console.assert(rows[0].n  === 42,   `SQL n: expected 42, got ${rows[0].n}`);

// ── Vector store ──────────────────────────────────────────────────────
const col = db.collection('vecs', 3);
col.upsert('v1', [1.0, 0.0, 0.0], { label: 'x-axis' });
col.upsert('v2', [0.0, 1.0, 0.0], { label: 'y-axis' });
const hits = col.search([0.9, 0.1, 0.0], { topK: 1 });
console.assert(hits.length    === 1,    `Vector hits: expected 1, got ${hits.length}`);
console.assert(hits[0].id     === 'v1', `Vector id: expected v1, got ${hits[0].id}`);

// ── Memory graph ─────────────────────────────────────────────────────
db.addNode('n1', 'session', { name: 'test' });
db.addNode('n2', 'thought', { text: 'hello' });
db.addEdge('n1', 'n2', 'recalled', 0.8);
const neighbors = db.neighbors('n1', 1);
console.assert(neighbors.length  === 1,    `Graph neighbors: expected 1, got ${neighbors.length}`);
console.assert(neighbors[0].id   === 'n2', `Graph id: expected n2, got ${neighbors[0].id}`);

// ── Stats ──────────────────────────────────────────────────────────────
const stats = db.stats();
console.assert(stats.collections === 1, `Stats collections: expected 1, got ${stats.collections}`);
console.assert(stats.vectors     === 2, `Stats vectors: expected 2, got ${stats.vectors}`);
console.assert(stats.nodes       === 2, `Stats nodes: expected 2, got ${stats.nodes}`);
console.assert(stats.edges       === 1, `Stats edges: expected 1, got ${stats.edges}`);

console.log('\u2705  All smoke tests passed.');
