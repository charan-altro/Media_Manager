# HLS Fix & Local Playback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve HLS streaming timeouts by improving backend path normalization and increasing the timeout, and add UI buttons to trigger local playback via default system players.

**Architecture:** Update `StreamManager` in Rust for better process spawning and add new UI components in React for local playback control.

**Tech Stack:** Rust, Axum, React, Tailwind CSS.

---

### Task 1: Backend HLS Timeout & Path Fix

**Files:**
- Modify: `media_core/src/scanner/streaming.rs`

- [ ] **Step 1: Implement path normalization and increase timeout**
Update `StreamManager::start_hls` to normalize the input path and bump the wait time.

```rust
// media_core/src/scanner/streaming.rs

// 1. In start_hls, before Command::new:
let normalized_input = crate::paths::normalize_slashes(input_path.to_str().unwrap());

// 2. In Command::new:
// change: -i, input_path.to_str().unwrap()
// to:     -i, &normalized_input

// 3. In wait_for_file timeout:
// change: from_secs(15)
// to:     from_secs(30)
```

- [ ] **Step 2: Commit**

```bash
git add media_core/src/scanner/streaming.rs
git commit -m "fix(backend): normalize HLS input paths and increase playlist timeout"
```

---

### Task 2: Frontend UI - Local Playback Button (Movies)

**Files:**
- Modify: `frontend/src/components/DetailModal.tsx`

- [ ] **Step 1: Update Movie Playback Buttons**
Rename the main playback button and add the "Play Locally" option.

```tsx
// frontend/src/components/DetailModal.tsx

// Find the movie playback button section:
{!isShow && (
  <div className="flex flex-col gap-3">
    <button 
      onClick={() => handlePlayMedia(item.id, 'movie')}
      className="w-full bg-red-600 hover:bg-red-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition active:scale-95 flex items-center justify-center gap-2 shadow-lg shadow-red-900/20"
    >
      <Play className="w-4 h-4 fill-current" /> Stream (Browser)
    </button>
    <button 
      onClick={() => {
        api.playMovie(item.id);
        toast.success("Opening in local player...");
      }}
      className="w-full bg-zinc-800 hover:bg-zinc-700 py-4 rounded-xl font-black uppercase tracking-widest text-xs transition border border-zinc-700 flex items-center justify-center gap-2"
    >
      <Monitor className="w-4 h-4" /> Play Locally (VLC)
    </button>
  </div>
)}
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/components/DetailModal.tsx
git commit -m "feat(frontend): add Local Playback button for movies"
```

---

### Task 3: Frontend UI - Local Playback Icon (TV Shows)

**Files:**
- Modify: `frontend/src/components/DetailModal.tsx`

- [ ] **Step 1: Add Local Play Icon to Episode Rows**
Add a new icon button next to the Download button for episodes.

```tsx
// frontend/src/components/DetailModal.tsx

// Inside the episodes loop:
<div className="flex items-center gap-2">
  <button 
    onClick={(e) => { 
      e.stopPropagation(); 
      api.playEpisode(ep.id); 
      toast.success("Opening in local player...");
    }} 
    className="p-2 hover:bg-zinc-800 rounded-lg text-zinc-600 hover:text-white transition"
    title="Play Locally"
  >
    <Monitor className="w-4 h-4" />
  </button>
  <button 
    onClick={(e) => { e.stopPropagation(); onDownload(ep.id, 'tv'); }} 
    className="p-2 hover:bg-zinc-800 rounded-lg text-zinc-600 hover:text-white transition"
  >
    {/* existing download svg */}
  </button>
  {/* ... existing Play icon ... */}
</div>
```

- [ ] **Step 2: Commit**

```bash
git add frontend/src/components/DetailModal.tsx
git commit -m "feat(frontend): add Local Playback icon for TV episodes"
```

---

### Final Verification

- [ ] **Step 1: Verify Local Play (Movie)**
Open a movie modal. Click "Play Locally (VLC)". Verify VLC or default player opens the file.

- [ ] **Step 2: Verify Local Play (Episode)**
Open a TV show modal. Click the Monitor icon on an episode. Verify local player opens.

- [ ] **Step 3: Verify HLS Fix**
Open a movie modal. Click "Stream (Browser)". Verify the stream starts within 30 seconds without timeout.
