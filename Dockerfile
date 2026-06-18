# syntax=docker/dockerfile:1
# ─────────────────────────────────────────────────────────────────────────────
# Stage 1 — builder
# ─────────────────────────────────────────────────────────────────────────────
FROM rust:1.75-slim AS builder

# Build dependencies needed by rusqlite's bundled SQLite
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        gcc \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependency compilation separately from application code.
# Copy manifests first so this layer is re-used when only source changes.
COPY Cargo.toml Cargo.lock ./

# Stub out lib + bin so Cargo can resolve the dependency graph without the
# full source tree.
RUN mkdir -p src/bin && \
    echo 'fn main() {}' > src/bin/agentdb.rs && \
    echo '' > src/lib.rs

RUN cargo build --release --bin agentdb 2>/dev/null; \
    # Remove the stubs so the real source replaces them
    rm -f target/release/deps/agentdb* \
          target/release/deps/datacules_agentdb*

# Now bring in the real source and build.
COPY src ./src

RUN cargo build --release --bin agentdb

# ─────────────────────────────────────────────────────────────────────────────
# Stage 2 — runtime
# ─────────────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# OCI image labels
LABEL org.opencontainers.image.title="AgentDB" \
      org.opencontainers.image.description="Single-file embedded database for AI agents" \
      org.opencontainers.image.source="https://github.com/hvrcharon1/agentdb" \
      org.opencontainers.image.licenses="Unlicense" \
      org.opencontainers.image.vendor="Datacules LLC"

# libgcc / libm are pulled in by the bundled SQLite build on some platforms;
# ca-certificates lets HTTPS work if the binary ever needs it.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Persist database files outside the container
WORKDIR /data
VOLUME /data

COPY --from=builder /build/target/release/agentdb /usr/local/bin/agentdb

ENTRYPOINT ["/usr/local/bin/agentdb"]
