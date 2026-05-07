# Stage 1: Build Rust binary
FROM debian:13-slim AS builder
WORKDIR /app

# Install build dependencies
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y --no-install-recommends \
    curl pkg-config libssl-dev build-essential ca-certificates && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

ENV PATH="/root/.cargo/bin:${PATH}"

COPY . .

# Build the server crate
RUN cargo build --release -p server

# Stage 2: Build React frontend
FROM node:20-slim AS frontend-builder
WORKDIR /app
COPY frontend/package*.json ./frontend/
RUN cd frontend && npm ci
COPY . .
WORKDIR /app/frontend
RUN npm run build

# Stage 3: Minimal runtime image
FROM debian:13-slim

# Install runtime dependencies: 
# - libssl3: Required by Rust binary
# - ca-certificates: For HTTPS requests
# - ffmpeg: For HLS transcoding and metadata extraction
RUN apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y --no-install-recommends \
    libssl3 ca-certificates ffmpeg && \
    rm -rf /var/lib/apt/lists/*

# Create application directories
RUN mkdir -p /app/data /app/transcodes /app/backups

WORKDIR /app

# Copy binaries and assets
COPY --from=builder /app/target/release/server /app/server
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist

# Set environment variables
ENV DATABASE_URL=sqlite:/app/data/mediavault.db
ENV RUST_LOG=info

# Expose the server port
EXPOSE 7878

CMD ["/app/server"]

