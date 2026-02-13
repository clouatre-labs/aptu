# SPDX-License-Identifier: Apache-2.0

FROM rust:1.93.1-alpine@sha256:4fec02de605563c297c78a31064c8335bc004fa2b0bf406b1b99441da64e2d2d AS chef
RUN apk add --no-cache musl-dev && cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p aptu-mcp
COPY . .
RUN cargo build --release -p aptu-mcp

FROM gcr.io/distroless/static-debian12:nonroot

LABEL org.opencontainers.image.title="aptu-mcp" \
      org.opencontainers.image.description="MCP server for AI-powered GitHub triage and review" \
      org.opencontainers.image.source="https://github.com/clouatre-labs/aptu" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.vendor="Clouatre Labs" \
      io.modelcontextprotocol.server.name="clouatre-labs/aptu-mcp"

COPY --from=builder /app/target/release/aptu-mcp /aptu-mcp

EXPOSE 8080
ENTRYPOINT ["/aptu-mcp"]
CMD ["--transport", "http", "--host", "0.0.0.0", "--port", "8080"]
