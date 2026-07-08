'use strict';
// Smoke test for the AgentDB Node.js native addon.
// Requires the .node binary — run `npm run build` first, then `npm test`.

const assert = require('assert/strict');
// napi-rs emits 'AgentDb' (PascalCase from snake_case), not 'AgentDB'.
const { AgentDb: AgentDB } = require('..');

const db = AgentDB.open(':memory:');

// -- SQL ------------------------------------------------------------------
db.execute('CREATE TABLE t (id TEXT PRIMARY KEY, n INTEGER)');
db.execute("INSERT INTO t VALUES ('a', 42)");
const rows = db.query('SELECT * FROM t');
assert.equal(rows.length, 1,  `SQL row count: expected 1, got ${rows.length}`);
assert.equal(rows[0].id, 'a', `SQL id: expected 'a', got '${rows[0].id}'`);
assert.equal(rows[0].n,  42,  `SQL n: expected 42, got ${rows[0].n}`);

// -- Vector store ---------------------------------------------------------
const col = db.collection('vecs', 3);
col.upsert('v1', [1.0, 0.0, 0.0], { label: 'x-axis' });
col.upsert('v2', [0.0, 1.0, 0.0], { label: 'y-axis' });
const hits = col.search([0.9, 0.1, 0.0], { topK: 1 });
assert.equal(hits.length, 1,    `Vector hits: expected 1, got ${hits.length}`);
assert.equal(hits[0].id,  'v1', `Vector id: expected v1, got ${hits[0].id}`);

// -- Memory graph ---------------------------------------------------------
db.addNode('n1', 'session', { name: 'test' });
db.addNode('n2', 'thought', { text: 'hello' });
db.addEdge('n1', 'n2', 'recalled', 0.8);
const neighbors = db.neighbors('n1', 1);
assert.equal(neighbors.length,  1,    `Graph neighbors: expected 1, got ${neighbors.length}`);
assert.equal(neighbors[0].id,   'n2', `Graph id: expected n2, got ${neighbors[0].id}`);

// -- Stats ----------------------------------------------------------------
const stats = db.stats();
assert.equal(stats.collections, 1, `Stats collections: expected 1, got ${stats.collections}`);
assert.equal(stats.vectors,     2, `Stats vectors: expected 2, got ${stats.vectors}`);
assert.equal(stats.nodes,       2, `Stats nodes: expected 2, got ${stats.nodes}`);
assert.equal(stats.edges,       1, `Stats edges: expected 1, got ${stats.edges}`);

console.log('All smoke tests passed.');
