# DockTop

A lightweight, terminal-based Docker management tool written in Rust.

## Features (MVP 1)

- **List Containers**: View all running and stopped containers.
- **Live Status**: Real-time status indicators (🟢 Running, 🔴 Stopped).
- **Statistics**: View CPU and Memory usage for the selected container.
- **Keyboard Navigation**: Use `Up`/`Down` to navigate, `q` to quit.

## Tech Stack

- **Language**: Rust
- **UI**: Ratatui
- **Backend**: Tokio, Reqwest (Unix Socket)

## How to Run

```bash
cargo run
```

## Development Phases

- [x] Phase 0: Foundation (Docker Socket Connection)
- [x] Phase 1: MVP 1 (List & Stats)
- [x] Phase 2: MVP 2 (Inspector & Logs)
- [x] Phase 3: MVP 3 (Control: Start/Stop/Restart)
- [ ] Phase 4: Polish (Graphs & Themes)
