'use strict';
/**
 * smoke.js — Sanity check for the AgentDB Node.js binding.
 *
 * Exercises every public method surface area through the local build.
 * Exits 0 on success, 1 on any failure.
 *
 * Run:
 *   cd nodejs
 *   npm run build
 *   node test/smoke.js
 */

let AgentDB;
try {
  ({ AgentDB } = require('..'));
} catch (err) {
  console.error('FAIL  Could not load AgentDB. Build the addon first:');
  console.error('      cd nodejs && npm run build');
  console.error(err.message);
  process.exit(1);
}

let passed = 0;
let failed = 0;

function assert(condition, label) {
  if (condition) {
    console.log(`  \u2713  ${label}`);
    passed++;
  } else {
    console.error(`  \u2717  ${label}`);
    failed++;
  }
}

function assertThrows(fn, label) {
  try {
    fn();
    console.error(`  \u2717  ${label} (expected throw, got none)`);
    failed++;
  } catch (_) {
    console.log(`  \u2713  ${label}`);
    passed++;
  }
}

// ---------------------------------------------------------------------------
console.log('\nAgentDB Node.js smoke test\n');

// -- Open ------------------------------------------------------------------
const db = AgentDB.open(':memory:');
assert(db !== null && db !== undefined, 'AgentDB.open(":memory:") returns an object');

// -- SQL -------------------------------------------------------------------
console.log('\nSQL layer:');
const n = db.execute('CREATE TABLE kv (key TEXT PRIMARY KEY, val INTEGER)');
assert(typeof n === 'number', 'execute() returns a number');

db.execute("INSERT INTO kv VALUES ('a', 1)");
db.execute("INSERT INTO kv VALUES ('b', 2)");
db.execute("INSERT INTO kv VALUES ('c', 3)");

const rows = db.query('SELECT * FROM kv ORDER BY key');
assert(Array.isArray(rows),     'query() returns an array');
assert(rows.length === 3,       'query() returns correct row count');
assert(rows[0].key === 'a',     'first row key is correct');
assert(rows[2].val === 3,       'last row val is correct');

// -- Vector store ----------------------------------------------------------
console.log('\nVector store:');
const col = db.collection('embeddings', 4);
assert(col !== null && col !== undefined, 'collection() returns an object');

col.upsert('v1', [1.0, 0.0, 0.0, 0.0]);
col.upsert('v2', [0.0, 1.0, 0.0, 0.0]);
col.upsert('v3', [0.0, 0.0, 1.0, 0.0]);
col.upsert('v4', [0.0, 0.0, 0.0, 1.0]);
assert(col.count() === 4, 'count() reports correct count after upsert');

// Upsert again — should update, not duplicate
col.upsert('v1', [1.0, 0.0, 0.0, 0.0], { tag: 'updated' });
assert(col.count() === 4, 'count() is unchanged after re-upsert');

const results = col.search([1.0, 0.0, 0.0, 0.0], { topK: 2 });
assert(Array.isArray(results),  'search() returns an array');
assert(results.length === 2,    'search() respects topK');
assert(results[0].id === 'v1',  'search() returns best match first');
assert(typeof results[0].score === 'number', 'result.score is a number');

// Batch upsert
const batchCount = col.upsertBatch([
  { id: 'b1', vector: [0.5, 0.5, 0.0, 0.0] },
  { id: 'b2', vector: [0.0, 0.5, 0.5, 0.0] },
]);
assert(batchCount === 2, 'upsertBatch() returns count of inserted rows');
assert(col.count() === 6, 'count() updated after batch upsert');

// Reindex should not throw
try { col.reindex(); assert(true, 'reindex() completes without error'); }
catch (e) { assert(false, `reindex() threw: ${e.message}`); }

// -- Memory graph ----------------------------------------------------------
console.log('\nMemory graph:');
db.addNode('n1', 'session',  { ts: 1 });
db.addNode('n2', 'thought',  null);
db.addNode('n3', 'thought',  null);
db.addEdge('n1', 'n2', 'recalled', 0.9);
db.addEdge('n1', 'n3', 'recalled', 0.5);
db.addEdge('n2', 'n3', 'leads_to', 0.7);

const neighbors1 = db.neighbors('n1', 1);
assert(Array.isArray(neighbors1),   'neighbors() returns an array');
assert(neighbors1.length === 2,     'neighbors() finds direct neighbours');
assert(
  neighbors1.every(n => typeof n.id === 'string' && typeof n.depth === 'number'),
  'neighbor entries have id and depth',
);

const neighbors2 = db.neighbors('n1', 2);
assert(neighbors2.length >= 2, 'depth=2 traversal finds at least same nodes');

// -- Full-text search ------------------------------------------------------
console.log('\nFull-text search:');
db.ftsIndex('docs', 'd1', 'd1', 'the quick brown fox');
db.ftsIndex('docs', 'd2', 'd2', 'jumped over the lazy dog');
db.ftsIndex('docs', 'd3', 'd3', 'a quick brown dog');

const fts = db.ftsSearch('docs', 'quick', 10);
assert(Array.isArray(fts),  'ftsSearch() returns an array');
assert(fts.length === 2,    'ftsSearch() matches correct documents');
assert(
  fts.every(r => typeof r.id === 'string' && typeof r.rank === 'number'),
  'FTS result entries have id and rank',
);

// -- Stats -----------------------------------------------------------------
console.log('\nStats:');
const stats = db.stats();
assert(typeof stats === 'object',           'stats() returns an object');
assert(typeof stats.collections === 'number', 'stats.collections is a number');
assert(typeof stats.vectors === 'number',   'stats.vectors is a number');
assert(typeof stats.nodes === 'number',     'stats.nodes is a number');
assert(typeof stats.edges === 'number',     'stats.edges is a number');
assert(stats.collections >= 1,              'stats.collections ≥ 1');
assert(stats.vectors === 6,                 'stats.vectors === 6');
assert(stats.nodes === 3,                   'stats.nodes === 3');
assert(stats.edges === 3,                   'stats.edges === 3');

// -- Summary ---------------------------------------------------------------
console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
process.exit(failed > 0 ? 1 : 0);
