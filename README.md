# Media Manager

> A high-performance, self-hosted media management application built in Rust using the Tauri framework.

## Overview

Media Manager is the next-generation evolution of the SelfHost Media Orchestrator Python project. It is re-architected from the ground up in Rust to deliver:

- **Ultimate Performance**: Sub-second scanning of 10,000+ file libraries via WalkDir + Rayon parallelism
- **Tiny Footprint**: ~5MB binary vs. 300MB+ Python + Node runtimes
- **Dual Deployment**: Runs as a native Windows `.exe` (Tauri) **or** a Docker container (Axum REST API)
- **Memory Safety**: Rust's ownership model eliminates entire classes of bugs that affected the Python version

## Project Structure

```
Media_Manager/                  ← Cargo Workspace Root
├── Cargo.toml                  ← Workspace manifest
├── core/                       ← Shared Rust library (no HTTP/IPC)
│   ├── src/
│   │   ├── db/                 ← SQLx SQLite schema & queries
│   │   ├── scanner/            ← WalkDir file discovery engine
│   │   ├── parser/             ← Regex filename parser
│   │   ├── scraper/            ← TMDB / OMDb async clients
│   │   ├── nfo/                ← NFO XML reader/writer
│   │   ├── renamer/            ← Template-based file renamer
│   │   ├── models/             ← Shared data structs (Serde)
│   │   └── task_manager/       ← Background task state
├── apps/
│   ├── server/                 ← Axum REST API → Docker container
│   │   └── src/
│   │       ├── routes/         ← HTTP endpoint handlers
│   │       └── sse/            ← Server-Sent Events for progress
│   └── desktop/                ← Tauri desktop wrapper → .exe
│       ├── src/                ← Tauri commands (IPC bridge)
│       └── tauri.conf.json
└── frontend/                   ← React + Vite + TypeScript UI
    ├── src/
    │   ├── api/                ← Adapter pattern (Tauri IPC | fetch)
    │   ├── components/
    │   └── pages/
    └── vite.config.ts
```

## Quick Start

### Desktop App (Windows)
```bash
cd apps/desktop
cargo tauri dev
```

### Docker Server
```bash
docker compose up --build
```

## Documentation

- [System Design](DOCS/ARCHITECTURE/system_design.md)
- [Application Architecture](DOCS/ARCHITECTURE/application_architecture.md)
- [Migration Plan](DOCS/MIGRATION/python_to_rust_migration.md)
- [MVP Scope](DOCS/PRODUCT/mvp.md)
- [Implementation Plan](DOCS/PLAN/implementation_plan.md)
