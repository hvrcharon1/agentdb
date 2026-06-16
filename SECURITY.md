# Security Policy

## Supported Versions

| Version | Status |
|---------|--------|
| 0.3.x | ✅ Actively maintained |
| 0.2.x | ⚠️ Critical fixes only |
| < 0.2 | ❌ No longer supported |

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report security issues privately through GitHub's built-in
[Security Advisories](https://github.com/hvrcharon1/agentdb/security/advisories/new)
feature. This allows maintainers to assess impact, prepare a patch, and coordinate
disclosure before the details become public.

### What to include

- A clear description of the vulnerability and its potential impact
- Steps to reproduce or a minimal proof-of-concept
- The AgentDB version(s) and language binding(s) affected
- Any suggested mitigations or fixes

### Response timeline

| Milestone | Target |
|-----------|--------|
| Acknowledgement | Within 48 hours |
| Severity assessment | Within 5 business days |
| Patch / coordinated disclosure | Agreed with reporter |

## Scope

AgentDB is an embedded, single-file database library. The threat model covers:

- **SQL injection** — unsanitised inputs to `execute` / `execute_params` or
  equivalent bindings
- **Path traversal** — crafted paths in the `open()` call allowing arbitrary file
  reads or writes
- **Denial of service** — malformed vectors, deeply nested graph structures, or
  crafted FTS queries causing unbounded memory or CPU usage
- **Supply-chain risks** — malicious transitive dependencies in the Cargo, PyPI,
  or npm dependency trees
- **FFI memory safety** — issues in the C FFI or WASM layer that allow memory
  corruption from the host process

**Out of scope:** Vulnerabilities in SQLite itself should be reported to the
[SQLite security team](https://www.sqlite.org/security.html).

## Disclosure Policy

We follow a **90-day coordinated disclosure window**. After a patch is released
we will publish a GitHub Security Advisory and credit the reporter (unless they
prefer to remain anonymous).
