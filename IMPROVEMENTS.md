# 🚀 Media Manager Codebase Improvements

This document outlines architectural and code quality improvements for both the Rust backend and the React frontend. Adopting these best practices will significantly improve maintainability, performance, and type safety as the project scales.

## 🦀 Backend (Rust) Best Practices

### 1. Structured Error Handling in Core Libraries (`thiserror` vs `anyhow`)
**Current State:** 
The `media_core` library relies almost exclusively on `anyhow::Result` and `anyhow!()` for error handling (70+ occurrences in `scraper`, `scanner`, `db`, etc.).
**Why Improve:** 
While `anyhow` is great for application binaries (`apps/desktop` or `apps/server`), libraries should expose structured, explicitly defined errors. Consumers of a library need to gracefully handle different failure states (e.g., `ScraperError::ApiRateLimit` vs `ScraperError::NetworkFailure`) without string-matching on `anyhow` output.
**Action:** 
Adopt `thiserror` in `media_core` to define specific Enums for errors:
```rust
#[derive(thiserror::Error, Debug)]
pub enum DatabaseError {
    #[error("Record not found")]
    NotFound,
    #[error("Database constraint violation: {0}")]
    ConstraintViolation(String),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}
```

### 2. Compile-Time SQL Verification (`sqlx::query!`)
**Current State:** 
Database operations in `media_core/src/db/queries.rs` are executed using `sqlx::query` and `sqlx::query_as`.
**Why Improve:** 
These macros are evaluated at runtime. One of Rust's greatest superpowers is compile-time safety. 
**Action:** 
Switch to `sqlx::query!` and `sqlx::query_as!`. This will require having the `.env` database URL available during compilation, allowing the compiler to check your SQL syntax against your actual database schema and guaranteeing type safety for the returned rows.

### 3. The Newtype Pattern for Identifiers
**Current State:** 
IDs for domain entities are passed as raw `i64` primitives across the whole application (e.g., `pub async fn upsert_movie(library_id: i64, ...)`, `pub id: i64`).
**Why Improve:** 
Relying on primitives makes it dangerously easy to swap parameters by mistake, such as passing a `movie_id` into a function that expects a `library_id` or `show_id`.
**Action:** 
Wrap primary keys in a "Newtype" struct. This prevents domain-logic bugs at compile time.
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct MovieId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct LibraryId(pub i64);
```

### 4. Zero-Copy and Borrowing
**Action:** Look for areas where strings are heavily cloned (such as metadata parsing or caching logic) and consider using `std::borrow::Cow<'a, str>`. This allows strings to be borrowed if they aren't mutated, saving on allocation overhead.

---

## ⚛️ Frontend (React/TypeScript) Best Practices

### 1. Global State Management & Prop Drilling
**Current State:** 
`App.tsx` holds almost all global state variables (`movies`, `tvShows`, `libraries`, `tasks`, `selectedLibrary`, `genreFilter`, `selectionMode`) and drills them down through multiple layers of components to pages like `MoviesPage` and `TvShowsPage`.
**Why Improve:** 
As the application grows, prop drilling becomes brittle, creates massive component signatures, and makes reusability impossible.
**Action:** 
Introduce a lightweight state management solution like **Zustand** or **React Context**.
Even better, use **TanStack React Query** (`@tanstack/react-query`) to handle fetching API data like `api.getMovies()`. It handles caching, loading states, background refreshing, and eliminates the need for massive `useEffect` fetching blocks inside `App.tsx`.

### 2. Excessive Re-rendering
**Current State:** 
In `App.tsx`, a `setInterval` is used to update the `currentTime` state every 1 second.
**Why Improve:** 
Because `currentTime` is at the very top level of the component tree, updating it forces the **entire application** (Navbar, Sidebar, MediaGrids, all lists) to re-render every single second.
**Action:** 
Extract the `currentTime` state and the specific UI that depends on it (presumably the `TasksPage` timestamp or a clock) into its own isolated, smaller component. This restricts the re-render cycle strictly to the component that needs it.

### 3. Separation of Concerns
**Action:** Move API data transformation (e.g., merging library data, grouping seasons) into isolated utility functions or custom hooks (`useLibrary()`, `useMovies()`) instead of managing it all sequentially in `loadData()` inside `App.tsx`.
