# Performance & Safety Improvement Plan

This document outlines the strategic steps to optimize the Media Manager project for speed, memory efficiency, and rock-solid reliability.

## Phase 1: High-Impact Configuration & Quick Wins
*   [x] **Release Profile Tuning:** Update root `Cargo.toml` with LTO, codegen-units, and panic settings.
*   [x] **Zero-Cost Iterators:** Replace allocating patterns (like `.to_lowercase()`) in loops with allocation-free comparisons.
*   [x] **Safety Audit:** Replace `.unwrap()` calls in critical paths with proper error handling or `.expect()`.

## Phase 2: Database Query Optimization
*   [x] **Atomic Upserts:** Refactor `db/queries.rs` to use the `RETURNING` clause for SQLite 3.35+, reducing DB roundtrips by 50% during scans.
*   [x] **Transaction Batching:** Refactored `db::queries` to accept generic `Executor` traits, enabling atomic batching where appropriate while balancing concurrent readers via SQLite WAL mode.

## Phase 3: Memory & Type Efficiency
*   [x] **Metadata Enums:** Replace `String` fields for `Resolution`, `Codec`, and `MediaStatus` with type-safe, memory-efficient Enums.
*   [x] **String Interning / Cow:** Refactored `Resolution` to avoid string allocations. `MediaStatus` now enforced.

## Phase 4: Architectural Refinement
*   [x] **Trait-Based Scrapers:** Abstracted API clients behind a `MediaScraper` trait, starting with TMDB, to enable mock testing and provider flexibility.
*   [x] **Streaming I/O:** Use `ServeFile` for serving artwork to reduce peak memory usage under load.

## Phase 5: Concurrency Tuning
*   [x] **Global Task Semaphore:** Implemented a centralized `heavy_task_semaphore` in the TaskManager to gracefully manage concurrent resource-intensive tasks (FFmpeg/Scraping) across all requests.
