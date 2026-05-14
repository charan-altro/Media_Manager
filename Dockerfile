# Stage 1: Build Rust binary
FROM debian:13-slim AS builder
WORKDIR /app
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    curl pkg-config libssl-dev build-essential ca-certificates && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

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

# Final build (will be fast if only source changed)
RUN cargo build --release -p server

# Stage 2: Build React frontend
FROM node:20-slim AS frontend-builder
WORKDIR /app
COPY frontend/package*.json ./frontend/
RUN cd frontend && npm ci
COPY . .
WORKDIR /app/frontend
RUN npm run build

# Stage 3: Build Custom FFmpeg with RPi hardware acceleration
FROM cgr.dev/chainguard/wolfi-base AS ffmpeg-builder
RUN apk update && apk add build-base git nasm yasm libv4l-dev
WORKDIR /ffmpeg
RUN git clone --depth 1 https://github.com/FFmpeg/FFmpeg.git .
RUN ./configure \
    --prefix=/usr/local \
    --enable-gpl \
    --enable-nonfree \
    --enable-libv4l2 \
    --enable-v4l2-m2m \
    --disable-debug \
    --disable-doc \
    --enable-optimizations && \
    make -j$(nproc) && \
    make install

# Stage 4: Minimal Hardened Runtime
FROM cgr.dev/chainguard/wolfi-base
# Install only essential runtime libs for FFmpeg (v4l)
RUN apk update && apk add libv4l shadow

# Create non-root user
RUN groupadd -r mediavault && useradd -r -g mediavault mediavault

# Create application directories and set ownership
RUN mkdir -p /app/data /app/transcodes /app/backups && \
    chown -R mediavault:mediavault /app

WORKDIR /app

# Copy binaries and assets
COPY --from=builder /app/target/release/server /app/server
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist
COPY --from=ffmpeg-builder /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=ffmpeg-builder /usr/local/bin/ffprobe /usr/local/bin/ffprobe

# Ensure binaries are executable and owned by mediavault
RUN chown -R mediavault:mediavault /app

# Distroless Conversion: Remove package manager and shell
RUN apk del apk-tools shadow && \
    rm -rf /bin/sh /lib/apk /var/cache/apk /etc/apk

# Declare volumes for persistence
VOLUME ["/app/data", "/app/transcodes", "/app/backups"]

# Set environment variables
ENV DATABASE_URL=sqlite:/app/data/mediavault.db
ENV RUST_LOG=info

# Switch to non-root user
USER mediavault

# Expose the server port
EXPOSE 7878

# Use absolute path as there is no shell
ENTRYPOINT ["/app/server"]
