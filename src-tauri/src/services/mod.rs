pub mod config_manager;
pub mod crypto;
pub mod job_manager;
pub mod ssh;

pub use config_manager::*;
pub use crypto::*;
pub use job_manager::*;
pub use ssh::*;

use std::sync::Mutex;

/// Global application services container, managed by Tauri state.
pub struct AppServices {
    pub config: Mutex<config_manager::ConfigManager>,
    pub jobs: Mutex<job_manager::JobManager>,
}

impl Default for AppServices {
    fn default() -> Self {
        Self {
            config: Mutex::new(config_manager::ConfigManager::default()),
            jobs: Mutex::new(job_manager::JobManager::default()),
        }
    }
}

/// Helper: get a random listen port in the configured range.
pub fn allocate_listen_port(base: u16) -> u16 {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    base + rng.gen_range(0..100)
}

/// Helper: extract a Spotify ID from any Spotify URL.
pub fn extract_spotify_id(url: &str) -> Option<String> {
    url.split('/')
        .last()
        .and_then(|s| s.split('?').next())
        .map(|s| s.to_string())
}

/// Helper: build the output subdirectory path based on job type and URL.
pub fn build_output_subdir(base: &str, job_type: &crate::models::JobType, url: &str) -> String {
    use crate::models::JobType;
    let sid = extract_spotify_id(url).unwrap_or_else(|| "unknown".into());
    match job_type {
        JobType::Playlist => format!("{}/playlists/{}", base, sid),
        JobType::Album => format!("{}/albums", base),
        JobType::Artist | JobType::Aggregate => format!("{}/artists", base),
        JobType::Track => format!("{}/tracks", base),
        JobType::Auto => format!("{}/downloads/{}", base, sid),
    }
}

/// Helper: build the remote sldl command string.
pub fn build_sldl_command(config: &crate::models::AppConfig, job: &crate::models::Job) -> String {
    let flag = job.job_type.sldl_flag();
    let url_escaped = shell_escape::escape(job.url.clone().into());

    if flag.is_empty() {
        format!(
            "nohup '{}' {} --config '{}' --listen-port {} > '{}' 2>&1 &
echo $!",
            config.sldl_path, url_escaped, job.remote_conf_path, job.listen_port, job.log_path,
        )
    } else {
        format!(
            "nohup '{}' {} {} --config '{}' --listen-port {} > '{}' 2>&1 &
echo $!",
            config.sldl_path,
            flag,
            url_escaped,
            job.remote_conf_path,
            job.listen_port,
            job.log_path,
        )
    }
}

/// Helper: parse sldl log for progress metrics.
pub fn parse_progress(log_tail: &str) -> crate::models::ParsedProgress {
    use crate::models::ParsedProgress;
    let mut p = ParsedProgress::default();

    for line in log_tail.lines().rev() {
        // Pattern: "Downloaded X of Total Y (Z%)"
        if let Some(start) = line.find("Downloaded") {
            let rest = &line[start..];
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 6 {
                if let Ok(d) = parts[1].parse::<usize>() {
                    p.downloaded = d;
                }
                if let Ok(t) = parts[4].parse::<usize>() {
                    p.total = t;
                }
                if p.total > 0 {
                    p.percent = Some((p.downloaded as f64 / p.total as f64) * 100.0);
                }
            }
        }
        // Pattern: "Downloaded X, Failed Y of Total Z"
        if let Some(start) = line.find("Downloaded") {
            let rest = &line[start..];
            if let Some(failed_idx) = rest.find("Failed") {
                let failed_part = &rest[failed_idx..];
                let parts: Vec<&str> = failed_part.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(f) = parts[1].parse::<usize>() {
                        p.failed = f;
                    }
                }
            }
        }
        // Pattern: "Not found: Artist - Title (length)"
        if line.starts_with("Not found:") {
            p.not_found += 1;
        }
        // Pattern: "Searching: Artist - Title"
        if line.starts_with("Searching:") {
            p.current_track = Some(line["Searching:".len()..].trim().to_string());
            p.current_action = Some("searching".into());
        }
        // Pattern: "Initialize: ..."
        if line.starts_with("Initialize:") {
            p.current_action = Some("downloading".into());
        }
        // Pattern: "All downloads complete" or similar
        if line.contains("complete") || line.contains("Succeeded") {
            p.current_action = Some("finishing".into());
        }
    }

    p
}

/// Detect a Soulseek ban from SSH / sldl output.
pub fn detect_ban(stderr: &str) -> Option<(String, u64)> {
    let lower = stderr.to_lowercase();
    if lower.contains("banned") || lower.contains("30 minutes") || lower.contains("rate limit") {
        let msg = if lower.contains("30 minutes") {
            "Soulseek 30-minute ban detected"
        } else {
            "Soulseek rate limit / ban detected"
        };
        return Some((msg.into(), 1800));
    }
    if lower.contains("the underlying tcp connection is closed") && lower.contains("timeout") {
        return Some(("Possible Soulseek ban (connection closed)".into(), 1800));
    }
    None
}

/// Check if the sldl binary exists and is executable on the remote host.
pub async fn verify_sldl_binary(host: &str, path: &str) -> Result<String, crate::error::AppError> {
    let out = ssh::run(host, &format!("{} --version 2>&1", path)).await?;
    Ok(out.trim().to_string())
}

/// Check disk space on the remote output directory.
pub async fn check_disk_space(
    host: &str,
    path: &str,
) -> Result<(f64, f64), crate::error::AppError> {
    let out = ssh::run(
        host,
        &format!("df -BG '{}' | tail -1 | awk '{{print $2,$4}}'", path),
    )
    .await?;
    let parts: Vec<&str> = out.trim().split_whitespace().collect();
    if parts.len() >= 2 {
        let total = parts[0].trim_end_matches('G').parse::<f64>().unwrap_or(0.0);
        let free = parts[1].trim_end_matches('G').parse::<f64>().unwrap_or(0.0);
        Ok((total, free))
    } else {
        Err(crate::error::AppError::Ssh {
            host: host.into(),
            cmd: "df".into(),
            cause: format!("Unexpected df output: {}", out),
        })
    }
}
