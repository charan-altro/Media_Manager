# MVP 1 & 2 Frontend Synchronization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align the React frontend with the backend's MVP 1 and MVP 2 features by fixing interface mismatches, normalizing data keys, and ensuring playback status refreshes after viewing.

**Architecture:** Surgical updates to the API adapter, global task handler, and detail modal to ensure data consistency and fresh UI state.

**Tech Stack:** React, TypeScript, HLS.js, Lucide-React.

---

### Task 1: Update API Interfaces

**Files:**
- Modify: `frontend/src/api/adapter.ts`

- [ ] **Step 1: Update `TaskUpdate` interface**
Add the reconciliation fields to the `TaskUpdate` interface to match the backend payload.

```typescript
// frontend/src/api/adapter.ts

export interface TaskUpdate {
  taskId: string;
  status: string;
  progress: number;
  total: number;
  message: string;
  startedAt?: number;
  finishedAt?: number;
  debugInfo?: string;
  filesNew?: number;      // Added
  filesHealed?: number;   // Added
  filesMissing?: number;  // Added
}
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/api/adapter.ts
git commit -m "feat(frontend): add reconciliation fields to TaskUpdate interface"
```

---

### Task 2: Data Normalization in App.tsx

**Files:**
- Modify: `frontend/src/App.tsx`

- [ ] **Step 1: Update `handleTaskUpdate` and `subscribeToTasks`**
Ensure that snake_case fields from the backend are mapped to camelCase fields in the frontend.

```typescript
// frontend/src/App.tsx

// Find handleTaskUpdate and ensure it handles the new fields
const handleTaskUpdate = (update: any) => {
  // Normalize fields if they come in as snake_case (standard Rust serde default)
  const normalizedUpdate: TaskUpdate = {
    ...update,
    taskId: update.taskId || update.task_id,
    startedAt: update.startedAt || update.started_at,
    finishedAt: update.finishedAt || update.finished_at,
    debugInfo: update.debugInfo || update.debug_info,
    filesNew: update.filesNew || update.files_new || 0,
    filesHealed: update.filesHealed || update.files_healed || 0,
    filesMissing: update.filesMissing || update.files_missing || 0,
  };

  setTasks(prev => {
    const oldStatus = prev[normalizedUpdate.taskId]?.status;
    // ... rest of logic using normalizedUpdate ...
    return { ...prev, [normalizedUpdate.taskId]: normalizedUpdate };
  });
};

// Also update subscribeToTasks to use handleTaskUpdate consistently
const subscribeToTasks = () => {
  api.getTasks().then(initialTasks => {
    initialTasks.forEach((t: any) => {
      handleTaskUpdate(t);
    });
  });
  // ...
};
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/App.tsx
git commit -m "fix(frontend): normalize snake_case task fields to camelCase"
```

---

### Task 3: Refresh Playback Status on Close

**Files:**
- Modify: `frontend/src/components/DetailModal.tsx`

- [ ] **Step 1: Update `VideoPlayer` `onClose` callback**
Add a call to `api.getPlaybackStatus` to ensure the modal reflects the most recent viewing progress.

```typescript
// frontend/src/components/DetailModal.tsx

{streamingUrl && (
  <VideoPlayer 
    url={streamingUrl} 
    mediaId={activeMediaId!}
    mediaType={activeMediaType!}
    initialPosition={resumePosition}
    onClose={async () => {
      setStreamingUrl(null);
      // Refresh playback status for this item
      const type = isShow ? 'tv' : 'movie';
      try {
        const status = await api.getPlaybackStatus(type, item.id);
        setPlaybackStatus(status);
      } catch (e) {
        console.error("Failed to refresh playback status", e);
      }
      loadData();
    }} 
  />
)}
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/components/DetailModal.tsx
git commit -m "feat(frontend): refresh playback status when video player closes"
```

---

### Final Verification

- [ ] **Step 1: Verify Task Badges**
Trigger a scan from the UI. Open the "Tasks" page. Verify that "New", "Healed", and "Missing" badges appear.

- [ ] **Step 2: Verify Resume Logic**
Play a movie, seek to middle, close player. Click "Play" again. Verify "Resume from..." dialog appears with correct time.
