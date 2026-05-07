# Media Orchestrator: Rust Migration Review

This document outlines the pending tasks, bugs, and improvements required to achieve full MVP parity and to complete the remaining post-MVP phases based on the project documentation.

## 1. Frontend Discrepancies & Improvements

The React frontend currently features a visually appealing "Netflix-style" layout with Tailwind CSS and Lucide icons. However, it lacks several structural and functional elements compared to a complete application.

### Missing MVP Features
| Feature | Status | Description |
| :--- | :--- | :--- |
| **Movie Detail/Hero View** | 🔴 Missing | Clicking a movie poster does not open a detail view (no backdrop, plot, cast, or detailed ratings). It currently only displays a grid. |
| **TV Shows UI** | 🔴 Missing | The "TV Shows" navigation button exists, but there is no logic to render TV shows, seasons, or episodes. |
| **Library Management** | 🟡 Partial | Users can add libraries, but the ability to **Edit** or **Remove** existing library paths is absent. |
| **Routing** | 🔴 Missing | The application lacks a router (e.g., `react-router-dom`). All UI is crammed into a single `App.tsx` file, making navigation buttons non-functional. |

### Frontend Bugs
> [!WARNING]
> **Runtime Crash on "MATCH" Button:** In `App.tsx` (line 153), the "MATCH" button `onClick` handler calls `api.request('bulk_scrape', ...)` which will throw an error because `request` is not exported as part of the `api` object in `adapter.ts`.

> [!WARNING]
> **Hardcoded Localhost:** `App.tsx` hardcodes `http://localhost:7878` for the SSE `EventSource` and local artwork images. This will break the Tauri desktop build, which does not run a local Axum server on port 7878 and should rely on Tauri's IPC or custom `asset://` protocols.

### Architectural Improvements
- **Componentization:** Break down `App.tsx` into smaller components (e.g., `Sidebar`, `MovieGrid`, `MovieCard`, `TaskToast`).
- **State Management:** Introduce proper state management for modals and views rather than relying solely on local component state.
- **Tauri IPC Consistency:** Ensure all backend calls, including exports (`exportCsv`, `exportHtml`), use Tauri IPC commands when running in desktop mode instead of `window.open` which relies on the HTTP API.

---

## 2. Pending Post-MVP Features (Phases 6-8)

Based on the `mvp.md` and `implementation_plan.md`, the following features are pending to complete the migration roadmap:

### Phase 6: Management Tools
- [ ] **Media Info Extraction (`ffprobe`)**: Implement `core/src/scanner/mediainfo.rs` using an `ffprobe` subprocess to extract exact video resolution and codecs.

### Phase 7: Subtitle & Export
- [ ] **Subtitles Downloader**: Port the OpenSubtitles hash algorithm to Rust (`core/src/subtitles/mod.rs`) for automatic subtitle fetching.
- [ ] **Data Export**: Ensure JSON/CSV/HTML export endpoints generate correctly formatted files from the SQLite DB.

### Phase 8: Playback & Monitoring
- [ ] **Hybrid Video Playback**: Implement the ability to stream media via HTTP (for Docker) or launch natively via the OS default player (for Tauri Desktop).
- [ ] **Real-time Directory Monitoring**: Integrate the `notify` crate to detect and process file system changes instantly without requiring a full library scan.
- [ ] **Webhook Receivers**: Build endpoints to receive push updates from external tools like Tdarr, Radarr, and Sonarr.
