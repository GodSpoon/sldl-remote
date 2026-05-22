# Sldl Remote

A cross-platform Tauri GUI for remotely controlling [sldl](https://github.com/fiso64/sldl) downloads on a PVE host (or any SSH-accessible machine).

## What It Does

- Paste Spotify URLs (playlists, albums, artists, tracks)
- Download via sldl on a remote Linux host over SSH
- Auto-organize into `playlists/`, `albums/`, `artists/` subfolders
- **Associated Album Mode** — for playlists, excavates and downloads the full album for every track
- Encrypted local config file (cross-platform)
- Live log viewer and job queue

## Current Architecture

```
Mac (this app)  ──SSH──►  PVE Container (dev-12 @ 192.168.70.12)
                             └── sldl v2.6.0
                             └── /media/music/{playlists,albums,artists}
```

### Key Findings From Building This

| Lesson | Detail |
|--------|--------|
| **Soulseek bans are IP-based** | 30-min ban after ~34 searches per 220 seconds. Running two 800+ track playlists concurrently triggered a 45+ minute lockout. |
| **Rate limit config** | `searches-per-time = 20`, `searches-renew-time = 300`, `fast-search = false` keeps you safe on big playlists. |
| **sldl binary** | macOS unsigned binaries get killed by Gatekeeper. Need `xattr -c` + `codesign -s -`. |
| **SSH SOCKS doesn't proxy .NET apps easily** | Tried proxychains-ng and sshuttle — .NET's Socket API bypasses system SOCKS proxy settings. Best fix: run sldl directly on the remote host. |
| **Sequential > concurrent for big playlists** | 802-track + 928-track playlists must run one at a time. |

## Project Structure

```
sldl-remote/
├── src-tauri/
│   ├── Cargo.toml          # Rust deps (tauri, aes-gcm, tokio, etc.)
│   ├── tauri.conf.json     # Tauri v2 app config
│   ├── build.rs            # Tauri build hook
│   ├── capabilities/
│   │   └── default.json    # Permission manifest
│   └── src/
│       └── main.rs         # All Rust backend logic
├── src/
│   ├── index.html          # Vite entry
│   ├── main.js             # Frontend UI logic
│   └── styles.css          # Dark theme
├── package.json            # Vite + @tauri-apps/api
├── vite.config.js          # Vite dev server config
└── README.md               # This file
```

## Tech Stack

- **Tauri v2** (Rust + Web) — lightweight native app
- **Vanilla JS + Vite** — no frontend framework overhead
- **AES-256-GCM** encrypted config file via `aes-gcm` crate
- **SSH key auth** — app spawns `ssh` commands to the remote host

## Prerequisites

- **macOS/Linux/Windows** with SSH client installed
- **Rust** (via [rustup](https://rustup.rs/))
- **Node.js** (for Vite dev server)
- **Remote host** with:
  - sldl installed (`/usr/local/bin/sldl`)
  - SSH key auth configured (no password prompts)
  - Soulseek + Spotify credentials configured

## Build & Run

```bash
cd sldl-remote

# 1. Install frontend deps
npm install

# 2. Install Tauri CLI globally
cargo install tauri-cli

# 3. Run in dev mode
npm run tauri dev

# 4. Build release binary
npm run tauri build
```

## Config File

Stored encrypted at:
- macOS: `~/Library/Application Support/com.spoon.sldl-remote/config.enc`
- Linux: `~/.config/sldl-remote/config.enc`
- Windows: `%APPDATA%\spoon\sldl-remote\config.enc`

Encryption key is in `.key` file alongside it (permissions 0600 on Unix).

## Tauri Commands (Rust Backend)

| Command | Description |
|---------|-------------|
| `get_config` | Load and decrypt config |
| `save_config_cmd` | Encrypt and save config |
| `test_connection` | SSH to remote, run `uname -sm` |
| `start_job` | Write per-job sldl config, spawn nohup sldl |
| `get_jobs` | List in-memory jobs |
| `refresh_job_status` | SSH tail logs, parse progress |
| `get_job_logs` | SSH tail -60 of job log |
| `stop_job` | SSH kill job PID |
| `list_downloads` | SSH find files in output dir |

## Known Issues / TODO

1. **Associated Album Mode** — UI toggle exists but the album-extraction pipeline (parse playlist tracks → queue album jobs) is not yet implemented. Currently just stores the flag.
2. **Job persistence** — Jobs are in-memory only. App restart loses job history. Could store to SQLite or JSON.
3. **Log parsing is brittle** — Uses string matching on sldl output. sldl format changes would break it.
4. **No SCP** — Per-job configs are written via SSH heredoc. Large configs or special chars could break this.
5. **Icon bundling disabled** — `bundle.active = false` in `tauri.conf.json`. Enable and add icons for production.

## License

Same as sldl (AGPL-3.0)
