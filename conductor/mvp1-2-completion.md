# MVP 1 & 2 Completion Plan

## Objective
Finalize the outstanding tasks for MVP 1 and MVP 2, specifically addressing the synchronization and streaming stability mechanisms outlined in the design document.

## Scope & Impact
This plan covers the completion of 6 specific tasks to bring the media manager to full MVP status.
- **Backend Model & Scanner:** Update the `TaskUpdate` struct with fields `files_new`, `files_healed`, and `files_missing`. Modify the scanner logic in `media_core/src/scanner/worker.rs` to track these counters and execute a missing file pass against the database.
- **Frontend UI:** Add visual badges to `TasksPage.tsx` to display the reconciliation counters and update `App.tsx` interfaces.
- **Streaming & Persistence:** Enhance `VideoPlayer.tsx` to ping the heartbeat endpoint periodically to keep async transcodes alive. Integrate a prompt dialog in `DetailModal.tsx` that queries the playback status endpoint and offers users the ability to resume where they left off or start from the beginning.

## Implementation Steps
1. **Task 1 & 2 & 3:** Implement and verify backend reconciliation in `media_core`.
2. **Task 4:** Modify `TasksPage.tsx` and `App.tsx`.
3. **Task 5 & 6:** Refactor `VideoPlayer.tsx` and `DetailModal.tsx`.
4. **Final Verification:** Fix any lingering compiler and linting errors to ensure a clean build and accurate UI behavior.

## Verification
- Run `cargo check -p media_core` and `cargo check -p server`.
- Run `npm run lint` in the `frontend` directory.