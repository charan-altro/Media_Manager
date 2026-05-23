# DevOps & Infrastructure Execution Plan: Media Manager

## 1. Executive Summary
The goal is to provide a hybrid deployment model for the SelfHost Media Orchestrator (Media Manager) built in Rust and React. It must support two primary distribution channels:
1. **Native Windows Executable (.exe/.msi)** - Providing high performance and native system integration via Tauri.
2. **Containerized Docker Image** - Providing cross-platform compatibility, deployable with a single `docker run` command.

This plan details the CI/CD pipeline implementation, codebase adjustments, and infrastructure required to achieve full automation.

---

## 2. Docker "One-Command" Architecture

To achieve the "One Simple Docker Command" objective without requiring `docker-compose` or an external Nginx container, the Rust backend (Axum) must serve the compiled React Single Page Application (SPA) natively.

### 2.1 Backend Adjustments (apps/server)
Currently, `apps/server/src/main.rs` only serves `/api/*` routes. It needs to be updated to serve the frontend static files.
- **Action**: Add `tower_http::services::ServeDir` to the Axum router fallback.
- **Code Change**:
  ```rust
  use tower_http::services::ServeDir;
  
  // Inside main() router setup:
  let app = Router::new()
      .route("/api/...", ...)
      // ... existing routes ...
      .fallback_service(ServeDir::new("/app/frontend/dist").append_index_html_on_directories(true))
      .with_state(app_state);
  ```
This allows the server running on port `7878` to handle both the REST API and the React frontend.

### 2.2 Dockerfile Optimization
The existing `Dockerfile` is correctly structured as a multi-stage build. 
- **Stage 1**: Compiles the Rust backend (`cargo build --release -p server`).
- **Stage 2**: Compiles the React frontend (`npm run build`).
- **Stage 3**: Minimal Debian runtime containing only the compiled `server` binary and the `frontend/dist` directory.

### 2.3 The Target "Simple Command"
Once published to a registry (like GitHub Container Registry), the user can deploy the entire stack with:
```bash
docker run -d \
  --name media-manager \
  -p 7878:7878 \
  -v ./data:/app/data \
  -v /path/to/your/media:/media:ro \
  -e DATABASE_URL="sqlite:/app/data/mediavault.db?mode=rwc" \
  -e TMDB_API_KEY="your_api_key_here" \
  --restart unless-stopped \
  ghcr.io/yourusername/media-manager:latest
```

---

## 3. Native Windows Application (Tauri)

The `apps/desktop` crate is configured for Tauri. The goal is to build an installer that bundles the frontend and backend logic into a single native `.exe` or `.msi`.

- **Build Target**: `x86_64-pc-windows-msvc`
- **Dependencies**: Windows SDK, Node.js, and Rust toolchain.
- **Output**: An MSI installer and a standalone executable.
- **Database**: SQLite database will be initialized in the user's `AppData/Roaming/MediaManager` directory.

---

## 4. CI/CD Pipeline Design (GitHub Actions)

We will implement two distinct GitHub Actions workflows in `.github/workflows/`.

### Pipeline 1: Docker Build & Publish (`docker-publish.yml`)
**Triggers:** Push to `main` branch or tag creation (`v*`).
**Steps:**
1. Checkout code.
2. Log in to GitHub Container Registry (GHCR) using `secrets.GITHUB_TOKEN`.
3. Extract metadata (tags, labels) for Docker.
4. Build and push multi-stage Docker image.
   - *Optimization*: Use Docker buildx cache to speed up Rust compilation times.

### Pipeline 2: Windows Native Release (`windows-release.yml`)
**Triggers:** Tag creation (`v*`) or manual workflow dispatch.
**Steps:**
1. Run on `windows-latest`.
2. Install Node.js (v20) and Rust (stable).
3. Build the React frontend (`npm ci` and `npm run build`).
4. Install Tauri CLI (`cargo install tauri-cli`).
5. Run `cargo tauri build` inside `apps/desktop`.
6. Upload the generated `.msi` and `.exe` artifacts to the GitHub Release.

---

## 5. Execution Steps for Implementation

1. **Modify Axum Server (`apps/server/src/main.rs`)**:
   Implement the `ServeDir` fallback so the backend serves the frontend SPA. Ensure 404s on frontend routes fallback to `index.html` (client-side routing).
2. **Create GitHub Actions Workflows**:
   - Write `.github/workflows/docker-publish.yml`
   - Write `.github/workflows/windows-release.yml`
3. **Configure Tauri (`tauri.conf.json`)**:
   Ensure the `build.distDir` points to `../../frontend/dist` and the `build.beforeBuildCommand` runs the npm build.
4. **Environment Variables Strategy**:
   For Docker, rely on `-e` flags or an `.env` file mounted. For Tauri, rely on a settings UI that stores keys in the SQLite database or system keyring.
5. **Testing**:
   Run the local Docker build and test the "one simple command" locally before pushing.
