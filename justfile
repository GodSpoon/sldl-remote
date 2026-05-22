# Sldl Remote — Development commands
# Install just: cargo install just

# Default: show help
default:
    @just --list

# Development: run the Tauri app with hot reload
dev:
    cargo tauri dev

# Build the frontend assets
build-web:
    npm run build

# Build the full Tauri app for release
build:
    cargo tauri build

# Run Rust tests
test-rust:
    cd src-tauri && cargo test

# Check Rust code
check-rust:
    cd src-tauri && cargo check

# Format Rust code
fmt-rust:
    cd src-tauri && cargo fmt

# Format JS code (requires prettier)
fmt-js:
    npx prettier --write "src/**/*.{js,css,html}"

# Lint Rust
lint-rust:
    cd src-tauri && cargo clippy -- -D warnings

# Clean build artifacts
clean:
    rm -rf dist src-tauri/target
    cd src-tauri && cargo clean

# Install all dependencies
install:
    npm install
    cd src-tauri && cargo fetch

# Update dependencies
update:
    npm update
    cd src-tauri && cargo update

# Run a specific Rust test by name
test-filter FILTER:
    cd src-tauri && cargo test {{FILTER}}

# Build release bundle for current platform
bundle:
    cargo tauri build --target universal-apple-darwin

# Build for Linux (requires cross-compilation setup)
bundle-linux:
    cargo tauri build --target x86_64-unknown-linux-gnu

# Build for Windows (requires cross-compilation setup)
bundle-windows:
    cargo tauri build --target x86_64-pc-windows-msvc

# Generate API docs for Rust backend
doc-rust:
    cd src-tauri && cargo doc --no-deps --open

# Start a local file server for the built frontend (testing)
serve:
    npx serve dist -l 3000

# Print project info
info:
    @echo "Sldl Remote"
    @echo "  Frontend:  Vite + Vanilla JS"
    @echo "  Backend:   Tauri v2 (Rust)"
    @echo "  Crypto:    AES-256-GCM"
    @echo "  Remote:    SSH command execution"
    @echo "  Platform:  macOS / Linux / Windows"
    @echo ""
    @echo "Key commands:"
    @echo "  just dev        — Run in dev mode"
    @echo "  just build      — Build release binary"
    @echo "  just test-rust  — Run Rust tests"
    @echo "  just clean      — Clean all build artifacts"
    @echo ""
    @echo "Config dir:"
    @just config-dir

# Print the config directory path
config-dir:
    @node -e "const {ProjectDirs} = require('directories'); const d = ProjectDirs.from('com', 'spoon', 'sldl-remote'); console.log(d.configDir())"

# Quick health check: verify all files exist
verify-structure:
    @test -f src-tauri/Cargo.toml || { echo "Missing src-tauri/Cargo.toml"; exit 1; }
    @test -f src-tauri/tauri.conf.json || { echo "Missing src-tauri/tauri.conf.json"; exit 1; }
    @test -f src-tauri/src/lib.rs || { echo "Missing src-tauri/src/lib.rs"; exit 1; }
    @test -f src-tauri/src/main.rs || { echo "Missing src-tauri/src/main.rs"; exit 1; }
    @test -f src/main.js || { echo "Missing src/main.js"; exit 1; }
    @test -f src/app.js || { echo "Missing src/app.js"; exit 1; }
    @test -f src/services/api.js || { echo "Missing src/services/api.js"; exit 1; }
    @test -f src/styles.css || { echo "Missing src/styles.css"; exit 1; }
    @test -f index.html || { echo "Missing index.html"; exit 1; }
    @test -f vite.config.js || { echo "Missing vite.config.js"; exit 1; }
    @test -f package.json || { echo "Missing package.json"; exit 1; }
    @echo "All required files present."

# Count lines of code
loc:
    @echo "Rust LOC:"
    @find src-tauri/src -name "*.rs" | xargs wc -l | tail -1
    @echo "JS LOC:"
    @find src -name "*.js" | xargs wc -l | tail -1
    @echo "CSS LOC:"
    @find src -name "*.css" | xargs wc -l | tail -1

# Archive the project (excluding node_modules and target)
archive:
    tar czf sldl-remote.tar.gz \
        --exclude='node_modules' \
        --exclude='src-tauri/target' \
        --exclude='dist' \
        .
    @echo "Created sldl-remote.tar.gz"

# Watch Rust files and re-run tests on change
watch-rust:
    cd src-tauri && cargo watch -x test

# Watch JS files and rebuild on change (separate from dev mode)
watch-js:
    npx vite build --watch

# Run the backend without the frontend (for API testing)
backend-only:
    cd src-tauri && cargo run --bin sldl-remote

# Build the frontend for production and copy into src-tauri
build-and-copy:
    npm run build
    cp -r dist/* src-tauri/dist/ 2>/dev/null || true

# Full CI pipeline: format, lint, test, build
ci:
    just fmt-rust
    just fmt-js
    just lint-rust
    just test-rust
    just build-web
    just verify-structure
    @echo "CI pipeline complete."

# Print available Tauri CLI commands
tauri-help:
    cargo tauri --help

# Sign the macOS binary (ad-hoc, for local testing)
sign-macos:
    cargo tauri build
    codesign -s - src-tauri/target/universal-apple-darwin/release/bundle/macos/Sldl\ Remote.app
    xattr -cr src-tauri/target/universal-apple-darwin/release/bundle/macos/Sldl\ Remote.app

# Print the current sldl version on the remote host
remote-version:
    ssh 192.168.70.12 "/usr/local/bin/sldl --version"

# Tail the current download log on the remote host
remote-logs:
    ssh 192.168.70.12 "tail -f /tmp/sldl-run.log"

# Show disk usage on the remote music dir
remote-du:
    ssh 192.168.70.12 "du -sh /media/music/* 2>/dev/null | sort -h"

# List downloaded files on the remote host
remote-ls:
    ssh 192.168.70.12 "find /media/music -type f | sort | head -50"

# Kill all sldl processes on the remote host (emergency)
remote-kill:
    ssh 192.168.70.12 "pkill -f '/usr/local/bin/sldl' || echo 'No sldl processes found'"

# Restart the remote download daemon
remote-restart:
    just remote-kill
    ssh 192.168.70.12 "nohup /tmp/sldl-run.sh > /tmp/sldl-run.log 2>&1 &"
    @echo "Remote daemon restarted."

# Check if the remote download is still running
remote-status:
    ssh 192.168.70.12 "ps aux | grep sldl | grep -v grep || echo 'Not running'"

# Download the latest sldl release to the remote host
remote-update-sldl:
    ssh 192.168.70.12 "cd /tmp && curl -sL -o sldl.zip 'https://github.com/fiso64/sldl/releases/download/v2.6.0/sldl_linux-x64.zip' && unzip -o sldl.zip && mv sldl /usr/local/bin/sldl && chmod +x /usr/local/bin/sldl && /usr/local/bin/sldl --version"

# Backup the remote music library (rsync to local)
backup-music DEST="./music-backup":
    rsync -avz --progress 192.168.70.12:/media/music/ {{DEST}}/

# Restore the remote music library from local backup
restore-music SRC="./music-backup":
    rsync -avz --progress {{SRC}}/ 192.168.70.12:/media/music/
