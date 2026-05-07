# Media Manager – Documentation Index

This directory contains the full design and planning documentation for the Rust/Tauri migration.

## Structure

| Directory | Contents |
|---|---|
| `ARCHITECTURE/` | System design, component breakdown, data flow diagrams |
| `PLAN/` | Implementation plan, phased task lists, tech stack reference |
| `PRODUCT/` | MVP scope, performance targets, roadmap |
| `MIGRATION/` | Python → Rust module mapping, API surface, risk register |

## Reading Order (First-Time Setup)

1. **[System Design](ARCHITECTURE/system_design.md)** — Start here. Understand the monorepo structure and dual-deployment architecture.
2. **[MVP Scope](PRODUCT/mvp.md)** — Understand what's in and out of scope.
3. **[Implementation Plan](PLAN/implementation_plan.md)** — The full phased build plan with code stubs.
4. **[Migration Guide](MIGRATION/python_to_rust_migration.md)** — Line-by-line module mapping from Python to Rust.
5. **[Application Architecture](ARCHITECTURE/application_architecture.md)** — Deep dive into each Rust module's internals.
