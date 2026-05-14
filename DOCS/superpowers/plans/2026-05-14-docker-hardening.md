# Docker Security Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remediate critical vulnerabilities by transitioning to a Wolfi-based, distroless-ready Docker image with custom FFmpeg supporting RPi hardware acceleration.

**Architecture:** Multi-stage build (Rust/Node/FFmpeg builders) targeting a stripped `wolfi-base` runtime. Switches `reqwest` to `rustls` to eliminate `libssl` runtime dependency.

**Tech Stack:** Rust, Wolfi OS, FFmpeg (v4l2m2m), Distroless principles.

---

### Task 1: Update Cargo Dependencies to Rustls

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Modify `Cargo.toml` to use `rustls-tls`**

Update the `reqwest` dependency in the `[workspace.dependencies]` section to disable default features (which include `native-tls`/OpenSSL) and enable `rustls-tls`.

```toml
reqwest = { version = "0.11", default-features = false, features = ["json", "rustls-tls"] }
```

- [ ] **Step 2: Verify `Cargo.toml` syntax**

Run: `cargo check` (in a dev environment) or simply verify the TOML structure.
Expected: No syntax errors.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "security: switch reqwest to rustls-tls to remove libssl dependency"
```

---

### Task 2: Implement Multi-Stage Dockerfile with Custom FFmpeg

**Files:**
- Modify: `Dockerfile`

- [ ] **Step 1: Replace `Dockerfile` with the new Multi-Stage Hardened version**

This version includes the FFmpeg builder stage with `v4l2m2m` support and the hardened runtime stage.

```dockerfile
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
```

- [ ] **Step 2: Commit**

```bash
git add Dockerfile
git commit -m "security: transition to Wolfi-based hardened multi-stage Dockerfile with custom FFmpeg"
```

---

### Task 3: Validation (Dry-Run / Manual Review)

- [ ] **Step 1: Verify the build process (Mental or Local Build)**

Since building the entire image (especially compiling FFmpeg) is time-intensive, verify that all stages correctly point to their respective sources and destinations.

- [ ] **Step 2: Confirm Entrypoint**

Note that the `CMD` was changed to `ENTRYPOINT` and uses an absolute path `["/app/server"]` because `/bin/sh` is removed in the hardening step.

- [ ] **Step 3: Check for remaining Shell usage**

Review the Dockerfile to ensure no `RUN` commands appear *after* the `apk del apk-tools` step.
Expected: None.
