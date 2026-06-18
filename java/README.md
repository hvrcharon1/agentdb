# AgentDB — Java SDK

Java JNI bindings for [AgentDB](https://github.com/hvrcharon1/agentdb).

## Prerequisites

### 1. Build the native shared library

```bash
# From the repository root
cargo build --release --features ffi --lib

# Output:
#   Linux   → target/release/libagentdb.so
#   macOS   → target/release/libagentdb.dylib
#   Windows → target/release/agentdb.dll
```

### 2. Make the library visible to the JVM

```bash
# Option A — set java.library.path at runtime
java -Djava.library.path=/path/to/agentdb/target/release -jar myapp.jar

# Option B — copy to a system library directory (Linux)
sudo cp target/release/libagentdb.so /usr/local/lib/
sudo ldconfig

# Option C — call AgentDB.loadLibrary(absolutePath) from Java before open()
AgentDB.loadLibrary("/path/to/libagentdb.so");
```

### 3. Build the Java SDK

```bash
cd java
mvn package -Dagentdb.native.lib=/path/to/target/release
```

## Usage

```java
import com.datacules.agentdb.AgentDB;
import com.datacules.agentdb.AgentDBException;

public class Example {
    public static void main(String[] args) {
        // Open (or create) a database
        try (AgentDB db = AgentDB.open("agent.db")) {

            // SQL
            db.execute("CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, data TEXT)");
            db.execute("INSERT OR REPLACE INTO sessions VALUES ('s1','{\"turns\":3}')");
            String rows = db.queryJson("SELECT * FROM sessions");
            System.out.println(rows);

            // Vector upsert and search
            float[] embedding = {0.1f, 0.2f, 0.3f, 0.4f};
            db.vectorUpsert("docs", "doc-1", embedding, "{\"title\":\"hello\"}");
            String results = db.vectorSearch("docs", embedding, 5, null);
            System.out.println(results);

            // Memory graph
            db.graphAddNode("session:s1", "session", "{\"agent\":\"gpt-4o\"}");
            db.graphAddNode("concept:llm", "concept", null);
            db.graphAddEdge("session:s1", "concept:llm", "mentions", 0.9);
            String neighbors = db.graphNeighbors("session:s1", 2, 0.5);
            System.out.println(neighbors);

            // Full-text search
            db.ftsIndex("docs", "doc-1", "docs", "AgentDB is an embedded AI-agent database");
            String fts = db.ftsSearch("docs", "embedded database", 5);
            System.out.println(fts);

            // Hybrid query
            String hybrid = db.hybridQuery("session:s1", embedding, "docs", 2, 5, 0.5);
            System.out.println(hybrid);

            // Stats
            System.out.println(db.stats());

        } catch (AgentDBException e) {
            System.err.println("AgentDB error: " + e.getMessage());
        }
    }
}
```

## API reference

| Method | Returns | Description |
|--------|---------|-------------|
| `AgentDB.open(path)` | `AgentDB` | Open/create database. Use `":memory:"` for ephemeral. |
| `db.close()` / try-with-resources | `void` | Release native resources. |
| `db.execute(sql)` | `long` rows affected | DDL/DML, no result rows. |
| `db.queryJson(sql)` | `String` JSON array | SELECT → JSON rows. |
| `db.vectorUpsert(col, id, vec, meta)` | `void` | Upsert vector with optional JSON metadata. |
| `db.vectorSearch(col, query, topK, filter)` | `String` JSON array | ANN search. |
| `db.graphAddNode(id, kind, data)` | `void` | Upsert memory-graph node. |
| `db.graphAddEdge(src, dst, rel, weight)` | `void` | Upsert directed edge. |
| `db.graphNeighbors(id, depth, minW)` | `String` JSON array | BFS/DFS traversal. |
| `db.ftsIndex(col, vecId, colId, text)` | `void` | Index document text. |
| `db.ftsSearch(col, query, topK)` | `String` JSON array | FTS with snippets. |
| `db.hybridQuery(anchor, emb, col, depth, k, α)` | `String` JSON array | Blended graph + vector ranking. |
| `db.stats()` | `String` JSON object | Collection / vector / node / edge counts. |

All methods throw `AgentDBException` (unchecked) on native errors.

## JNI glue (for native library authors)

The Java class declares these native methods; the JNI implementation lives in
the Rust crate under `src/jni.rs` (built into the shared library):

```
Java_com_datacules_agentdb_AgentDB_nativeOpen
Java_com_datacules_agentdb_AgentDB_nativeClose
Java_com_datacules_agentdb_AgentDB_nativeExecute
Java_com_datacules_agentdb_AgentDB_nativeQueryJson
Java_com_datacules_agentdb_AgentDB_nativeVectorUpsert
Java_com_datacules_agentdb_AgentDB_nativeVectorSearch
Java_com_datacules_agentdb_AgentDB_nativeGraphAddNode
Java_com_datacules_agentdb_AgentDB_nativeGraphAddEdge
Java_com_datacules_agentdb_AgentDB_nativeGraphNeighbors
Java_com_datacules_agentdb_AgentDB_nativeFtsIndex
Java_com_datacules_agentdb_AgentDB_nativeFtsSearch
Java_com_datacules_agentdb_AgentDB_nativeHybridQuery
Java_com_datacules_agentdb_AgentDB_nativeStats
Java_com_datacules_agentdb_AgentDB_nativeLastError
```
