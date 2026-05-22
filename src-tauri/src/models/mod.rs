pub mod config;
pub mod job;

pub use config::*;
pub use job::*;

use serde::{Deserialize, Serialize};

/// Response from the `discover_albums_from_playlist` command.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumDiscoveryResult {
    pub track_count: usize,
    pub unique_albums: Vec<AlbumInfo>,
    pub already_have: Vec<AlbumInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AlbumInfo {
    pub artist: String,
    pub album: String,
    pub track_title: String,
}

/// Overall library statistics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryStats {
    pub total_files: usize,
    pub total_artists: usize,
    pub total_albums: usize,
    pub total_size_mb: u64,
    pub by_category: Vec<CategoryStat>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CategoryStat {
    pub name: String,
    pub file_count: usize,
    pub size_mb: u64,
}

/// Frontend-facing connection health status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub ok: bool,
    pub host: String,
    pub os_info: String,
    pub disk_free_gb: f64,
    pub message: String,
}

/// Pagination wrapper for large lists.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
}

impl<T> Paginated<T> {
    pub fn new(items: Vec<T>, page: usize, per_page: usize) -> Self {
        let total = items.len();
        Self {
            items: items.into_iter().skip(page * per_page).take(per_page).collect(),
            total,
            page,
            per_page,
        }
    }
}

/// A raw file path in the remote music library.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LibraryFile {
    pub path: String,
    pub relative_path: String,
    pub size_bytes: u64,
    pub modified: String,
}

/// Frontend toast / notification payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Notification {
    pub level: String, // "info" | "success" | "warning" | "error"
    pub title: String,
    pub message: String,
    pub job_id: Option<String>,
}

/// Queue command for batching album downloads.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumQueueItem {
    pub artist: String,
    pub album: String,
    pub priority: i32, // higher = sooner
    pub source_playlist_id: Option<String>,
}

/// Pre-flight validation result before starting a job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobValidation {
    pub url_type: String,
    pub spotify_id: Option<String>,
    pub estimated_tracks: Option<usize>,
    pub warnings: Vec<String>,
}

/// Raw sldl log line parsed into structured data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: Option<String>,
    pub level: String, // "info" | "progress" | "success" | "error" | "search"
    pub message: String,
    pub raw: String,
}

/// Search result entry from sldl output.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub user: String,
    pub file_path: String,
    pub format: String,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub size_mb: Option<f64>,
    pub length_sec: Option<u32>,
}

/// Downloaded file metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadedTrack {
    pub job_id: String,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub format: String,
    pub bitrate: u32,
    pub length_sec: u32,
    pub file_path: String,
    pub downloaded_at: String,
}

/// User preference schema for the app (separate from sldl config).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserPreferences {
    pub auto_refresh_interval_ms: u64,
    pub log_tail_lines: usize,
    pub default_output_subdir: String,
    pub confirm_large_playlists: bool,
    pub playlist_threshold: usize,
    pub dark_mode: bool,
    pub notifications_enabled: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            auto_refresh_interval_ms: 3000,
            log_tail_lines: 60,
            default_output_subdir: "playlists".into(),
            confirm_large_playlists: true,
            playlist_threshold: 200,
            dark_mode: true,
            notifications_enabled: true,
        }
    }
}

/// Complete app state snapshot for export/import.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppStateSnapshot {
    pub config: AppConfig,
    pub preferences: UserPreferences,
    pub jobs: Vec<Job>,
    pub version: String,
    pub exported_at: String,
}

/// Remote host system info (cached after first connection).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RemoteHostInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub sldl_version: String,
    pub disk_total_gb: f64,
    pub disk_free_gb: f64,
    pub cpu_count: usize,
    pub memory_mb: u64,
    pub last_seen: String,
}

/// sldl config profile (for switching between quality presets).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigProfile {
    pub name: String,
    pub description: String,
    pub config_patch: serde_json::Value, // partial override of AppConfig fields
}

impl ConfigProfile {
    pub fn builtin_profiles() -> Vec<Self> {
        vec![
            Self {
                name: "Conservative".into(),
                description: "Slow but safe. Won't trigger bans on massive playlists.".into(),
                config_patch: serde_json::json!({
                    "searches_per_time": 10,
                    "searches_renew_time": 300,
                    "fast_search": false,
                    "pref_format": "mp3",
                    "pref_min_bitrate": 320,
                }),
            },
            Self {
                name: "Balanced".into(),
                description: "Default. Good for most use cases.".into(),
                config_patch: serde_json::json!({
                    "searches_per_time": 20,
                    "searches_renew_time": 300,
                    "fast_search": false,
                    "pref_format": "mp3,flac",
                    "pref_min_bitrate": 256,
                }),
            },
            Self {
                name: "Aggressive".into(),
                description: "Fast downloads. Risk of bans on large batches.".into(),
                config_patch: serde_json::json!({
                    "searches_per_time": 34,
                    "searches_renew_time": 220,
                    "fast_search": true,
                    "pref_format": "flac,mp3",
                    "pref_min_bitrate": 0,
                }),
            },
            Self {
                name: "Lossless Only".into(),
                description: "Only accept FLAC/WAV. Very strict.".into(),
                config_patch: serde_json::json!({
                    "searches_per_time": 20,
                    "searches_renew_time": 300,
                    "fast_search": false,
                    "pref_format": "flac,wav",
                    "pref_min_bitrate": 0,
                }),
            },
        ]
    }
}

/// Event emitted to frontend via Tauri channels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum BackendEvent {
    JobStarted { job_id: String, job_type: String },
    JobProgress { job_id: String, progress: String, percent: Option<f64> },
    JobCompleted { job_id: String, stats: JobStats },
    JobFailed { job_id: String, error: String },
    LogLine { job_id: String, line: String },
    ConnectionLost { host: String },
    ConnectionRestored { host: String },
    BanDetected { ip: Option<String>, retry_after_sec: u64 },
    Notification(Notification),
}

/// Job completion statistics.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct JobStats {
    pub tracks_total: usize,
    pub tracks_downloaded: usize,
    pub tracks_failed: usize,
    pub tracks_not_found: usize,
    pub total_mb: u64,
    pub elapsed_sec: u64,
}

/// Rate limiter state (shared across all jobs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RateLimitState {
    pub searches_available: u32,
    pub last_replenish: String,
    pub ban_until: Option<String>,
}

/// Health check result for the remote host.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthCheck {
    pub ssh_ok: bool,
    pub sldl_binary_ok: bool,
    pub sldl_version: Option<String>,
    /// True if sldl can log in to Soulseek (not banned).
    pub soulseek_login_ok: bool,
    pub disk_space_ok: bool,
    pub spotify_api_ok: bool,
    pub errors: Vec<String>,
}
