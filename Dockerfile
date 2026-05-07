# Stage 1: Build Rust binary
FROM debian:13-slim AS builder
WORKDIR /app
# Install build dependencies and Rust toolchain
RUN apt-get update && apt-get upgrade -y && \
    apt-get install -y curl pkg-config libssl-dev build-essential && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
COPY . .
# We build the server crate specifically
RUN cargo build --release -p server

# Stage 2: Build React frontend
FROM node:20-alpine AS frontend-builder
WORKDIR /app
COPY frontend/package*.json ./frontend/
RUN cd frontend && npm ci
COPY frontend/ ./frontend/
RUN cd frontend && npm run build

# Stage 3: Minimal runtime image
FROM debian:13-slim
# Upgrade base system to patch CVEs, then install runtime dependencies
RUN apt-get update && apt-get upgrade -y && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /app/data
WORKDIR /app
COPY --from=builder /app/target/release/server /app/server
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist
# Set environment variables
ENV DATABASE_URL=sqlite:/app/data/mediavault.db
EXPOSE 7878
CMD ["/app/server"]
