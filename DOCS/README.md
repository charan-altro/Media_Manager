# Media Manager – Documentation Index

This directory contains the consolidated design, planning, and migration documentation for the Media Manager ecosystem.

---

## 📘 Primary Reference
- **[MASTER PROJECT BOOK](MASTER_PROJECT_BOOK.md)** — **Start here.** The comprehensive, consolidated reference for the entire project, architecture, and current status.

---

## 📑 Detailed Documentation
- **[ARCHITECTURE](ARCHITECTURE.md)** — System design, component breakdown, and technical logic.
- **[PRODUCT](PRODUCT.md)** — MVP scope, feature matrix, and product vision.
- **[MIGRATION](MIGRATION.md)** — Python → Rust mapping, performance actuals, and migration report.
- **[PLAN](PLAN.md)** — Phased implementation roadmap and final completion status.

---

## 🚀 Technical Highlights
- **Stack**: Rust (Axum/Tauri) · React · SQLite (SQLx)
- **Performance**: ~4.2s library scans, 42MB Docker image, 8MB Windows binary.
- **Architecture**: Unified Rust Core with dynamic API Adapter for Desktop/Server deployment.
