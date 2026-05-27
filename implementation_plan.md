# Fix Local Player Playback and Metadata Scanning via Dynamic Path Resolution

The local player does not open video files and library scanning fails because the database contains paths starting with `D:/MEDIA/...`, but the physical drive `D:` is not present on the host. Instead, it is configured in `.env` as `DRIVE_D_PATH=//192.168.1.9/FastMedia`.

We will implement dynamic path resolution in the backend, mapping virtual drive letters (e.g., `D:`) to their configured paths on the fly. This will resolve failures in the file watcher (watchdog), library scanner, and local media launcher.

## User Review Required

> [!IMPORTANT]
> - **Drive Mapping Resolution**: We translate virtual drive prefixes (like `D:/` or `D:\`) to mapped network/host paths (like `//192.168.1.9/FastMedia`) on the fly when interacting with the file system.
> - **Database Portability**: Slashes and paths in the SQLite database remain in their original form, ensuring database compatibility and clean portable records.

## Open Questions

None.

## Proposed Changes

### Core Library Path Utilities

#### [MODIFY] [paths.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/paths.rs)

- Add a helper `resolve_path` to check if a path starts with a drive letter, look up the corresponding `DRIVE_<LETTER>_PATH` environment variable, and replace it dynamically.
- Update `make_relative` to perform root path resolution before generating the relative path.
- Update `make_absolute` to resolve the root library path prior to joining.

---

### Watchdog & Library Scanner Services

#### [MODIFY] [watchdog.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/scanner/watchdog.rs)

- Resolve the library path using `paths::resolve_path` before passing it to `watcher.watch()`.
- During file change detection, compare event paths against the resolved library path.

#### [MODIFY] [service.rs](file:///c:/Users/chara/New_Projects_antigravity/Media_Manager-dev/Media_Manager/media_core/src/scanner/service.rs)

- Resolve the library path using `paths::resolve_path` in `scan_library` before running `Path::exists()` check and calling `WalkDir::new()`.

---

## Verification Plan

### Automated Tests
- Run `cargo test --test db_dump_test` to confirm compilation.
- Run `cargo check --workspace` to verify workspace status.

### Manual Verification
- Start the server using `.\dev.ps1 web`.
- Click "Play Locally (VLC)" in the UI and confirm that the player starts.
- Click "Refresh Info" or run a library scan and verify that metadata downloads correctly.
