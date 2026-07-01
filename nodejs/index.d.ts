/**
 * AgentDB — single-file embedded database for AI agents.
 *
 * SQL + Vector Search + Full-Text Search + Hybrid Queries + Memory Graphs +
 * Conversation Threading + Workflow Persistence + Reasoning Traces.
 *
 * @example
 * ```ts
 * import { AgentDB } from '@datacules/agentdb';
 *
 * const db = AgentDB.open(':memory:');
 *
 * // SQL
 * db.execute('CREATE TABLE sessions (id TEXT PRIMARY KEY)');
 *
 * // Vectors
 * const col = db.collection('thoughts', 4);
 * col.upsert('t1', [0.9, 0.1, 0.0, 0.0], { score: 9 });
 * const results = col.search([0.9, 0.1, 0.0, 0.0], { topK: 5 });
 *
 * // Memory graph
 * db.addNode('s1', 'session');
 * db.addNode('t1', 'thought');
 * db.addEdge('s1', 't1', 'recalled', 0.9);
 *
 * const stats = db.stats();
 * console.log(stats);
 * ```
 */

/**
 * Distance metric used when searching a vector collection.
 *
 * - `'cosine'`     — cosine similarity (default; best for normalized embeddings)
 * - `'euclidean'`  — Euclidean / L2 distance (lower is closer)
 * - `'dot'`        — dot product (highest is closest; requires unit vectors)
 */
export type DistanceMetric = 'cosine' | 'euclidean' | 'dot';

export interface SearchResult {
  id:       string;
  score:    number;
  metadata: Record<string, unknown> | null;
}

export interface FtsResult {
  id:      string;
  snippet: string;
  rank:    number;
}

export interface HybridResult {
  id:          string;
  rankScore:   number;
  vectorScore: number;
  graphWeight: number;
}

export interface NeighborResult {
  id:     string;
  kind:   string;
  depth:  number;
  weight: number;
  data:   Record<string, unknown> | null;
}

export interface DbStats {
  collections:   number;
  vectors:       number;
  nodes:         number;
  edges:         number;
  conversations: number;
  messages:      number;
  workflows:     number;
  workflowSteps: number;
  traces:        number;
}

export interface SearchOptions {
  topK?:   number;
  filter?: Record<string, unknown>;
  /** Distance metric to use for this search. Defaults to `'cosine'`. */
  metric?: DistanceMetric;
}

export interface HybridOptions {
  graphDepth?: number;
  topK?:       number;
  alpha?:      number;
  filter?:     Record<string, unknown>;
}

export interface Conversation {
  id:        string;
  title:     string | null;
  metadata:  Record<string, unknown> | null;
  createdAt: number;
  updatedAt: number;
}

export interface Message {
  id:             string;
  conversationId: string;
  role:           string;
  content:        string;
  metadata:       Record<string, unknown> | null;
  createdAt:      number;
}

export interface MessageSearchResult {
  messageId:      string;
  conversationId: string;
  snippet:        string;
  rank:           number;
}

export interface Workflow {
  id:        string;
  name:      string;
  status:    string;
  input:     Record<string, unknown> | null;
  output:    Record<string, unknown> | null;
  error:     string | null;
  metadata:  Record<string, unknown> | null;
  createdAt: number;
  updatedAt: number;
  /** Total step count (always populated; full step objects only via `getWorkflow`). */
  stepCount: number;
  steps:     WorkflowStep[];
}

export interface WorkflowStep {
  id:          string;
  workflowId:  string;
  stepIndex:   number;
  name:        string;
  status:      string;
  input:       Record<string, unknown> | null;
  output:      Record<string, unknown> | null;
  error:       string | null;
  startedAt:   number | null;
  completedAt: number | null;
}

export interface Trace {
  id:        string;
  sessionId: string | null;
  parentId:  string | null;
  traceType: string;
  content:   string;
  metadata:  Record<string, unknown> | null;
  createdAt: number;
}

/** A vector collection handle. */
export class Collection {
  /** Upsert a single vector. */
  upsert(id: string, vector: number[], metadata?: Record<string, unknown> | null): void;

  /** Upsert a vector and atomically index its text for FTS. */
  upsertWithText(id: string, vector: number[], text: string, metadata?: Record<string, unknown> | null): void;

  /** Upsert multiple vectors in a single transaction. */
  upsertBatch(entries: Array<{ id: string; vector: number[]; metadata?: Record<string, unknown> | null }>): number;

  /** Approximate nearest-neighbor search (synchronous). */
  search(query: number[], options?: SearchOptions): SearchResult[];

  /** Delete a vector by ID. */
  delete(id: string): void;

  /** Number of vectors in this collection. */
  count(): number;

  /** Rebuild the HNSW index. */
  reindex(): void;
}

/** Main AgentDB connection. */
export class AgentDB {
  /** Open or create an AgentDB database at the given path. */
  static open(path: string): AgentDB;

  /** Execute a raw SQL statement. Returns rows affected. */
  execute(sql: string): number;

  /** Query and return rows as an array of plain objects. */
  query(sql: string): Record<string, unknown>[];

  /** Get or create a vector collection with the given dimensionality. */
  collection(name: string, dim: number): Collection;

  /** Drop a vector collection by name. */
  dropCollection(name: string): void;

  // ── Memory Graph ──────────────────────────────────────────────────

  /** Add or update a memory graph node. */
  addNode(id: string, kind: string, data?: Record<string, unknown> | null): void;

  /** Get a single node by ID, or null if it doesn't exist. */
  getNode(id: string): Record<string, unknown> | null;

  /** Delete a node and its edges. */
  deleteNode(id: string): void;

  /** Add or update a directed edge in the memory graph. */
  addEdge(src: string, dst: string, relation: string, weight: number): void;

  /** Delete a specific edge. */
  deleteEdge(src: string, dst: string, relation: string): void;

  /** Traverse the memory graph from a node. */
  neighbors(nodeId: string, maxDepth?: number, minWeight?: number): NeighborResult[];

  // ── Full-Text Search ──────────────────────────────────────────────

  /** Index text for full-text search. */
  ftsIndex(collection: string, id: string, collectionId: string, text: string): void;

  /** Full-text search over a collection. */
  ftsSearch(collection: string, query: string, topK: number): FtsResult[];

  /** Delete a text entry from the FTS index. */
  ftsDelete(collection: string, id: string): void;

  /** Run FTS5 optimize on a collection. */
  ftsOptimize(collection: string): void;

  /** Run a hybrid graph + vector query (synchronous). */
  hybridQuery(anchorNode: string, embedding: number[], collection: string, options?: HybridOptions): HybridResult[];

  // ── Conversations ──────────────────────────────────────────────────

  /** Create a new conversation thread. */
  createConversation(id: string, title?: string | null, metadata?: Record<string, unknown> | null): void;

  /** Append a message to a conversation. */
  addMessage(conversationId: string, role: string, content: string, metadata?: Record<string, unknown> | null): string;

  /** Retrieve messages in a conversation, optionally limited. */
  getMessages(conversationId: string, limit?: number | null): Message[];

  /** List all conversations. */
  listConversations(): Conversation[];

  /** Delete a conversation and all its messages. */
  deleteConversation(id: string): void;

  /** Full-text search over all message content. */
  searchMessages(query: string, topK: number, conversationId?: string | null): MessageSearchResult[];

  // ── Workflows ──────────────────────────────────────────────────────

  /** Create a new workflow in pending status. */
  createWorkflow(id: string, name: string, input?: Record<string, unknown> | null, metadata?: Record<string, unknown> | null): void;

  /** Add a step to a workflow. Returns the step ID. */
  addWorkflowStep(workflowId: string, name: string, input?: Record<string, unknown> | null): string;

  /** Update a step's status, output, and/or error. */
  updateWorkflowStep(stepId: string, status: string, output?: Record<string, unknown> | null, error?: string | null): void;

  /** Mark a workflow as completed. */
  completeWorkflow(id: string, output?: Record<string, unknown> | null): void;

  /** Mark a workflow as failed. */
  failWorkflow(id: string, error?: string | null): void;

  /** Get a workflow and all its steps. */
  getWorkflow(id: string): Workflow;

  /** List workflows, optionally filtered by status. */
  listWorkflows(statusFilter?: string | null): Workflow[];

  // ── Traces ─────────────────────────────────────────────────────────

  /** Add a reasoning trace entry. Returns the trace ID. */
  addTrace(traceType: string, content: string, sessionId?: string | null, parentId?: string | null, metadata?: Record<string, unknown> | null): string;

  /** Get traces for a session with optional pagination. */
  getTraces(sessionId: string, limit?: number | null, offset?: number | null): Trace[];

  /** Get a subtree of traces rooted at a trace ID. */
  getTraceTree(traceId: string): Trace[];

  // ── Stats ──────────────────────────────────────────────────────────

  /** Return database-wide statistics. */
  stats(): DbStats;
}
