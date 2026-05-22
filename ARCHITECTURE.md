# Sldl Remote — Architecture Document

## Overview

Sldl Remote is a cross-platform Tauri v2 application that acts as a remote controller for the [sldl](https://github.com/fiso64/sldl) Soulseek batch downloader. It runs on macOS, Linux, and Windows, communicates with a remote Linux host (e.g., a PVE container) via SSH, and provides a GUI for queuing downloads, monitoring progress, and managing a remote music library.

---

## Table of Contents

1. [Architecture Diagram](#architecture-diagram)
2. [Tech Stack](#tech-stack)
3. [Data Flow](#data-flow)
4. [Rust Backend](#rust-backend)
5. [Frontend](#frontend)
6. [Security Model](#security-model)
7. [Rate Limiting & Soulseek Ban Handling](#rate-limiting--soulseek-ban-handling)
8. [Associated Album Mode](#associated-album-mode)
9. [Error Handling Strategy](#error-handling-strategy)
10. [State Management](#state-management)
11. [Testing Strategy](#testing-strategy)
12. [Build & Deploy](#build--deploy)
13. [Future Work](#future-work)

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     User's Machine                          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Tauri App (Rust + WebView)                         │   │
│  │  ┌──────────────┐  ┌─────────────────────────────┐ │   │
│  │  │ Frontend     │  │ Backend (Rust)              │ │   │
│  │  │ ┌──────────┐ │  │ ┌─────────────────────────┐ │ │   │
│  │  │ │ Vanilla  │◄┼──┼─┤ Commands (IPC handlers) │ │ │   │
│  │  │ │ JS + CSS │ │  │ └─────────────────────────┘ │ │   │
│  │  │ └──────────┘ │  │ ┌─────────────────────────┐ │ │   │
│  │  │              │  │ │ Services                │ │ │   │
│  │  │ ┌──────────┐ │  │ │ ├─ ConfigManager        │ │ │   │
│  │  │ │ api.js   │ │  │ │ ├─ JobManager           │ │ │   │
│  │  │ │ app.js   │──┼──┼─┤ ├─ SSH executor         │ │ │   │
│  │  │ └──────────┘ │  │ │ └─ Crypto (AES-256-GCM) │ │ │   │
│  │  └──────────────┘  │ └─────────────────────────┘ │ │   │
│  │                    │ ┌─────────────────────────┐ │ │   │
│  │  ~/.config/        │ │ Models                  │ │ │   │
│  │  sldl-remote/      │ │ ├─ AppConfig            │ │ │   │
│  │  ├── config.enc    │ │ ├─ Job                  │ │ │   │
│  │  └── .key          │ │ └─ Notification         │ │ │   │
│  └────────────────────┘ └─────────────────────────┘ │ │   │
│                                                     │ │   │
└─────────────────────────────────────────────────────┘ │   │
                                                        │   │
                            SSH (port 22)              │   │
                        ┌──────────────────────────────┘   │
                        │                                   │
                        ▼                                   │
┌─────────────────────────────────────────────────────────┐ │
│              Remote Host (PVE Container)                │ │
│  ┌───────────────────────────────────────────────────┐ │ │
│  │  Linux x86_64  @ 192.168.70.12                    │ │ │
│  │                                                   │ │ │
│  │  /usr/local/bin/sldl  ←── sldl v2.6.0            │ │ │
│  │  /root/sldl.conf     ←── per-job configs          │ │ │
│  │  /tmp/sldl-remote-*.log ←── job logs              │ │ │
│  │                                                   │ │ │
│  │  /media/music/                                    │ │ │
│  │  ├── playlists/                                   │ │ │
│  │  │   └── <spotify-id>/                           │ │ │
│  │  │       └── Artist/Album/01. Title.mp3          │ │ │
│  │  ├── albums/                                      │ │ │
│  │  ├── artists/                                     │ │ │
│  │  └── tracks/                                      │ │ │
│  │                                                   │ │ │
│  │  Spotify API  ←── playlist/album/artist lookup   │ │ │
│  │       ↓                                           │ │ │
│  │  Soulseek.net  ←── P2P search & download         │ │ │
│  └───────────────────────────────────────────────────┘ │ │
└─────────────────────────────────────────────────────────┘ │
```

---

## Tech Stack

### Frontend
| Layer | Technology | Rationale |
|-------|-----------|-----------|
| Framework | Tauri v2 | Lightweight native app, not Electron |
| Build tool | Vite | Fast HMR, modern ESM |
| UI | Vanilla JS + CSS | No framework overhead, full control |
| Styling | CSS custom properties | Dark theme, easy to customize |
| Icons | Inline SVG / emoji | No icon font dependency |

### Backend (Rust)
| Crate | Purpose |
|-------|---------|
| `tauri` v2 | App framework, IPC, windowing |
| `tokio` | Async runtime, process spawning |
| `serde` + `serde_json` | Serialization |
| `aes-gcm` | Config file encryption |
| `rand` | Key/nonce generation |
| `directories` | Cross-platform config dirs |
| `uuid` | Job ID generation |
| `chrono` | Timestamps |
| `anyhow` | Error propagation |
| `shell-escape` | Safe SSH command construction |

---

## Data Flow

### 1. Starting a Download

```
User pastes URL → Frontend validates → Tauri command `start_job`
    → Load config from encrypted file
    → Detect Spotify type from URL
    → Allocate listen port (random in 49900-49999)
    → Build output subdir path
    → Write per-job sldl.conf to remote via SSH
    → Spawn sldl via SSH nohup
    → Capture PID
    → Store Job in in-memory registry
    → Return job_id to frontend
```

### 2. Polling Progress

```
Frontend timer (3s) → Tauri command `refresh_job_status`
    → For each active job:
        → SSH tail -60 <log_file>
        → Parse log for progress metrics
        → Update Job in registry
        → Detect ban patterns
    → Return updated jobs to frontend
    → Frontend re-renders job list + logs
```

### 3. Associated Album Mode

```
User enables "Associated Album Mode" on a playlist
    → Normal playlist job starts
    → Frontend shows "Discovering Albums..."
    → Backend runs sldl --print tracks-full
    → Parses track listing for unique (artist, album) pairs
    → Shows picker: "47 unique albums found"
    → User selects albums
    → Backend queues individual sldl -a jobs
    → Links them as children of parent playlist job
```

---

## Rust Backend

### Module Structure

```
src-tauri/src/
├── lib.rs              # Entry point, Tauri builder, command registration
├── main.rs             # Binary entry (calls lib::run())
├── error.rs            # AppError enum, AppResult, to_tauri helper
├── models/
│   ├── mod.rs          # Re-exports, shared types (Notification, Paginated, etc.)
│   ├── config.rs       # AppConfig, QualityPreset, ConfigProfile
│   └── job.rs          # Job, JobType, JobStatus, StartJobRequest, etc.
├── services/
│   ├── mod.rs          # Re-exports, helpers (extract_spotify_id, parse_progress, detect_ban)
│   ├── crypto.rs       # AES-256-GCM encrypt/decrypt for config file
│   ├── ssh.rs          # SSH command execution, streaming, file I/O
│   ├── config_manager.rs # Load/save AppConfig with caching
│   └── job_manager.rs  # In-memory HashMap<String, Job> registry
└── commands/
    ├── mod.rs          # Re-exports, CommandResponse wrapper, notification helpers
    ├── config.rs       # get_config, save_config, test_connection, health_check, etc.
    ├── jobs.rs         # start_job, list_jobs, refresh_job_status, stop_job, etc.
    ├── library.rs      # list_downloads, get_download_stats, search_library, etc.
    └── album_mode.rs   # discover_albums_from_playlist, start_associated_album_jobs
```

### Key Design Decisions

1. **SSH over SSH library**: We spawn the `ssh` binary instead of using an async SSH crate (e.g., `russh`) to avoid vendoring OpenSSL/libcrypto. Tradeoff: requires `ssh` CLI installed.

2. **In-memory jobs**: Jobs are ephemeral. App restart clears the registry. The source of truth is the remote sldl process + log files. This simplifies state management but means we lose history on restart.

3. **Per-job config files**: Each job gets its own `/tmp/sldl-remote-<uuid>.conf` on the remote host. This isolates jobs and avoids config conflicts.

4. **Encrypted config**: AES-256-GCM with a random key stored alongside the encrypted file. Not password-protected — protects against casual snooping, not determined attackers.

---

## Frontend

### Module Structure

```
src/
├── main.js             # Entry point: imports app.js, calls init() on DOMContentLoaded
├── app.js              # Main app controller: state, rendering, event handlers
├── services/
│   └── api.js          # Thin wrapper around all Tauri invoke() calls
└── styles.css          # Complete dark theme
```

### State Management

The frontend uses a single global `state` object in `app.js`:

```js
const state = {
  config: null,           // Current AppConfig
  jobs: [],               // Array of Job objects
  selectedJobId: null,    // Currently selected job for log view
  connectionOk: false,
  connectionMessage: '',
  pollInterval: null,     // setInterval handle
  autoRefresh: true,
  // ... album mode state
}
```

No reactive framework. Components re-render explicitly when state changes.

### Rendering Strategy

- `renderApp()`: Builds the full DOM structure once on init.
- `renderJobs()`: Re-renders only the job list panel when jobs update.
- `renderLogs()`: Updates the log viewer text.
- `renderAlbumDiscovery()`: Shows/hides the album discovery overlay.

All HTML is built via template literals. No virtual DOM.

---

## Security Model

### Threat Model

| Threat | Mitigation |
|--------|-----------|
| Config file snooping | AES-256-GCM encryption |
| Key file exposure | Unix 0600 permissions |
| SSH key exposure | Stored only on user's machine, never transmitted |
| Command injection | `shell-escape` crate for all remote commands |
| Man-in-the-middle | SSH StrictHostKeyChecking, TLS for GitHub downloads |
| Soulseek credential exposure | Stored encrypted; visible to remote sldl process only |

### What is NOT Protected

- The encryption key is stored on disk next to the encrypted file.
- A process running as the user can read both files.
- Memory dumps could expose decrypted config.
- The remote host has full access to Soulseek/Spotify credentials.

---

## Rate Limiting & Soulseek Ban Handling

### What We Learned

- Soulseek server bans IPs for ~30 minutes after ~34 searches per 220 seconds.
- Running two 800+ track playlists concurrently triggers an extended ban (45+ min).
- The ban is **IP-based**, not account-based. Changing IP (VPN, router restart) bypasses it.

### Mitigations in the App

1. **Conservative defaults**: `searches-per-time = 20`, `searches-renew-time = 300`, `fast-search = false`
2. **Config profiles**: "Conservative", "Balanced", "Aggressive", "Lossless Only"
3. **Ban detection**: Parses sldl stderr for ban messages, marks job as `Banned` status
4. **Auto-retry daemon**: Not implemented in GUI yet, but the groundwork exists

### Future Improvements

- Implement an exponential backoff retry queue.
- Detect ban before it happens by tracking search rate locally.
- Support proxy/VPN configuration in settings.

---

## Associated Album Mode

### Feature Name
**Associated Album Mode** — excavates and downloads the full album for every track in a playlist.

### Flow

1. **Discovery Phase**: Run `sldl <playlist> --print tracks-full` to get the full track listing.
2. **Extraction**: Parse output for unique `(artist, album)` pairs.
3. **Selection UI**: Show picker with checkboxes. User can deselect albums they don't want.
4. **Queue Phase**: Create individual `sldl -a` jobs for each selected album.
5. **Tracking**: Album jobs are linked as children of the parent playlist job. Aggregate progress is computed across the family.

### Current State
- UI toggle exists.
- Discovery command scaffolded but uses placeholder parsing.
- **TODO**: Replace with real Spotify Web API call to get accurate album metadata.

---

## Error Handling Strategy

### Rust Side

All fallible operations return `AppResult<T>` (`Result<T, AppError>`).

`AppError` variants:
- `Crypto` — encryption/decryption failure
- `Config` — file I/O or parse failure
- `Ssh { host, cmd, cause }` — SSH remote command failure
- `JobNotFound` — invalid job ID
- `InvalidInput` — bad URL, unknown type
- `SoulseekBanned { ip, message }` — rate limit / ban
- `Internal` — catch-all for unexpected errors

Tauri commands convert to `Result<T, String>` via `to_tauri()`.

### Frontend Side

- Alerts for blocking errors (failed to start job, connection lost).
- Inline error states for non-blocking errors (log fetch failed).
- No global error boundary — errors are handled at the call site.

---

## State Management

### Rust

Tauri manages two pieces of state via `.manage()`:

1. **ConfigManager** (Mutex): Loads/saves encrypted config. Cached in memory.
2. **JobManager** (Mutex): In-memory HashMap of jobs. No persistence.

### Frontend

Single global `state` object. No reactive framework. Explicit re-renders.

---

## Testing Strategy

### Rust Unit Tests

| Module | Tests |
|--------|-------|
| `crypto` | Round-trip encrypt/decrypt, fresh nonce per write |
| `ssh` | SSH command building (no exec without real host) |
| `job_manager` | Insert/get/list/filter, status transitions |
| `models` | Default construction, serialization round-trip |

Run: `cd src-tauri && cargo test`

### Integration Tests

Not yet implemented. Would require:
- A mock SSH server (or Docker container).
- A mock sldl binary that produces known output.
- End-to-end tests via Tauri's WebDriver support.

### Manual Testing Checklist

- [ ] Start playlist download
- [ ] Start album download
- [ ] Start artist aggregate download
- [ ] Associated Album Mode discovery + queue
- [ ] Stop running job
- [ ] Restart failed job
- [ ] Live log viewer updates
- [ ] Connection health check
- [ ] Config save/load/encrypt
- [ ] Export/import config
- [ ] Dark theme renders correctly
- [ ] Responsive layout on small window

---

## Build & Deploy

### Prerequisites

- Rust (via rustup)
- Node.js >= 18
- SSH client (macOS/Linux built-in, Windows: OpenSSH or WSL)
- `cargo install tauri-cli`

### Development

```bash
npm install          # Install frontend deps
cargo tauri dev      # Run with hot reload
```

### Release Build

```bash
cargo tauri build    # Build .app (macOS), .deb/.AppImage (Linux), .msi (Windows)
```

### Cross-Platform

Tauri supports cross-compilation. See [Tauri docs](https://tauri.app/v1/guides/building/cross-platform/) for details.

---

## Future Work

### Near-term
1. **Real Spotify API integration** for Associated Album Mode album discovery.
2. **Job persistence** — SQLite or JSON file for job history across restarts.
3. **Notifications** — macOS native notifications on job completion.
4. **Drag & drop** — drop Spotify URLs from browser onto app window.
5. **Keyboard shortcuts** — Cmd+N new job, Cmd+R refresh, etc.

### Medium-term
1. **Library browser** — browse downloaded files in-app with metadata.
2. **Audio preview** — stream a track before downloading (via remote mpv).
3. **Duplicate detection** — skip albums already in library.
4. **Smart queue** — prioritize albums by rarity, bitrate, etc.
5. **Dark/light theme toggle**.

### Long-term
1. **Multiple remote hosts** — switch between PVE containers, VPS, etc.
2. **Docker deployment** — run sldl in a container, manage via this app.
3. **Web version** — compile backend to WASM, run sldl in browser (unlikely due to networking).
4. **Mobile companion** — iOS/Android app for monitoring downloads on the go.

---

## File Inventory

Total files: ~30 source files across Rust, JS, CSS, and config.

```
sldl-remote/
├── ARCHITECTURE.md          # This document
├── README.md                # User-facing quickstart
├── justfile                 # Development commands
├── package.json             # Node.js deps + scripts
├── vite.config.js           # Vite dev server config
├── index.html               # App entry HTML
├── src/
│   ├── main.js              # JS entry point
│   ├── app.js               # Main app controller
│   ├── services/api.js      # Tauri invoke wrapper
│   └── styles.css           # Complete dark theme
└── src-tauri/
    ├── Cargo.toml           # Rust deps
    ├── tauri.conf.json      # Tauri app config
    ├── build.rs             # Build hook
    ├── capabilities/
    │   └── default.json     # Permission manifest
    └── src/
        ├── lib.rs           # Library entry + command registration
        ├── main.rs          # Binary entry
        ├── error.rs         # Error types
        ├── models/
        │   ├── mod.rs       # Shared types
        │   ├── config.rs    # AppConfig + presets
        │   └── job.rs       # Job + request/response types
        ├── services/
        │   ├── mod.rs       # Helpers
        │   ├── crypto.rs    # AES-256-GCM
        │   ├── ssh.rs       # SSH execution
        │   ├── config_manager.rs
        │   └── job_manager.rs
        └── commands/
            ├── mod.rs       # Common helpers
            ├── config.rs    # Config commands
            ├── jobs.rs      # Job commands
            ├── library.rs   # Library commands
            └── album_mode.rs
```

---

## Decision Log

| Date | Decision | Context |
|------|----------|---------|
| 2026-05-22 | Tauri v2 over Electron | Smaller binary, native feel, Rust backend |
| 2026-05-22 | Vanilla JS over React/Vue/Svelte | No framework overhead, easier to maintain |
| 2026-05-22 | SSH binary over SSH library (russh) | Avoid OpenSSL dependency, simpler |
| 2026-05-22 | AES-256-GCM over password-based encryption | Key stored on disk, simpler UX |
| 2026-05-22 | In-memory jobs over persisted jobs | Simpler, source of truth is remote |
| 2026-05-22 | Per-job config files over shared config | Isolation, no conflicts |
| 2026-05-22 | Sequential downloads over concurrent | Avoid Soulseek bans on large playlists |
| 2026-05-22 | "Associated Album Mode" as feature name | User chose this over alternatives |

---

## Contact / Maintenance

This is a personal project. For issues or contributions, open a GitHub issue on the sldl-remote repository.
