/**
 * AgentDB — single-file embedded database for AI agents.
 *
 * SQL + Vector Search + Full-Text Search + Hybrid Queries + Memory Graphs.
 *
 * @example
 * ```ts
 * import { AgentDB } from 'agentdb';
 *
 * const db = AgentDB.open(':memory:');
 *
 * // SQL
 * db.execute('CREATE TABLE sessions (id TEXT PRIMARY KEY)');
 *
 * // Vectors
 * const col = db.collection('thoughts', 4);
 * col.upsert('t1', [0.9, 0.1, 0.0, 0.0], { score: 9 });
 * const results = await col.search([0.9, 0.1, 0.0, 0.0], { topK: 5 });
 *
 * // Memory graph
 * db.addNode('s1', 'session');
 * db.addNode('t1', 'thought');
 * db.addEdge('s1', 't1', 'recalled', 0.9);
 *
 * const stats = db.stats();
 * console.log(stats); // { collections: 1, vectors: 1, nodes: 2, edges: 1 }
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
  collections: number;
  vectors:     number;
  nodes:       number;
  edges:       number;
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
}

/** A vector collection handle. */
export class Collection {
  /** Upsert a single vector. */
  upsert(id: string, vector: number[], metadata?: Record<string, unknown> | null): void;

  /** Upsert multiple vectors in a single transaction. */
  upsertBatch(entries: Array<{ id: string; vector: number[]; metadata?: Record<string, unknown> | null }>): number;

  /** Approximate nearest-neighbor search. */
  search(query: number[], options?: SearchOptions): SearchResult[];

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

  /** Add or update a memory graph node. */
  addNode(id: string, kind: string, data?: Record<string, unknown> | null): void;

  /** Add or update a directed edge in the memory graph. */
  addEdge(src: string, dst: string, relation: string, weight: number): void;

  /** Traverse the memory graph from a node. */
  neighbors(nodeId: string, maxDepth?: number, minWeight?: number): NeighborResult[];

  /** Index text for full-text search. */
  ftsIndex(collection: string, id: string, collectionId: string, text: string): void;

  /** Full-text search over a collection. */
  ftsSearch(collection: string, query: string, topK: number): FtsResult[];

  /** Run a hybrid graph + vector query. */
  hybridQuery(anchorNode: string, embedding: number[], collection: string, options?: HybridOptions): HybridResult[];

  /** Return database-wide statistics. */
  stats(): DbStats;
}
