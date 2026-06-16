# Contributing to AgentDB

Thank you for taking the time to contribute! Every bug report, feature suggestion,
documentation fix, and pull request helps make AgentDB better for the whole
AI-agent ecosystem.

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Ways to Contribute](#ways-to-contribute)
3. [Development Setup](#development-setup)
4. [Making Changes](#making-changes)
5. [Testing](#testing)
6. [Commit Style](#commit-style)
7. [Pull Request Process](#pull-request-process)

---

## Code of Conduct

Be kind, constructive, and welcoming. Harassment, personal attacks, and
discriminatory language are not tolerated in issues, pull requests, or any
other project space.

---

## Ways to Contribute

| Type | How |
|------|-----|
| 🐛 Bug reports | Open an issue using the **Bug Report** template |
| 💡 Feature ideas | Open an issue using the **Feature Request** template |
| 📖 Documentation | Fix typos, clarify examples, or expand API docs |
| 🧪 Tests | Add coverage for edge cases or untested code paths |
| ⚡ Performance | Improvements to the HNSW index, graph traversal, or FTS layer |
| 🔌 Language SDKs | Fixes and extensions for the Python or Node.js bindings |

---

## Development Setup

### Prerequisites

| Tool | Minimum version | Purpose |
|------|----------------|---------|
| Rust (stable) | 1.76 | Core library and FFI bindings |
| Python | 3.9 | Python SDK (`maturin` / PyO3) |
| Node.js | 18 | Node.js binding (`napi-rs`) |
| `maturin` | latest | Build and develop the Python wheel |
| `cargo-criterion` | optional | Run benchmarks with HTML reports |

### Rust core

```bash
git clone https://github.com/hvrcharon1/agentdb.git
cd agentdb

# Build and run all tests
cargo test --all-features

# Run benchmarks
cargo bench
```

### Python binding

```bash
cd python
pip install maturin
maturin develop          # installs an editable wheel into your venv
python -c "import agentdb; print(agentdb.__version__)"
```

### Node.js binding

```bash
cd nodejs
npm install
npm run build
node test/smoke.js
```

---

## Making Changes

1. **Fork** the repository, then create a focused branch:
   ```bash
   git checkout -b feat/my-feature    # new functionality
   git checkout -b fix/the-bug        # bug fix
   git checkout -b docs/improve-readme
   ```
2. Keep each PR to **one logical concern** — it is much easier to review and revert.
3. Add or update tests for every behaviour change.
4. Update the relevant documentation: rustdoc, `README.md`, and `CHANGELOG.md`
   (under `[Unreleased]`).

---

## Testing

```bash
# Rust unit + integration tests
cargo test --all-features

# Rust benchmarks (opt-in)
cargo bench

# Python — build the wheel then run examples
cd python && maturin develop
python python/examples/agent_memory.py

# Node.js smoke test
cd nodejs
npm run build
node test/smoke.js
```

All CI checks must be green before a PR can be merged.

---

## Commit Style

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short summary in imperative mood>

[optional body]

[optional footer: Closes #<issue>]
```

**Types:** `feat` · `fix` · `docs` · `test` · `bench` · `refactor` · `ci` · `chore`

**Scope examples:** `vectors` · `memory` · `fts` · `hybrid` · `python` · `nodejs` · `ffi`

```
feat(vectors): add dot-product distance metric
fix(memory): guard against cycles in recursive CTE traversal
docs(python): add agent_memory example
bench(vectors): add 1536-d embedding throughput case
```

---

## Pull Request Process

1. Fill in the **Pull Request template** completely.
2. Link the related issue with `Closes #<number>` in the PR body.
3. Ensure all CI jobs are green before requesting review.
4. At least **one maintainer approval** is required to merge.
5. **Squash merge** is preferred to keep the history linear.
6. After merging, delete your feature branch.

Thank you for contributing to AgentDB! 🚀
