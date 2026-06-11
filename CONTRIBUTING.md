# Contributing to AgentDB

Thank you for your interest in contributing. This document covers development setup, PR process, and code standards.

## Development Setup

### Prerequisites

- Rust 1.75.0 or later (MSRV; install via [rustup](https://rustup.rs))
- `cargo` — included with Rust

Optional toolchains for binding development:

| Binding | Requires |
|---|---|
| Python | Python 3.9+, `pip install maturin` |
| Node.js | Node.js ≥ 18, `npm install -g @napi-rs/cli` |
| C FFI header | `cargo install cbindgen` |
| WASM | `cargo install wasm-pack`, `rustup target add wasm32-unknown-unknown` |

### Clone and build

```bash
git clone https://github.com/hvrcharon1/agentdb.git
cd agentdb
cargo build
cargo test
```

### Running the examples

```bash
cargo run --example agent_memory
cargo run --example rag_pipeline
cargo run --example graph_traverse
cargo run --example v020_query_power
```

### Running lints

```bash
cargo fmt --all -- --check    # formatting check
cargo clippy --all-targets    # lints (default features)
```

### Running coverage

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage/
open coverage/tarpaulin-report.html
```

---

## Making Changes

### Branch naming

| Type | Prefix | Example |
|---|---|---|
| Feature | `feat/` | `feat/async-api` |
| Bug fix | `fix/` | `fix/hnsw-memory-leak` |
| Documentation | `docs/` | `docs/python-quickstart` |
| Refactoring | `refactor/` | `refactor/fts-internals` |
| CI / tooling | `ci/` | `ci/coverage-threshold` |

### Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(vectors): add euclidean distance metric
fix(graph): fix CTE recursion depth limit on SQLite 3.39
docs(python): add agent_memory.py quick-start example
ci: remove hardcoded ref:main from CI lint job
```

### Pull request process

1. **Fork** the repository and create your branch from `main`.
2. **Write tests** for any new behaviour; regression tests for any bug fix.
3. **Run CI checks locally** before pushing:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets
   cargo test --lib --tests
   ```
4. **Open a pull request** against `main` with a clear title and description.
5. Describe *what* changed and *why* in the PR body.
6. At least one maintainer review is required before merge.

---

## Code Standards

### Formatting

All Rust code must be formatted with `cargo fmt` using the settings in `rustfmt.toml`. The CI lint job enforces this on every push and PR.

### Clippy

All warnings from `cargo clippy --all-targets` must be addressed. Suppressing a lint with `#[allow(...)]` is permitted only with an explanatory comment.

### Documentation

Every public item (`pub struct`, `pub fn`, `pub enum`, `pub trait`) must have a `///` rustdoc comment with:

- A one-sentence description.
- Parameter and return value documentation for non-trivial functions.
- A `/// # Examples` block for primary API methods.

Run `cargo doc --open` locally to verify docs.rs output before pushing.

### Tests

- Unit tests live in a `#[cfg(test)] mod tests { … }` block in the same file.
- Integration tests live in `tests/`.
- Each new feature should add at least one integration test.
- Bug fixes should add a regression test that fails before the fix and passes after.

### Error handling

- Always return `crate::error::Result<T>` from functions that can fail.
- Never `unwrap()` or `expect()` in library code (only in tests or examples).
- Add a new `AgentDbError` variant for genuinely new failure conditions.

### SQL safety

- Never concatenate user-supplied strings into SQL. Use parameterized queries (`?1`, `?2`, …) via `rusqlite::params!`.
- All user-facing SQL inputs in the CLI (`agentdb sql`) and FFI (`agentdb_execute`) must go through parameterized execution.

---

## Security

To report a security vulnerability, please see [SECURITY.md](SECURITY.md) — **do not** open a public GitHub issue.

---

## License

By contributing to AgentDB you agree that your contributions will be released into the public domain under the same terms as the project. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
