# Security Policy

## Supported Versions

| Version | Supported |
|---|---|
| 0.3.x (latest) | ✅ Receives security patches |
| 0.2.x | ⚠️ Critical fixes only |
| 0.1.x | ❌ End of life |

---

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Email **security@datacules.com** with:

1. A description of the vulnerability and its potential impact.
2. Steps to reproduce (a proof-of-concept code snippet is welcome).
3. The AgentDB version(s) affected.
4. Any suggested fix, if you have one.

### What to expect

- **Acknowledgement within 48 hours** of your report.
- **Status update within 7 days** (confirmed / not reproduced / needs more info).
- **Fix timeline**: critical (CVSS ≥ 9.0) within 14 days; high (CVSS 7.0–8.9) within 30 days; medium/low within 90 days.
- **Credit**: reporters who wish to be credited will be acknowledged in the CHANGELOG and release notes when the fix ships.

We follow a coordinated disclosure model. Please give us reasonable time to release a fix before publishing details publicly.

---

## Security Scope

In scope:

- SQL injection via the FFI (`agentdb_execute`) or CLI (`agentdb sql`) if user input is passed without parameterization.
- Memory safety issues in the Rust core, FFI layer, or WASM module.
- Arbitrary code execution or privilege escalation.
- Denial of service via malformed `.agentdb` files or crafted query inputs.
- Information disclosure (reading memory or file data outside the intended database scope).

Out of scope:

- Vulnerabilities in applications *built using* AgentDB (report those to the relevant project).
- Issues requiring physical access to the machine running AgentDB.
- Social engineering.

---

## Hardening Guidance

For production deployments:

- **Parameterize all inputs.** Never pass user-supplied strings directly to `db.execute()` or FFI `agentdb_execute()`. Use `db.execute_params()` in Rust and the equivalent parameterized forms in Python and Node.js.
- **Validate vector dimensions** client-side before upsert in addition to the API-level enforcement.
- **Restrict file permissions** on `.agentdb` files to the minimum required (e.g., `chmod 600`).
- **Run `cargo audit` periodically** to catch CVEs in transitive dependencies. AgentDB's CI does this automatically on every push to `main`.
- **WASM**: the WASM module crosses the JS/Wasm boundary using JSON strings and typed arrays only — no raw memory pointers are exposed.

---

## Dependency Security

AgentDB runs `cargo audit` in CI on every push to `main`. The dependency surface is kept minimal:

| Dependency | Purpose |
|---|---|
| `rusqlite` | SQLite bindings (bundled C, SQLite 3.x security track) |
| `serde` / `serde_json` | Serialization |
| `bincode` | HNSW index serialization |
| `thiserror` | Error type derivation |
| `uuid` | ID generation |
| `rand` | HNSW random-level generation |
| `clap` | CLI argument parsing |
