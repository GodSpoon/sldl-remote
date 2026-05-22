use chrono::Local;
use serde::{Deserialize, Serialize};

/// A single download job tracked by the app.
///
/// Jobs are ephemeral (in-memory). The source of truth for job state
/// is the remote sldl process + its log file on the remote host.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub url: String,
    pub job_type: JobType,
    /// True if this job was created via Associated Album Mode.
    pub associated_album_mode: bool,
    pub status: JobStatus,
    pub progress: String,
    /// Absolute output directory on remote host.
    pub output_dir: String,
    /// Absolute log file path on remote host.
    pub log_path: String,
    /// Absolute per-job sldl.conf path on remote host.
    pub remote_conf_path: String,
    /// Remote listen port assigned to this job.
    pub listen_port: u16,
    /// Remote PID of the sldl process (if known).
    pub pid: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
    /// Number of tracks known to exist (from playlist extraction).
    pub total_tracks: Option<usize>,
    /// Tracks successfully downloaded so far.
    pub downloaded_count: usize,
    /// Tracks that failed after all retries.
    pub failed_count: usize,
    /// Tracks not found on Soulseek.
    pub not_found_count: usize,
    /// Total MB downloaded so far.
    pub total_mb: u64,
    /// For album-mode jobs: list of album jobs spawned from this.
    pub child_job_ids: Vec<String>,
    /// For child album jobs: the parent playlist job ID.
    pub parent_job_id: Option<String>,
    /// Last-seen log line (for deduplication during polling).
    pub last_log_line: String,
    /// Parsed completion percentage (0.0 - 100.0).
    pub percent_complete: Option<f64>,
    /// Whether the user has acknowledged completion/failure.
    pub acknowledged: bool,
}

impl Default for Job {
    fn default() -> Self {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        Self {
            id: String::new(),
            url: String::new(),
            job_type: JobType::Auto,
            associated_album_mode: false,
            status: JobStatus::Pending,
            progress: "Pending".into(),
            output_dir: String::new(),
            log_path: String::new(),
            remote_conf_path: String::new(),
            listen_port: 0,
            pid: None,
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
            error_message: None,
            total_tracks: None,
            downloaded_count: 0,
            failed_count: 0,
            not_found_count: 0,
            total_mb: 0,
            child_job_ids: vec![],
            parent_job_id: None,
            last_log_line: String::new(),
            percent_complete: None,
            acknowledged: false,
        }
    }
}

/// Job classification derived from the Spotify URL.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobType {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "playlist")]
    Playlist,
    #[serde(rename = "album")]
    Album,
    #[serde(rename = "artist")]
    Artist,
    #[serde(rename = "track")]
    Track,
    #[serde(rename = "aggregate")]
    Aggregate,
}

impl JobType {
    pub fn detect(url: &str) -> Self {
        let lower = url.to_lowercase();
        if lower.contains("/playlist/") { Self::Playlist }
        else if lower.contains("/album/") { Self::Album }
        else if lower.contains("/artist/") { Self::Artist }
        else if lower.contains("/track/") { Self::Track }
        else { Self::Auto }
    }

    pub fn sldl_flag(&self) -> &'static str {
        match self {
            Self::Album => "-a",
            Self::Artist | Self::Aggregate => "-g",
            _ => "",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Playlist => "playlist",
            Self::Album => "album",
            Self::Artist => "artist",
            Self::Track => "track",
            Self::Aggregate => "aggregate",
        }
    }
}

/// Job lifecycle state.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "stopped")]
    Stopped,
    #[serde(rename = "banned")]
    Banned,
    #[serde(rename = "queued")]
    Queued,
}

impl JobStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Running | Self::Queued)
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Stopped)
    }
}

/// Request payload for starting a new job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartJobRequest {
    pub url: String,
    pub job_type: String,
    pub associated_album_mode: bool,
    /// Override output sub-directory (optional).
    pub output_subdir: Option<String>,
    /// Override listen port (optional, for testing).
    pub listen_port: Option<u16>,
    /// If set, enqueue but don't start immediately.
    pub enqueue_only: bool,
}

/// Request payload for starting Associated Album Mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartAlbumModeRequest {
    pub playlist_job_id: String,
    pub selected_albums: Vec<AlbumSelection>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlbumSelection {
    pub artist: String,
    pub album: String,
    pub priority: i32,
}

/// Job filter for listing.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct JobFilter {
    pub status: Option<String>,
    pub job_type: Option<String>,
    pub search: Option<String>,
}

/// Sort order for job lists.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum JobSort {
    CreatedAt,
    UpdatedAt,
    Progress,
    Status,
}

/// Request to update a job's acknowledgement state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AckJobRequest {
    pub job_id: String,
    pub acknowledged: bool,
}

/// Response from starting a job.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StartJobResponse {
    pub job_id: String,
    pub log_path: String,
    pub listen_port: u16,
    pub warnings: Vec<String>,
}

/// Batch operation request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchJobRequest {
    pub job_ids: Vec<String>,
    pub operation: BatchOperation,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BatchOperation {
    Stop,
    Restart,
    Delete,
    Acknowledge,
}

/// Parsed progress info extracted from sldl log output.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ParsedProgress {
    pub downloaded: usize,
    pub failed: usize,
    pub not_found: usize,
    pub total: usize,
    pub percent: Option<f64>,
    pub current_track: Option<String>,
    pub current_action: Option<String>,
}
