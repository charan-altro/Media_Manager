# Vidstack Migration Implementation Plan

## Objective
Migrate the Media Manager frontend video player from Video.js v8 to Vidstack Player (@vidstack/react) v10. Replace monolithic legacy plugins with native React components, implement a custom React-based A-B loop, and expose backend `hash` data to support native VTT thumbnails scrubbing.

## Key Files & Context
- `frontend/src/components/VidstackPlayer.tsx` (New file to replace `VideoPlayer.tsx`)
- `frontend/src/hooks/useVidstackAbLoop.ts` (New file for loop logic)
- `frontend/src/components/AbLoopControls.tsx` (New file for loop UI)
- `frontend/src/api/adapter.ts` (API updates)
- `media_core/src/models/movie.rs` (Backend model updates)
- `media_core/src/db/movie_repo.rs` (Backend SQL updates)

## Implementation Steps

### Phase 1: Backend Updates (VTT Support)
1. **Update Models:** Add `hash: Option<String>` to the `Movie` struct in `media_core/src/models/movie.rs`.
2. **Update SQL Queries:** Modify `find_all`, `find_by_id`, and `find_by_ids` in `media_core/src/db/movie_repo.rs` to select `mf.hash` from the `movie_files` table.
3. **Frontend API:** Update the `Movie` interface in `frontend/src/api/adapter.ts` to include `hash?: string`.

### Phase 2: Frontend A-B Loop Logic
1. **Create Hook:** Create `frontend/src/hooks/useVidstackAbLoop.ts`.
2. **Implementation:** Use `@vidstack/react` hooks (`useMediaState`, `useMediaRemote`) inside a `useEffect` that watches `currentTime`. When `currentTime >= loopEnd`, trigger `remote.seek(loopStart)`.

### Phase 3: Vidstack Player Component
1. **Create `VidstackPlayer.tsx`:** Build the new player using `<MediaPlayer>` and `<DefaultVideoLayout>`.
2. **Source Strategy:** Use a `useEffect` to fetch both Direct Play and HLS URLs using `api.startStreaming`. Pass these as an array to `MediaPlayer`'s `src` prop.
3. **Thumbnail Support:** Construct the thumbnail URL using the `hash` (e.g., `${API_BASE}/assets/${hash}/vtt`) and pass it to `<DefaultVideoLayout thumbnails={vttUrl} />`.
4. **Hotkeys:** Implement custom hotkey listeners for 'a', 'b', and 'l' to interact with the A-B loop hook.

### Phase 4: Integration
1. **Replace Video.js:** Update `frontend/src/pages/MoviesPage.tsx` and `TvShowsPage.tsx` to mount `VidstackPlayer` instead of `VideoPlayer`.
2. **Cleanup:** Delete `VideoPlayer.tsx`, `VttThumbnails.tsx`, and `useAbLoop.ts`.

## Verification & Testing
1. **Compilation:** Verify the Rust backend compiles successfully.
2. **Playback:** Verify videos load and play automatically.
3. **A-B Loop:** Set points A and B, ensure the video jumps back to A seamlessly when reaching B. Verify hotkeys work.
4. **Thumbnails:** Hover over the progress bar and verify thumbnails render correctly.