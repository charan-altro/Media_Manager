# Design: MVP 1 & 2 Frontend Synchronization

**Date**: 2026-05-13
**Status**: Approved
**Topic**: Aligning frontend UI with completed backend MVP 1 & 2 features (Surgical Integration)

---

## 1. Goal
Ensure that the completed backend functionality for MVP 1 (reconciliation counters) and MVP 2 (streaming heartbeat and resume persistence) is correctly displayed and functional in the React frontend.

## 2. Identified Gaps
1.  **Interface Mismatch**: The `TaskUpdate` interface in `frontend/src/api/adapter.ts` does not include `filesNew`, `filesHealed`, and `filesMissing`, causing the UI to ignore these values when rendering badges.
2.  **Case Mapping (Rust to TS)**: The backend sends task payloads with snake_case keys (e.g., `files_new`), while the frontend expects camelCase keys (`filesNew`). `App.tsx` needs to map these values when handling Server-Sent Events (SSE).
3.  **State Synchronization**: When a user closes the video player, `DetailModal.tsx` does not refetch the latest `playbackStatus`. This causes the "Resume Playback" dialog to miss recent progress until the page is fully refreshed.

## 3. Implementation Approach (Surgical Integration)

### 3.1. API Interface Alignment (`frontend/src/api/adapter.ts`)
Update the `TaskUpdate` interface to match the backend payload capabilities.

```typescript
export interface TaskUpdate {
  taskId: string;
  status: string;
  progress: number;
  total: number;
  message: string;
  startedAt?: number;
  finishedAt?: number;
  debugInfo?: string;
  filesNew?: number;
  filesHealed?: number;
  filesMissing?: number;
}
```

### 3.2. Case Mapping in Global State (`frontend/src/App.tsx`)
In `App.tsx`, the SSE stream pushes JSON strings. Ensure the `handleTaskUpdate` or the parsing logic correctly maps `files_new` to `filesNew`, etc.

*   In the `subscribeToTasks` `eventSource.onmessage` handler, intercept the parsed JSON.
*   Map `task.files_new` to `task.filesNew`, `task.files_healed` to `task.filesHealed`, and `task.files_missing` to `task.filesMissing`.
*   Ensure the initial fetch (`api.getTasks().then(...)`) also maps these fields correctly.

### 3.3. DetailModal Playback State Sync (`frontend/src/components/DetailModal.tsx`)
Update the `VideoPlayer` `onClose` callback inside `DetailModal.tsx`.

*   Currently, when `VideoPlayer` triggers `onClose`, `DetailModal` clears `streamingUrl` and calls `loadData()`.
*   Add a call to `api.getPlaybackStatus` to update the local `playbackStatus` state so the progress bar in the modal instantly reflects the progress made during the viewing session.

## 4. Testing Strategy
*   **Tasks UI**: Trigger a library scan and verify that the "New", "Healed", and "Missing" badges appear on the `TasksPage` with non-zero values.
*   **Resume Dialog**: Play a video for > 5 seconds, close it, and immediately click "Play" again. The Resume dialog should appear offering to start from the new time.

## 5. Out of Scope
*   Moving the entire global state to a React Context (deferred to a future refactor).
