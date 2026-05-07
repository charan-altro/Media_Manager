# 🐛 Media Manager Bug Fix Report

## 🔍 Overview of Issues Found & Fixed

During a comprehensive review of the Media Manager project, particularly focusing on the Windows EXE (Desktop) build and core functionalities (like adding and scanning libraries), several critical integration bugs between the frontend and the Rust desktop backend were identified.

Although the core Rust logic and React frontend successfully compile without errors, the IPC (Inter-Process Communication) layer between them was fundamentally broken due to parameter naming mismatches.

### 1. Tauri Command Parameter Mismatch (The "Silent Failure" Bug)
**Issue:**
Tauri commands defined in Rust expect parameters in `snake_case` by default unless configured otherwise. The frontend API adapter (`frontend/src/api/adapter.ts`) was sending payloads with `camelCase` keys or completely omitting expected keys. When running the EXE build, this caused the Tauri invoke commands to silently fail, leaving users unable to add libraries, scan them, or fetch media.

**Specific Broken Commands:**
- `create_library`: Frontend sent `{ name, path, mediaType }`, Rust expected `media_type`.
- `delete_library`: Frontend sent `{ method: 'DELETE' }` but omitted the required `id`.
- `get_movies` & `get_tv_shows`: Frontend sent `libraryId`, Rust expected `library_id`.
- `get_seasons` & `get_episodes`: Frontend sent `showId` / `seasonId`, Rust expected `show_id` / `season_id`.
- `start_scan`: Frontend sent `libraryId`, Rust expected `library_id`.
- `refresh_metadata`: Frontend omitted the required `id` parameter.

**Fix Applied:**
Updated `frontend/src/api/adapter.ts` to strictly map all frontend variables to the exact `snake_case` parameter names expected by the Rust backend in `apps/desktop/src/main.rs`.

### 2. Tauri Environment Detection Bug
**Issue:**
The desktop app checks if it's running inside Tauri using `(window as any).__TAURI_INTERNALS__ !== undefined`. In modern Tauri 2.0 builds, relying solely on `__TAURI_INTERNALS__` can sometimes be brittle depending on the exact build configuration, potentially causing the frontend to incorrectly assume it's running in a web browser and attempt to `fetch` from `http://localhost:7878/api` instead of using native IPC.

**Fix Applied:**
Broadened the `IS_TAURI` detection in `adapter.ts` to check multiple known Tauri markers:
`export const IS_TAURI = (window as any).__TAURI_INTERNALS__ !== undefined || (window as any).__TAURI__ !== undefined || (window as any).__TAURI_IPC__ !== undefined;`

### 3. Outdated Compilation Errors
**Issue:**
The `errors.txt` file at the root of the project listed several Rust compilation errors (e.g., `error[E0609]: no field year on type &tv::TVShow` in `exporter/mod.rs`).

**Status:**
Verified via `cargo check` in both `media_core` and `apps/desktop`. These errors have already been resolved in the source code. The project compiles perfectly. The `errors.txt` is simply a leftover artifact from previous development phases.

## 🛠️ Next Steps & Recommendations

1. **Test the Windows Build:** 
   Run `.\build-windows.ps1` again. The produced EXE should now perfectly handle library creation and scanning.
2. **Implement Missing Desktop Commands:** 
   The frontend API refers to endpoints like `cleanup_duplicates`, `cleanup_empty_folders`, and `rename_movie` which are not currently registered in the Desktop's `tauri::generate_handler!` array in `apps/desktop/src/main.rs`. These need to be implemented or mapped if they are strictly server-only commands.
3. **Database Schema Constraints:** 
   The index `idx_movies_unique_identity` on the `movies` table uses `IFNULL(year, 0)`. While functional, expression indexes in SQLite can occasionally cause compatibility issues with certain ORM layers if not carefully managed. Monitor this if bulk scraping behaves unexpectedly.
