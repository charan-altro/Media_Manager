# Docker Security Hardening Design

## Objective
Remediate critical vulnerabilities (CVE-2026-34875, CVE-2026-33846) present in the `latest` Docker image by transitioning to a security-first base image. The new design focuses on achieving a "Zero CVE" status and adhering to distroless principles while maintaining support for Raspberry Pi hardware acceleration.

## Scope & Impact
The `Dockerfile` and `Cargo.toml` will be updated. This will change how the application is built and executed inside the container but will not change application functionality.

## Proposed Solution: The "Hardened Pragmatist" Multi-Stage Build

### 1. Dependency Updates
*   **Rust `reqwest`**: Switch from the default OpenSSL native-tls to `rustls-tls` by modifying `Cargo.toml`. This removes the need for `libssl` and `ca-certificates` (at the OS level) in the final runtime image, significantly reducing the attack surface.

### 2. Multi-Stage Dockerfile Architecture
*   **Stage 1: Rust Builder (`debian:13-slim`)**
    *   Continues to compile the `server` crate.
*   **Stage 2: Frontend Builder (`node:20-slim`)**
    *   Continues to compile the React application.
*   **Stage 3: FFmpeg Builder (`cgr.dev/chainguard/wolfi-base`)**
    *   Install build dependencies (`build-base`, etc.).
    *   Download FFmpeg source code.
    *   Configure with `--enable-v4l2-m2m` to ensure Raspberry Pi hardware acceleration is supported.
    *   Compile FFmpeg.
*   **Stage 4: Runtime (`cgr.dev/chainguard/wolfi-base`)**
    *   Install any essential runtime libraries for FFmpeg (e.g., `libv4l`).
    *   Copy the compiled Rust binary from Stage 1.
    *   Copy the built React assets from Stage 2.
    *   Copy the custom-compiled FFmpeg from Stage 3.
    *   **Distroless Conversion**: Explicitly remove the package manager and shell (`apk del apk-tools && rm -rf /bin/sh /lib/apk /var/cache/apk`) to harden the image.

## Alternatives Considered
*   `cgr.dev/chainguard/ffmpeg:latest` as runtime: Rejected because it lacks compiled-in support for Raspberry Pi hardware acceleration (`v4l2m2m`).
*   `debian:12-slim` (Bookworm): Rejected due to slower security patch cadences and remaining CVEs.

## Migration & Rollback
If the custom FFmpeg build fails on edge cases, we can temporarily revert the Dockerfile back to `debian:13-slim` until the Wolfi compile environment is fully adjusted.