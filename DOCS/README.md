# 📘 Media Manager – Documentation Index

This directory contains the organized design, planning, architectural, and historical documentation for the Media Manager ecosystem.

---

## 📂 Core Documentation
- **[MASTER PROJECT BOOK](MASTER_PROJECT_BOOK.md)** — Start here. The comprehensive, consolidated reference for the entire project, architecture, and current status.
- **[ARCHITECTURE](ARCHITECTURE.md)** — System design, Cargo workspace layout, concurrency model, and technical diagrams.
- **[PRODUCT](PRODUCT.md)** — Product vision, MVP scope, and comparative feature matrix.
- **[MIGRATION](MIGRATION.md)** — Python-to-Rust migration strategy, module mapping, and performance optimization wins.
- **[PLAN](PLAN.md)** — Phased development roadmap milestones and completion checklists.

---

## 🛠️ Technical Deep-Dives
- **[Stash Feature Parity Mapping](technical/stash_feature_map.md)** — Direct comparison of features between Stash (Go) and Media Manager (Rust).
- **[Streaming Analysis](technical/stash_streaming_analysis.md)** — Reverse-engineering study on Stash's video streaming pipeline.
- **[Streaming Implementation Plan](technical/streaming_implementation_plan.md)** — Step-by-step roadmap for smart remuxing and fragmented MP4 delivery.
- **[Stash Feature Roadmap](technical/stash_feature_implementation_plan.md)** — Implementation tasks checklist for fingerprinting, scanning, and visual previews.
- **[DevOps & Infrastructure Plan](technical/devops_infrastructure_plan.md)** — Build automation pipelines for Docker (headless server) and Tauri (Windows MSI).

---

## 🗄️ Archive & Legacy Proposals
- **[Milestone 1-2 Completion Design](archive/2026-05-12-mvp1-2-completion-design.md)** — MVP completion logs for progress indicators and HLS playback.
- **[Feature Audit Report](archive/feature_audit_report.md)** — Historical feature comparison auditing implementation gaps.
- **[Codebase Improvements Review](archive/IMPLEMENTATION_IMPROVEMENT_PLAN.md)** — Static analysis feedback on Rust and React code patterns.
- **[Windows IPC Bug Fixes Log](archive/BUG_FIX.md)** — Log of Tauri parameter alignment and environments detection fixes.
- **[Rust Core Refactoring Suggestions](archive/IMPROVEMENTS.md)** — Best practices proposal for errors and compile-time SQL verification.
- **[Stash-Parity TODOs](archive/TODO.md)** — Finished database/probing task lists.
- **[DEFERRED: Video.js v10 Migration Guide](archive/videojs_v10_migration_guide.md)** — Archived proposal for upgrading the frontend player library.
- **[DEFERRED: Video.js v10 Implementation Steps](archive/videojs_v10_migration_guide-implementation_plan.md)** — Detailed phase steps checklist for player replacement.

---

## 🚀 Technical Highlights
- **Stack**: Rust (Axum/Tauri) · React · SQLite (SQLx)
- **Performance**: ~4.2s library scans, 42MB Docker image, 8MB Windows binary.
- **Architecture**: Unified Rust Core with dynamic API Adapter for Desktop/Server deployment.
