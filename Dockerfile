# Stage 1: Build Rust binary
FROM debian:13-slim AS builder
WORKDIR /app
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    curl pkg-config libssl-dev build-essential ca-certificates && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"
COPY . .
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
RUN apk update && apk add libv4l

# Create application directories
RUN mkdir -p /app/data /app/transcodes /app/backups

WORKDIR /app

# Copy binaries and assets
COPY --from=builder /app/target/release/server /app/server
COPY --from=frontend-builder /app/frontend/dist /app/frontend/dist
COPY --from=ffmpeg-builder /usr/local/bin/ffmpeg /usr/local/bin/ffmpeg
COPY --from=ffmpeg-builder /usr/local/bin/ffprobe /usr/local/bin/ffprobe

# Distroless Conversion: Remove package manager and shell
RUN apk del apk-tools && \
    rm -rf /bin/sh /lib/apk /var/cache/apk /etc/apk

# Set environment variables
ENV DATABASE_URL=sqlite:/app/data/mediavault.db
ENV RUST_LOG=info

# Expose the server port
EXPOSE 7878

# Use absolute path as there is no shell
ENTRYPOINT ["/app/server"]
