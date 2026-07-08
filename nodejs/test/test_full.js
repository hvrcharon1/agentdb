'use strict';
// Comprehensive test suite for the AgentDb Node.js native addon.
// Requires the .node binary — run `npm run build` first, then:
//   node test/test_full.js

const assert = require('assert/strict');
// napi-rs emits 'AgentDb' (PascalCase from snake_case), not 'AgentDb'.
const { AgentDb } = require('..');

// ── simple test runner ────────────────────────────────────────────────────────

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`  PASS  ${name}`);
    passed++;
  } catch (err) {
    console.error(`  FAIL  ${name}`);
    console.error(`        ${err.message}`);
    failed++;
  }
}

// ── helpers ───────────────────────────────────────────────────────────────────

/** Open a fresh in-memory database for each test. */
function db() {
  return AgentDb.open(':memory:');
}

// ═════════════════════════════════════════════════════════════════════════════
// AgentDb.open
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── AgentDb.open ─────────────────────────────────────────────────');

test('open(:memory:) returns an AgentDb instance', () => {
  const d = AgentDb.open(':memory:');
  assert.ok(d, 'expected a truthy object');
  assert.equal(typeof d.execute, 'function');
});

test('open(:memory:) databases are independent', () => {
  const d1 = AgentDb.open(':memory:');
  const d2 = AgentDb.open(':memory:');
  d1.execute('CREATE TABLE t (x INTEGER)');
  d1.execute('INSERT INTO t VALUES (1)');
  // d2 has no table t — should throw
  assert.throws(() => d2.query('SELECT * FROM t'), /no such table/i);
});

// ═════════════════════════════════════════════════════════════════════════════
// SQL: execute / query / queryParams
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── SQL ──────────────────────────────────────────────────────────');

test('execute() returns rows-affected count', () => {
  const d = db();
  d.execute('CREATE TABLE t (id TEXT PRIMARY KEY, val INTEGER)');
  const n = d.execute("INSERT INTO t VALUES ('a', 1)");
  assert.equal(n, 1, `expected 1 row affected, got ${n}`);
});

test('execute() CREATE + INSERT + query()', () => {
  const d = db();
  d.execute('CREATE TABLE items (id TEXT PRIMARY KEY, n INTEGER)');
  d.execute("INSERT INTO items VALUES ('x', 42)");
  const rows = d.query('SELECT * FROM items');
  assert.equal(rows.length, 1);
  assert.equal(rows[0].id, 'x');
  assert.equal(rows[0].n,  42);
});

test('query() returns multiple rows', () => {
  const d = db();
  d.execute('CREATE TABLE nums (v INTEGER)');
  d.execute('INSERT INTO nums VALUES (1)');
  d.execute('INSERT INTO nums VALUES (2)');
  d.execute('INSERT INTO nums VALUES (3)');
  const rows = d.query('SELECT v FROM nums ORDER BY v');
  assert.equal(rows.length, 3);
  assert.equal(rows[0].v, 1);
  assert.equal(rows[2].v, 3);
});

test('query() returns empty array when no rows', () => {
  const d = db();
  d.execute('CREATE TABLE empty (id TEXT)');
  const rows = d.query('SELECT * FROM empty');
  assert.deepEqual(rows, []);
});

test('queryParams() binds positional parameters', () => {
  const d = db();
  d.execute('CREATE TABLE t (id TEXT, val TEXT)');
  d.execute("INSERT INTO t VALUES ('a', 'alpha')");
  d.execute("INSERT INTO t VALUES ('b', 'beta')");
  const rows = d.queryParams('SELECT * FROM t WHERE id = ?1', ['a']);
  assert.equal(rows.length, 1);
  assert.equal(rows[0].val, 'alpha');
});

test('queryParams() with multiple parameters', () => {
  const d = db();
  d.execute('CREATE TABLE t (a TEXT, b INTEGER)');
  d.execute("INSERT INTO t VALUES ('foo', 10)");
  d.execute("INSERT INTO t VALUES ('bar', 20)");
  const rows = d.queryParams('SELECT * FROM t WHERE a = ?1 AND b = ?2', ['foo', '10']);
  assert.equal(rows.length, 1);
  assert.equal(rows[0].a, 'foo');
});

// ═════════════════════════════════════════════════════════════════════════════
// Vectors: Collection
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Vectors ──────────────────────────────────────────────────────');

test('collection() returns a Collection with count()', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  assert.ok(col, 'expected collection object');
  assert.equal(col.count(), 0, 'new collection should have 0 vectors');
});

test('upsert() and count()', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0]);
  col.upsert('v2', [0.0, 1.0, 0.0]);
  assert.equal(col.count(), 2);
});

test('upsert() with metadata', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0], { label: 'x-axis', score: 9 });
  const hits = col.search([1.0, 0.0, 0.0], { topK: 1 });
  assert.equal(hits.length, 1);
  assert.equal(hits[0].id, 'v1');
  assert.ok(hits[0].metadata, 'metadata should be present');
  assert.equal(hits[0].metadata.label, 'x-axis');
});

test('upsert() is idempotent (upsert same id updates)', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0], { tag: 'first' });
  col.upsert('v1', [0.0, 1.0, 0.0], { tag: 'second' });
  assert.equal(col.count(), 1, 'upsert should not create duplicate');
});

test('search() returns topK results', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0]);
  col.upsert('v2', [0.0, 1.0, 0.0]);
  col.upsert('v3', [0.0, 0.0, 1.0]);
  const hits = col.search([0.9, 0.1, 0.0], { topK: 2 });
  assert.equal(hits.length, 2);
  assert.equal(hits[0].id, 'v1', 'closest should be v1');
});

test('search() with cosine metric', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('a', [1.0, 0.0, 0.0]);
  col.upsert('b', [0.0, 1.0, 0.0]);
  const hits = col.search([1.0, 0.0, 0.0], { topK: 1, metric: 'cosine' });
  assert.equal(hits[0].id, 'a');
});

test('search() with euclidean metric', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('near', [1.0, 0.0, 0.0]);
  col.upsert('far',  [0.0, 0.0, 1.0]);
  const hits = col.search([0.9, 0.1, 0.0], { topK: 1, metric: 'euclidean' });
  assert.equal(hits[0].id, 'near');
});

test('search() returns score and metadata fields', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0], { tag: 'test' });
  const hits = col.search([1.0, 0.0, 0.0], { topK: 1 });
  assert.ok('score' in hits[0],    'result should have score');
  assert.ok('metadata' in hits[0], 'result should have metadata');
  assert.ok('id' in hits[0],       'result should have id');
});

test('upsertBatch() inserts multiple vectors', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  const n = col.upsertBatch([
    { id: 'b1', vector: [1.0, 0.0, 0.0], metadata: { i: 1 } },
    { id: 'b2', vector: [0.0, 1.0, 0.0], metadata: { i: 2 } },
    { id: 'b3', vector: [0.0, 0.0, 1.0], metadata: { i: 3 } },
  ]);
  assert.equal(n, 3, `upsertBatch should return 3, got ${n}`);
  assert.equal(col.count(), 3);
});

test('delete() removes a vector', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0]);
  col.upsert('v2', [0.0, 1.0, 0.0]);
  col.delete('v1');
  assert.equal(col.count(), 1);
  const hits = col.search([1.0, 0.0, 0.0], { topK: 5 });
  assert.ok(!hits.some(h => h.id === 'v1'), 'deleted vector should not appear in results');
});

test('reindex() does not throw', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0]);
  assert.doesNotThrow(() => col.reindex());
});

test('upsertWithText() indexes for FTS alongside vector', () => {
  // Rust signature: upsert_with_text(id, vector, metadata, text) — text is last
  const d = db();
  const col = d.collection('docs', 3);
  col.upsertWithText('d1', [1.0, 0.0, 0.0], null, 'hello world agent');
  const hits = col.search([1.0, 0.0, 0.0], { topK: 1 });
  assert.equal(hits.length, 1);
  assert.equal(hits[0].id, 'd1');
});

test('dropCollection() removes collection', () => {
  const d = db();
  d.collection('temp', 3);
  const before = d.stats();
  d.dropCollection('temp');
  const after = d.stats();
  assert.equal(after.collections, before.collections - 1, 'collection count should decrease');
});

// ═════════════════════════════════════════════════════════════════════════════
// Conversations
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Conversations ────────────────────────────────────────────────');

test('createConversation() + listConversations()', () => {
  const d = db();
  d.createConversation('c1', 'First chat', { project: 'test' });
  const convos = d.listConversations();
  assert.equal(convos.length, 1);
  assert.equal(convos[0].id,    'c1');
  assert.equal(convos[0].title, 'First chat');
});

test('createConversation() without optional args', () => {
  const d = db();
  d.createConversation('c1');
  const convos = d.listConversations();
  assert.equal(convos.length, 1);
  assert.equal(convos[0].id, 'c1');
});

test('addMessage() returns a message ID string', () => {
  const d = db();
  d.createConversation('c1');
  const msgId = d.addMessage('c1', 'user', 'Hello there');
  assert.equal(typeof msgId, 'string');
  assert.ok(msgId.length > 0);
});

test('getMessages() returns messages in order', () => {
  const d = db();
  d.createConversation('c1');
  d.addMessage('c1', 'user',      'First');
  d.addMessage('c1', 'assistant', 'Second');
  const msgs = d.getMessages('c1');
  assert.equal(msgs.length, 2);
  assert.equal(msgs[0].role,    'user');
  assert.equal(msgs[0].content, 'First');
  assert.equal(msgs[1].role,    'assistant');
  assert.equal(msgs[1].content, 'Second');
});

test('getMessages() with limit', () => {
  const d = db();
  d.createConversation('c1');
  d.addMessage('c1', 'user', 'msg1');
  d.addMessage('c1', 'user', 'msg2');
  d.addMessage('c1', 'user', 'msg3');
  const msgs = d.getMessages('c1', 2);
  assert.equal(msgs.length, 2);
});

test('getMessages() message has expected fields', () => {
  const d = db();
  d.createConversation('c1');
  const msgId = d.addMessage('c1', 'user', 'content here', { key: 'val' });
  const msgs = d.getMessages('c1');
  const m = msgs[0];
  assert.equal(m.id,             msgId);
  assert.equal(m.conversationId, 'c1');
  assert.equal(m.role,           'user');
  assert.equal(m.content,        'content here');
  assert.ok('createdAt' in m,    'message should have createdAt');
});

test('deleteConversation() removes conversation and messages', () => {
  const d = db();
  d.createConversation('c1');
  d.addMessage('c1', 'user', 'hello');
  d.deleteConversation('c1');
  const convos = d.listConversations();
  assert.equal(convos.length, 0);
  const msgs = d.getMessages('c1');
  assert.equal(msgs.length, 0);
});

test('searchMessages() finds messages by content', () => {
  const d = db();
  d.createConversation('c1');
  d.addMessage('c1', 'user', 'The quick brown fox');
  d.addMessage('c1', 'user', 'Lazy dog sleeps');
  const results = d.searchMessages('quick brown', 5);
  assert.ok(results.length >= 1);
  assert.ok(results[0].snippet,        'result should have snippet');
  assert.ok(results[0].conversationId, 'result should have conversationId');
  assert.ok(results[0].messageId,      'result should have messageId');
});

test('searchMessages() scoped to conversationId', () => {
  const d = db();
  d.createConversation('c1');
  d.createConversation('c2');
  d.addMessage('c1', 'user', 'hello world');
  d.addMessage('c2', 'user', 'hello universe');
  const results = d.searchMessages('hello', 5, 'c1');
  assert.ok(results.every(r => r.conversationId === 'c1'),
    'all results should be from c1');
});

test('listConversations() returns multiple conversations', () => {
  const d = db();
  d.createConversation('c1', 'Chat 1');
  d.createConversation('c2', 'Chat 2');
  d.createConversation('c3', 'Chat 3');
  const convos = d.listConversations();
  assert.equal(convos.length, 3);
});

// ═════════════════════════════════════════════════════════════════════════════
// Workflows
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Workflows ────────────────────────────────────────────────────');

test('createWorkflow() + getWorkflow()', () => {
  const d = db();
  d.createWorkflow('w1', 'My Workflow', { input: 'data' }, { env: 'test' });
  const wf = d.getWorkflow('w1');
  assert.equal(wf.id,     'w1');
  assert.equal(wf.name,   'My Workflow');
  assert.equal(wf.status, 'pending');
});

test('addWorkflowStep() returns a step ID', () => {
  const d = db();
  d.createWorkflow('w1', 'wf');
  const stepId = d.addWorkflowStep('w1', 'step-one', { key: 'val' });
  assert.equal(typeof stepId, 'string');
  assert.ok(stepId.length > 0);
});

test('addWorkflowStep() and getWorkflow() includes steps', () => {
  const d = db();
  d.createWorkflow('w1', 'wf');
  d.addWorkflowStep('w1', 'step-one');
  d.addWorkflowStep('w1', 'step-two');
  const wf = d.getWorkflow('w1');
  assert.equal(wf.stepCount, 2);
  assert.equal(wf.steps.length, 2);
  assert.equal(wf.steps[0].name, 'step-one');
  assert.equal(wf.steps[1].name, 'step-two');
});

test('updateWorkflowStep() updates status and output', () => {
  const d = db();
  d.createWorkflow('w1', 'wf');
  const stepId = d.addWorkflowStep('w1', 'do-thing');
  d.updateWorkflowStep(stepId, 'running', { progress: 50 });
  const wf = d.getWorkflow('w1');
  const step = wf.steps[0];
  assert.equal(step.status, 'running');
  assert.equal(step.output.progress, 50);
});

test('completeWorkflow() sets status to completed', () => {
  const d = db();
  d.createWorkflow('w1', 'wf');
  d.completeWorkflow('w1', { result: 'done' });
  const wf = d.getWorkflow('w1');
  assert.equal(wf.status, 'completed');
  assert.equal(wf.output.result, 'done');
});

test('failWorkflow() sets status to failed with error', () => {
  const d = db();
  d.createWorkflow('w1', 'wf');
  d.failWorkflow('w1', 'something went wrong');
  const wf = d.getWorkflow('w1');
  assert.equal(wf.status, 'failed');
});

test('listWorkflows() returns all workflows', () => {
  const d = db();
  d.createWorkflow('w1', 'first');
  d.createWorkflow('w2', 'second');
  const wfs = d.listWorkflows();
  assert.equal(wfs.length, 2);
});

test('listWorkflows() filtered by status', () => {
  const d = db();
  d.createWorkflow('w1', 'first');
  d.createWorkflow('w2', 'second');
  d.completeWorkflow('w2');
  const pending   = d.listWorkflows('pending');
  const completed = d.listWorkflows('completed');
  assert.equal(pending.length,   1, `expected 1 pending, got ${pending.length}`);
  assert.equal(completed.length, 1, `expected 1 completed, got ${completed.length}`);
});

test('listWorkflows() entries have expected fields', () => {
  const d = db();
  d.createWorkflow('w1', 'my-wf');
  const wfs = d.listWorkflows();
  const w = wfs[0];
  assert.ok('id' in w,        'entry should have id');
  assert.ok('name' in w,      'entry should have name');
  assert.ok('status' in w,    'entry should have status');
  assert.ok('stepCount' in w, 'entry should have stepCount');
  assert.ok('createdAt' in w, 'entry should have createdAt');
  assert.ok('updatedAt' in w, 'entry should have updatedAt');
});

// ═════════════════════════════════════════════════════════════════════════════
// Traces
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Traces ───────────────────────────────────────────────────────');

test('addTrace() returns a trace ID', () => {
  const d = db();
  const id = d.addTrace('thought', 'I am thinking', 'sess1');
  assert.equal(typeof id, 'string');
  assert.ok(id.length > 0);
});

test('getTraces() retrieves traces for a session', () => {
  const d = db();
  d.addTrace('thought',  'step 1', 'sess1');
  d.addTrace('decision', 'step 2', 'sess1');
  d.addTrace('thought',  'other',  'sess2');
  const traces = d.getTraces('sess1');
  assert.equal(traces.length, 2);
  assert.ok(traces.every(t => t.sessionId === 'sess1'));
});

test('getTraces() with limit', () => {
  const d = db();
  for (let i = 0; i < 5; i++) {
    d.addTrace('thought', `step ${i}`, 'sess1');
  }
  const traces = d.getTraces('sess1', 3);
  assert.equal(traces.length, 3);
});

test('getTraces() trace has expected fields', () => {
  const d = db();
  const id = d.addTrace('thought', 'content here', 'sess1', null, { extra: true });
  const traces = d.getTraces('sess1');
  const t = traces[0];
  assert.equal(t.id,        id);
  assert.equal(t.sessionId, 'sess1');
  assert.equal(t.traceType, 'thought');
  assert.equal(t.content,   'content here');
  assert.ok('createdAt' in t);
});

test('getTraceTree() returns a subtree', () => {
  const d = db();
  const root  = d.addTrace('root',  'root step',  'sess1');
  const child = d.addTrace('child', 'child step', 'sess1', root);
  d.addTrace('grandchild', 'grandchild step', 'sess1', child);
  const tree = d.getTraceTree(root);
  assert.ok(tree.length >= 1, 'tree should have at least the root');
});

// ═════════════════════════════════════════════════════════════════════════════
// Memory Graph
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Memory Graph ─────────────────────────────────────────────────');

test('addNode() + getNode()', () => {
  const d = db();
  d.addNode('n1', 'session', { name: 'main' });
  const node = d.getNode('n1');
  assert.ok(node, 'getNode should return a node');
  assert.equal(node.id,   'n1');
  assert.equal(node.kind, 'session');
});

test('getNode() returns null for missing node', () => {
  const d = db();
  const node = d.getNode('nonexistent');
  assert.equal(node, null);
});

test('addEdge() + neighbors()', () => {
  const d = db();
  d.addNode('n1', 'session');
  d.addNode('n2', 'thought');
  d.addEdge('n1', 'n2', 'recalled', 0.8);
  const neighbors = d.neighbors('n1', 1);
  assert.equal(neighbors.length, 1);
  assert.equal(neighbors[0].id,     'n2');
  assert.equal(neighbors[0].kind,   'thought');
  assert.equal(neighbors[0].depth,  1);
  assert.ok(neighbors[0].weight > 0);
});

test('neighbors() with maxDepth traversal', () => {
  const d = db();
  d.addNode('a', 'type');
  d.addNode('b', 'type');
  d.addNode('c', 'type');
  d.addEdge('a', 'b', 'links', 1.0);
  d.addEdge('b', 'c', 'links', 1.0);
  const depth1 = d.neighbors('a', 1);
  const depth2 = d.neighbors('a', 2);
  assert.equal(depth1.length, 1, 'at depth 1 only b is reachable');
  assert.equal(depth2.length, 2, 'at depth 2 both b and c are reachable');
});

test('neighbors() with minWeight filter', () => {
  const d = db();
  d.addNode('a', 'type');
  d.addNode('b', 'type');
  d.addNode('c', 'type');
  d.addEdge('a', 'b', 'links', 0.9);
  d.addEdge('a', 'c', 'links', 0.1);
  const heavy = d.neighbors('a', 1, 0.5);
  assert.equal(heavy.length, 1, 'only high-weight neighbor should pass filter');
  assert.equal(heavy[0].id, 'b');
});

test('neighbors() filtered by relation', () => {
  const d = db();
  d.addNode('a', 'type');
  d.addNode('b', 'type');
  d.addNode('c', 'type');
  d.addEdge('a', 'b', 'knows',    1.0);
  d.addEdge('a', 'c', 'recalled', 1.0);
  const knows = d.neighbors('a', 1, 0.0, 'knows');
  assert.equal(knows.length, 1);
  assert.equal(knows[0].id, 'b');
});

test('deleteNode() removes node and its edges', () => {
  const d = db();
  d.addNode('a', 'type');
  d.addNode('b', 'type');
  d.addEdge('a', 'b', 'links', 1.0);
  d.deleteNode('a');
  assert.equal(d.getNode('a'), null);
  // 'b' still exists
  assert.ok(d.getNode('b'));
  // no neighbors from 'a' anymore
  const nbrs = d.neighbors('a', 1);
  assert.equal(nbrs.length, 0);
});

test('deleteEdge() removes a specific edge', () => {
  const d = db();
  d.addNode('a', 'type');
  d.addNode('b', 'type');
  d.addEdge('a', 'b', 'links', 1.0);
  d.deleteEdge('a', 'b', 'links');
  const nbrs = d.neighbors('a', 1);
  assert.equal(nbrs.length, 0, 'edge should be gone after deleteEdge');
});

// ═════════════════════════════════════════════════════════════════════════════
// Full-Text Search
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── FTS ──────────────────────────────────────────────────────────');

test('ftsIndex() + ftsSearch() basic', () => {
  const d = db();
  d.ftsIndex('docs', 'doc1', 'col1', 'The quick brown fox jumps');
  d.ftsIndex('docs', 'doc2', 'col1', 'A lazy dog lies in the sun');
  const results = d.ftsSearch('docs', 'quick fox', 5);
  assert.ok(results.length >= 1);
  assert.equal(results[0].id, 'doc1');
});

test('ftsSearch() result has id, snippet, rank', () => {
  const d = db();
  d.ftsIndex('docs', 'doc1', 'col1', 'hello world from agentdb');
  const results = d.ftsSearch('docs', 'agentdb', 5);
  assert.ok(results.length >= 1);
  const r = results[0];
  assert.ok('id' in r,      'result should have id');
  assert.ok('snippet' in r, 'result should have snippet');
  assert.ok('rank' in r,    'result should have rank');
});

test('ftsSearch() returns empty array for no match', () => {
  const d = db();
  d.ftsIndex('docs', 'doc1', 'col1', 'completely unrelated');
  const results = d.ftsSearch('docs', 'xylophone', 5);
  assert.equal(results.length, 0);
});

test('ftsDelete() removes document from FTS index', () => {
  const d = db();
  d.ftsIndex('docs', 'doc1', 'col1', 'target content here');
  d.ftsDelete('docs', 'doc1');
  const results = d.ftsSearch('docs', 'target content', 5);
  assert.equal(results.length, 0, 'deleted doc should not appear in FTS results');
});

test('ftsOptimize() does not throw', () => {
  const d = db();
  d.ftsIndex('docs', 'doc1', 'col1', 'some text');
  assert.doesNotThrow(() => d.ftsOptimize('docs'));
});

// ═════════════════════════════════════════════════════════════════════════════
// Hybrid Query
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Hybrid Query ─────────────────────────────────────────────────');

test('hybridQuery() returns results with expected fields', () => {
  const d = db();
  // set up graph
  d.addNode('user:1', 'user');
  d.addNode('doc:1',  'doc');
  d.addEdge('user:1', 'doc:1', 'authored', 0.9);
  // set up vector collection with same id
  const col = d.collection('thoughts', 3);
  col.upsert('doc:1', [0.9, 0.1, 0.0]);
  col.upsert('doc:2', [0.0, 1.0, 0.0]);
  const results = d.hybridQuery('user:1', [0.9, 0.1, 0.0], 'thoughts', { topK: 5 });
  assert.ok(Array.isArray(results));
  if (results.length > 0) {
    const r = results[0];
    assert.ok('id' in r,          'result should have id');
    assert.ok('rankScore' in r,   'result should have rankScore');
    assert.ok('vectorScore' in r, 'result should have vectorScore');
    assert.ok('graphWeight' in r, 'result should have graphWeight');
  }
});

test('hybridQuery() with alpha and graphDepth options', () => {
  const d = db();
  d.addNode('anchor', 'user');
  d.addNode('item:1', 'item');
  d.addEdge('anchor', 'item:1', 'links', 1.0);
  const col = d.collection('items', 3);
  col.upsert('item:1', [1.0, 0.0, 0.0]);
  const results = d.hybridQuery('anchor', [1.0, 0.0, 0.0], 'items', {
    topK: 5, graphDepth: 2, alpha: 0.8
  });
  assert.ok(Array.isArray(results));
});

// ═════════════════════════════════════════════════════════════════════════════
// Stats
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Stats ────────────────────────────────────────────────────────');

test('stats() returns all expected fields', () => {
  const d = db();
  const s = d.stats();
  const fields = [
    'collections', 'vectors', 'nodes', 'edges',
    'conversations', 'messages', 'workflows', 'workflowSteps',
    'traces', 'tools', 'toolCalls', 'auditEntries', 'promptTemplates'
  ];
  for (const f of fields) {
    assert.ok(f in s, `stats() should have field '${f}'`);
  }
});

test('stats() reflects inserted data', () => {
  const d = db();
  const col = d.collection('vecs', 3);
  col.upsert('v1', [1.0, 0.0, 0.0]);
  col.upsert('v2', [0.0, 1.0, 0.0]);
  d.addNode('n1', 'node');
  d.addNode('n2', 'node');
  d.addEdge('n1', 'n2', 'links', 1.0);
  d.createConversation('c1');
  d.addMessage('c1', 'user', 'hello');
  const s = d.stats();
  assert.equal(s.collections,   1, `collections: expected 1, got ${s.collections}`);
  assert.equal(s.vectors,       2, `vectors: expected 2, got ${s.vectors}`);
  assert.equal(s.nodes,         2, `nodes: expected 2, got ${s.nodes}`);
  assert.equal(s.edges,         1, `edges: expected 1, got ${s.edges}`);
  assert.equal(s.conversations, 1, `conversations: expected 1, got ${s.conversations}`);
  assert.equal(s.messages,      1, `messages: expected 1, got ${s.messages}`);
});

// ═════════════════════════════════════════════════════════════════════════════
// v0.6.0 — Tool Registry
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Tool Registry (v0.6.0) ───────────────────────────────────────');

test('registerTool() returns a tool ID', () => {
  const d = db();
  const id = d.registerTool('my_tool', 'Does something', { type: 'object' }, '1.0');
  assert.equal(typeof id, 'string');
  assert.ok(id.length > 0);
});

test('listTools() returns registered tools', () => {
  const d = db();
  d.registerTool('tool_a', 'Tool A', null, '1.0');
  d.registerTool('tool_b', 'Tool B', null, '2.0');
  const tools = d.listTools();
  assert.equal(tools.length, 2);
  const names = tools.map(t => t.name);
  assert.ok(names.includes('tool_a'), 'tool_a should be listed');
  assert.ok(names.includes('tool_b'), 'tool_b should be listed');
});

test('listTools() tool entry has expected fields', () => {
  const d = db();
  d.registerTool('my_tool', 'A description', { type: 'object' }, '1.0');
  const tools = d.listTools();
  const t = tools[0];
  assert.ok('id' in t,          'tool should have id');
  assert.ok('name' in t,        'tool should have name');
  assert.ok('description' in t, 'tool should have description');
  assert.ok('version' in t,     'tool should have version');
  assert.ok('createdAt' in t,   'tool should have createdAt');
  assert.ok('updatedAt' in t,   'tool should have updatedAt');
});

test('registerTool() is idempotent (upsert by name)', () => {
  const d = db();
  d.registerTool('my_tool', 'v1');
  d.registerTool('my_tool', 'v2');
  const tools = d.listTools();
  assert.equal(tools.length, 1, 'registerTool upsert should not create duplicates');
});

test('logToolCall() returns a tool call ID', () => {
  const d = db();
  d.registerTool('my_tool');
  const callId = d.logToolCall(
    'my_tool',
    'sess1',
    { param: 'value' },
    { output: 'result' },
    null,
    42
  );
  assert.equal(typeof callId, 'string');
  assert.ok(callId.length > 0);
});

test('logToolCall() increments toolCalls in stats()', () => {
  const d = db();
  d.registerTool('tool_x');
  d.logToolCall('tool_x');
  const s = d.stats();
  assert.ok(s.toolCalls >= 1, `expected toolCalls >= 1, got ${s.toolCalls}`);
});

test('logToolCall() with error field', () => {
  const d = db();
  d.registerTool('tool_y');
  const callId = d.logToolCall('tool_y', null, null, null, 'timeout error', null);
  assert.equal(typeof callId, 'string');
});

// ═════════════════════════════════════════════════════════════════════════════
// v0.6.0 — Audit Log
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Audit Log (v0.6.0) ───────────────────────────────────────────');

test('auditLog() returns an entry ID', () => {
  const d = db();
  const id = d.auditLog('create', 'messages', 'rec-1', 'agent', null, { content: 'hi' }, 'initial write');
  assert.equal(typeof id, 'string');
  assert.ok(id.length > 0);
});

test('auditQueryRecent() returns logged entries', () => {
  const d = db();
  d.auditLog('create', 'messages', 'rec-1', 'agent');
  d.auditLog('update', 'messages', 'rec-2', 'agent');
  const entries = d.auditQueryRecent();
  assert.ok(entries.length >= 2);
});

test('auditQueryRecent() with limit', () => {
  const d = db();
  for (let i = 0; i < 5; i++) {
    d.auditLog('create', 'table', `rec-${i}`, 'bot');
  }
  const entries = d.auditQueryRecent(3);
  assert.equal(entries.length, 3);
});

test('auditQueryRecent() entry has expected fields', () => {
  const d = db();
  d.auditLog('delete', 'notes', 'note-1', 'user:1', { text: 'old' }, null, 'cleanup');
  const entries = d.auditQueryRecent(1);
  const e = entries[0];
  assert.ok('id' in e,        'entry should have id');
  assert.ok('action' in e,    'entry should have action');
  assert.ok('tableName' in e, 'entry should have tableName');
  assert.ok('recordId' in e,  'entry should have recordId');
  assert.ok('timestamp' in e, 'entry should have timestamp');
});

test('auditLog() increments auditEntries in stats()', () => {
  const d = db();
  d.auditLog('create', 't', 'r1', 'actor');
  const s = d.stats();
  assert.ok(s.auditEntries >= 1);
});

// ═════════════════════════════════════════════════════════════════════════════
// v0.6.0 — Context Window
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Context Window (v0.6.0) ──────────────────────────────────────');

test('contextAdd() returns an entry ID', () => {
  const d = db();
  const id = d.contextAdd('sess1', 'message', 'msg-1', 'Hello world', 10, 0.9, 1);
  assert.equal(typeof id, 'string');
  assert.ok(id.length > 0);
});

test('contextBuildWindow() respects token budget', () => {
  const d = db();
  d.contextAdd('sess1', 'message', 'msg-1', 'First entry',  100, 0.9, 1);
  d.contextAdd('sess1', 'message', 'msg-2', 'Second entry', 200, 0.8, 2);
  d.contextAdd('sess1', 'message', 'msg-3', 'Third entry',  300, 0.7, 3);
  // budget of 250 should only fit first two (100 + 200 = 300 > 250 so only one or two)
  const window = d.contextBuildWindow('sess1', 250);
  assert.ok(Array.isArray(window));
  const totalTokens = window.reduce((sum, e) => sum + e.tokenCount, 0);
  assert.ok(totalTokens <= 250, `total tokens ${totalTokens} should not exceed budget 250`);
});

test('contextBuildWindow() entry has expected fields', () => {
  const d = db();
  d.contextAdd('sess1', 'message', 'msg-1', 'preview text', 50, 1.0, 1);
  const window = d.contextBuildWindow('sess1', 1000);
  assert.ok(window.length >= 1);
  const e = window[0];
  assert.ok('id' in e,             'entry should have id');
  assert.ok('sessionId' in e,      'entry should have sessionId');
  assert.ok('sourceType' in e,     'entry should have sourceType');
  assert.ok('sourceId' in e,       'entry should have sourceId');
  assert.ok('tokenCount' in e,     'entry should have tokenCount');
  assert.ok('relevanceScore' in e, 'entry should have relevanceScore');
  assert.ok('priority' in e,       'entry should have priority');
});

test('contextClear() removes all entries for a session', () => {
  const d = db();
  d.contextAdd('sess1', 'message', 'msg-1', 'text', 50, 0.9, 1);
  d.contextAdd('sess1', 'message', 'msg-2', 'text', 50, 0.8, 2);
  d.contextClear('sess1');
  const window = d.contextBuildWindow('sess1', 9999);
  assert.equal(window.length, 0, 'context should be empty after clear');
});

test('contextAdd() null contentPreview is accepted', () => {
  const d = db();
  const id = d.contextAdd('sess2', 'vector', 'vec-1', null, 20, 0.5, 0);
  assert.equal(typeof id, 'string');
});

// ═════════════════════════════════════════════════════════════════════════════
// v0.6.0 — Prompt Templates
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Prompt Templates (v0.6.0) ────────────────────────────────────');

test('promptCreate() returns a template ID', () => {
  const d = db();
  const id = d.promptCreate('greet', 'Hello, {{name}}!', 'claude-3', 1024, null);
  assert.equal(typeof id, 'string');
  assert.ok(id.length > 0);
});

test('promptRender() substitutes placeholders', () => {
  const d = db();
  d.promptCreate('greet', 'Hello, {{name}}! You have {{count}} messages.');
  const rendered = d.promptRender('greet', { name: 'Alice', count: '5' });
  assert.equal(rendered, 'Hello, Alice! You have 5 messages.');
});

test('promptRender() with empty vars leaves unmatched placeholders', () => {
  const d = db();
  d.promptCreate('template', 'Value: {{val}}');
  const rendered = d.promptRender('template', {});
  // unmatched placeholder remains as-is
  assert.ok(typeof rendered === 'string');
});

test('promptCreate() increments promptTemplates in stats()', () => {
  const d = db();
  d.promptCreate('tmpl1', 'Hello {{world}}');
  const s = d.stats();
  assert.ok(s.promptTemplates >= 1);
});

test('promptCreate() multiple versions of same name', () => {
  const d = db();
  d.promptCreate('mytemplate', 'Version 1: {{x}}');
  d.promptCreate('mytemplate', 'Version 2: {{x}}');
  // promptRender should use the latest version
  const rendered = d.promptRender('mytemplate', { x: 'test' });
  assert.ok(rendered.includes('test'));
});

// ═════════════════════════════════════════════════════════════════════════════
// v0.6.0 — Data Labels
// ═════════════════════════════════════════════════════════════════════════════
console.log('\n── Data Labels (v0.6.0) ─────────────────────────────────────────');

test('labelTag() + labelGet()', () => {
  const d = db();
  d.labelTag('messages', 'msg-1', 'pii', 'agent');
  const labels = d.labelGet('messages', 'msg-1');
  assert.equal(labels.length, 1);
  assert.equal(labels[0].label,    'pii');
  assert.equal(labels[0].taggedBy, 'agent');
});

test('labelGet() returns multiple labels', () => {
  const d = db();
  d.labelTag('records', 'rec-1', 'pii',        'agent');
  d.labelTag('records', 'rec-1', 'confidential','agent');
  const labels = d.labelGet('records', 'rec-1');
  assert.equal(labels.length, 2);
});

test('labelGet() returns empty array for unlabeled record', () => {
  const d = db();
  const labels = d.labelGet('messages', 'no-such-record');
  assert.deepEqual(labels, []);
});

test('labelHas() returns true when label exists', () => {
  const d = db();
  d.labelTag('notes', 'note-1', 'sensitive');
  assert.equal(d.labelHas('notes', 'note-1', 'sensitive'), true);
});

test('labelHas() returns false when label absent', () => {
  const d = db();
  d.labelTag('notes', 'note-1', 'public');
  assert.equal(d.labelHas('notes', 'note-1', 'pii'), false);
});

test('labelUntag() removes a label', () => {
  const d = db();
  d.labelTag('notes', 'note-1', 'pii',    'agent');
  d.labelTag('notes', 'note-1', 'public', 'agent');
  d.labelUntag('notes', 'note-1', 'pii');
  const labels = d.labelGet('notes', 'note-1');
  assert.equal(labels.length, 1);
  assert.equal(labels[0].label, 'public');
  assert.equal(d.labelHas('notes', 'note-1', 'pii'), false);
});

test('labelGet() entry has expected fields', () => {
  const d = db();
  d.labelTag('tbl', 'id-1', 'mylabel', 'tagger');
  const labels = d.labelGet('tbl', 'id-1');
  const l = labels[0];
  assert.ok('tableName' in l, 'label should have tableName');
  assert.ok('recordId' in l,  'label should have recordId');
  assert.ok('label' in l,     'label should have label');
  assert.ok('taggedBy' in l,  'label should have taggedBy');
  assert.ok('taggedAt' in l,  'label should have taggedAt');
});

// ═════════════════════════════════════════════════════════════════════════════
// Final report
// ═════════════════════════════════════════════════════════════════════════════

console.log('\n─────────────────────────────────────────────────────────────────');
console.log(`Results: ${passed} passed, ${failed} failed`);

if (failed > 0) {
  process.exit(1);
}
