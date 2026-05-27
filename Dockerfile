# Stage 1: Build Rust binary on Wolfi Rust SDK (aligns glibc with runtime)
FROM cgr.dev/chainguard/rust:latest-dev AS builder
USER root
WORKDIR /app
RUN apk update && apk add --no-cache openssl-dev pkgconf build-base

# Copy workspace configuration
COPY Cargo.toml Cargo.lock ./
COPY media_core/Cargo.toml media_core/Cargo.toml
COPY apps/server/Cargo.toml apps/server/Cargo.toml
COPY apps/desktop/Cargo.toml apps/desktop/Cargo.toml
COPY apps/cli/Cargo.toml apps/cli/Cargo.toml

# Create dummy source files for dependency pre-fetching
RUN mkdir -p media_core/src apps/server/src apps/desktop/src apps/cli/src && \
    echo "fn main() {}" > apps/server/src/main.rs && \
    echo "fn main() {}" > apps/desktop/src/main.rs && \
    echo "fn main() {}" > apps/cli/src/main.rs && \
    touch media_core/src/lib.rs

# Pre-fetch dependencies
RUN cargo build --release -p server

# Copy actual source code
COPY . .

# Final build
RUN cargo build --release -p server

# Stage 2: Build React frontend
FROM node:20-slim AS frontend-builder
WORKDIR /app
COPY frontend/package*.json ./frontend/
RUN cd frontend && npm ci
COPY . .
WORKDIR /app/frontend
RUN npm run build

# Stage 3: Hardened Runtime (Wolfi-based for zero CVE posture)
FROM cgr.dev/chainguard/wolfi-base

# Install runtime dependencies (shadow for user management, pre-compiled ffmpeg)
RUN apk update && apk add --no-cache shadow ffmpeg

# Create non-root user
RUN groupadd -r mediavault && useradd -r -g mediavault mediavault

# Create application directories and set ownership
RUN mkdir -p /app/data /app/transcodes /app/backups && \
    chown -R mediavault:mediavault /app

WORKDIR /app

# Copy binaries and assets
COPY --from=builder /app/target/release/server /app/server
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist

# Ensure binaries are executable and owned by mediavault
RUN chown -R mediavault:mediavault /app

# Optional Hardening: Remove package manager and shell for production.
# Uncomment the following line to convert to a true distroless container after verifying startup:
# RUN apk del apk-tools shadow && rm -rf /bin/sh /lib/apk /var/cache/apk /etc/apk

# Declare volumes for persistence
VOLUME ["/app/data", "/app/transcodes", "/app/backups"]

# Set environment variables
ENV DATABASE_URL=sqlite:/app/data/mediavault.db
ENV RUST_LOG=info
ENV FFMPEG_PATH=/usr/bin/ffmpeg
ENV FFPROBE_PATH=/usr/bin/ffprobe

# Switch to non-root user
USER mediavault

# Expose the server port
EXPOSE 7878

# Use absolute path as there is no shell
ENTRYPOINT ["/app/server"]
