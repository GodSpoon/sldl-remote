pub mod config;
pub mod jobs;
pub mod library;
pub mod album_mode;





/// Helper: convert AppResult to Tauri Result<T, String>.
pub fn ok<T>(r: crate::error::AppResult<T>) -> Result<T, String> {
    r.map_err(|e| e.to_string())
}

/// Common response wrapper for commands that need both data and notifications.
#[derive(serde::Serialize)]
pub struct CommandResponse<T> {
    pub data: T,
    pub notifications: Vec<crate::models::Notification>,
}

impl<T> CommandResponse<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            notifications: vec![],
        }
    }

    pub fn with_notification(mut self, n: crate::models::Notification) -> Self {
        self.notifications.push(n);
        self
    }
}

/// Helper: build a success notification.
pub fn success_notif(title: &str, message: &str) -> crate::models::Notification {
    crate::models::Notification {
        level: "success".into(),
        title: title.into(),
        message: message.into(),
        job_id: None,
    }
}

/// Helper: build an error notification.
pub fn error_notif(title: &str, message: &str) -> crate::models::Notification {
    crate::models::Notification {
        level: "error".into(),
        title: title.into(),
        message: message.into(),
        job_id: None,
    }
}

/// Helper: build an info notification.
pub fn info_notif(title: &str, message: &str) -> crate::models::Notification {
    crate::models::Notification {
        level: "info".into(),
        title: title.into(),
        message: message.into(),
        job_id: None,
    }
}

/// Helper: build a warning notification.
pub fn warn_notif(title: &str, message: &str) -> crate::models::Notification {
    crate::models::Notification {
        level: "warning".into(),
        title: title.into(),
        message: message.into(),
        job_id: None,
    }
}

/// Parse a Spotify URL to extract type and ID.
#[tauri::command]
pub fn parse_spotify_url(url: String) -> Result<serde_json::Value, String> {
    let job_type = crate::models::JobType::detect(&url);
    let id = crate::services::extract_spotify_id(&url);
    Ok(serde_json::json!({
        "type": job_type.label(),
        "id": id,
        "url": url,
    }))
}

/// Validate a job before starting it (URL format, disk space, etc.).
#[tauri::command]
pub async fn validate_job(url: String) -> Result<crate::models::JobValidation, String> {
    let config = crate::services::config_manager::ConfigManager::load_fresh()
        .map_err(|e| e.to_string())?;

    let job_type = crate::models::JobType::detect(&url);
    let spotify_id = crate::services::extract_spotify_id(&url);
    let mut warnings = vec![];

    // Check URL is actually Spotify
    if !url.contains("spotify.com") && !url.contains("spotify:") {
        warnings.push("URL does not look like a Spotify link".into());
    }

    // Check disk space
    let (_total, free) = crate::services::check_disk_space(&config.ssh_target(),
        &config.output_path,
    )
    .await
    .map_err(|e| e.to_string())?;

    if free < 5.0 {
        warnings.push(format!(
            "Only {:.1} GB free on remote. Large downloads may fail.",
            free
        ));
    }

    // Estimate track count (very rough)
    let estimated_tracks = match job_type {
        crate::models::JobType::Playlist => {
            // Can't know without fetching. Default warning for large playlists.
            warnings.push("Playlist size unknown until fetched".into());
            None
        }
        crate::models::JobType::Album => Some(10),
        crate::models::JobType::Artist => {
            warnings.push("Artist discographies can be very large".into());
            None
        }
        _ => None,
    };

    Ok(crate::models::JobValidation {
        url_type: job_type.label().into(),
        spotify_id,
        estimated_tracks,
        warnings,
    })
}

/// Export the encrypted config to a file chosen by the user.
#[tauri::command]
pub fn export_config(path: String) -> Result<(), String> {
    let mgr = crate::services::config_manager::ConfigManager::default();
    let p = std::path::Path::new(&path);
    mgr.export(p).map_err(|e| e.to_string())
}

/// Import an encrypted config from a file chosen by the user.
#[tauri::command]
pub fn import_config(path: String) -> Result<(), String> {
    let mut mgr = crate::services::config_manager::ConfigManager::default();
    let p = std::path::Path::new(&path);
    mgr.import(p).map_err(|e| e.to_string())
}

/// Get built-in config profiles.
#[tauri::command]
pub fn get_builtin_profiles() -> Vec<crate::models::ConfigProfile> {
    crate::models::ConfigProfile::builtin_profiles()
}

/// Apply a profile patch to a config.
#[tauri::command]
pub fn apply_profile(
    mut config: crate::models::AppConfig,
    profile: crate::models::ConfigProfile,
) -> crate::models::AppConfig {
    if let Some(obj) = profile.config_patch.as_object() {
        if let Some(v) = obj.get("searches_per_time") {
            if let Some(n) = v.as_u64() { config.searches_per_time = n as u32; }
        }
        if let Some(v) = obj.get("searches_renew_time") {
            if let Some(n) = v.as_u64() { config.searches_renew_time = n as u32; }
        }
        if let Some(v) = obj.get("fast_search") {
            if let Some(b) = v.as_bool() { config.fast_search = b; }
        }
        if let Some(v) = obj.get("pref_format") {
            if let Some(s) = v.as_str() { config.pref_format = s.into(); }
        }
        if let Some(v) = obj.get("pref_min_bitrate") {
            if let Some(n) = v.as_u64() { config.pref_min_bitrate = n as u32; }
        }
        if let Some(v) = obj.get("pref_max_bitrate") {
            if let Some(n) = v.as_u64() { config.pref_max_bitrate = n as u32; }
        }
    }
    config
}

/// Get the full remote host info (caches on first call).
#[tauri::command]
pub async fn get_remote_host_info() -> Result<crate::models::RemoteHostInfo, String> {
    let config = crate::services::config_manager::ConfigManager::load_fresh()
        .map_err(|e| e.to_string())?;
    crate::services::ssh::get_system_info(&config)
        .await
        .map_err(|e| e.to_string())
}

/// Run a full health check on the remote environment.
#[tauri::command]
pub async fn health_check() -> Result<crate::models::HealthCheck, String> {
    let config = crate::services::config_manager::ConfigManager::load_fresh()
        .map_err(|e| e.to_string())?;
    let mut errors = vec![];

    let ssh_ok = crate::services::ssh::run(
        &config.ssh_target(),
        "echo ok"
    )
    .await
    .is_ok();

    if !ssh_ok {
        errors.push("SSH connection failed".into());
    }

    let sldl_version = if ssh_ok {
        match crate::services::verify_sldl_binary(&config.ssh_target(),
            &config.sldl_path,
        )
        .await
        {
            Ok(v) => {
                if v.is_empty() || v == "unknown" {
                    errors.push(format!("sldl not found at {}", config.sldl_path));
                    None
                } else {
                    Some(v)
                }
            }
            Err(e) => {
                errors.push(format!("sldl check failed: {}", e));
                None
            }
        }
    } else {
        None
    };

    let disk_ok = if ssh_ok {
        match crate::services::check_disk_space(
            &config.ssh_target(),
            &config.output_path,
        )
        .await
        {
            Ok((_, free)) => {
                if free < 1.0 {
                    errors.push(format!(
                        "Only {:.1} GB free on {}",
                        free, config.output_path
                    ));
                    false
                } else {
                    true
                }
            }
            Err(e) => {
                errors.push(format!("Disk check failed: {}", e));
                false
            }
        }
    } else {
        false
    };

    // Spotify API check: can we list tracks from a public playlist?
    let spotify_ok = if ssh_ok {
        // Quick test: try to get liked songs count (1 track)
        match crate::services::ssh::run(
            &config.ssh_target(),
            &format!(
                "{} spotify-likes -n 1 --print tracks 2>&1 | head -3",
                config.sldl_path
            ),
        )
        .await
        {
            Ok(out) => {
                out.contains("Loading Spotify") || out.contains("Downloading")
            }
            Err(_) => false,
        }
    } else {
        false
    };

    // Soulseek login check
    let slsk_ok = if ssh_ok && sldl_version.is_some() {
        match crate::services::ssh::run(
            &config.ssh_target(),
            &format!(
                "{} 'The Beatles - Eleanor Rigby' -n 1 --listen-port 49996 2>&1 | head -5",
                config.sldl_path
            ),
        )
        .await
        {
            Ok(out) => {
                !out.contains("Login failed") && !out.contains("banned")
            }
            Err(_) => false,
        }
    } else {
        false
    };

    Ok(crate::models::HealthCheck {
        ssh_ok,
        sldl_binary_ok: sldl_version.is_some(),
        sldl_version,
        soulseek_login_ok: slsk_ok,
        disk_space_ok: disk_ok,
        spotify_api_ok: spotify_ok,
        errors,
    })
}
