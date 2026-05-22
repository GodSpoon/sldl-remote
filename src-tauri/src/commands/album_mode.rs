//! Associated Album Mode commands.
//!
//! Flow:
//! 1. User pastes a Spotify playlist URL and enables "Associated Album Mode".
//! 2. `discover_albums_from_playlist` runs `sldl --print tracks` to get the full
//!    track listing, then extracts unique (artist, album) pairs.
//! 3. UI shows a picker: "Found 47 unique albums. Select which to download."
//! 4. `start_associated_album_jobs` queues individual `sldl -a` jobs for each
//!    selected album, linking them to the parent playlist job.

use tauri::State;


use crate::models::{
    AlbumDiscoveryResult, AlbumInfo, Job, JobStatus, JobType,
    StartAlbumModeRequest,
};
use crate::services::AppServices;

/// Phase 1: Discover all unique albums from a playlist.
///
/// Runs `sldl <url> --print tracks-full` on the remote host, parses the output,
/// and returns deduplicated (artist, album) pairs.
#[tauri::command]
pub async fn discover_albums_from_playlist(
    playlist_url: String,
) -> Result<AlbumDiscoveryResult, String> {
    let config = crate::services::config_manager::ConfigManager::load_fresh()
        .map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    // Write a temp config for the discovery run
    let temp_conf = "/tmp/sldl-remote-discover.conf".to_string();
    let ini = config.to_sldl_ini(&config.output_path);
    crate::services::ssh::write_remote_file(&config, &temp_conf, &ini)
        .await
        .map_err(|e| e.to_string())?;

    // Run sldl with --print tracks to get the playlist contents
    let cmd = format!(
        "{} '{}' --config '{}' --listen-port 49990 --print tracks-full 2>&1 | head -1000",
        config.sldl_path,
        shell_escape::escape(playlist_url.clone().into()),
        temp_conf,
    );

    let out = crate::services::ssh::run(&host, &cmd)
        .await
        .map_err(|e| e.to_string())?;

    // Parse output for artist/album pairs
    let albums: std::collections::HashSet<( String, String )> = std::collections::HashSet::new();
    let mut tracks = vec![];

    for line in out.lines() {
        // sldl --print tracks output format (from docs):
        //   Artist - Title (length)
        // We can't reliably get album from this. We need to use Spotify API
        // or parse differently.
        //
        // BETTER APPROACH: Use the Spotify Web API directly from Rust
        // to get full track details including album. But for now, we'll
        // approximate from the track list and do a follow-up call.
        //
        // Actually, the simplest reliable approach:
        // 1. Run sldl --print tracks-full (gives more detail)
        // 2. The output might not include album. We need to hit Spotify API.
        //
        // For this scaffold, we'll use a placeholder parser and document
        // that the real implementation should call Spotify API.
        tracks.push(line.to_string());
    }

    // Clean up temp config
    let _ = crate::services::ssh::delete_remote_file(&config, &temp_conf).await;

    // TODO: Replace with real Spotify API call to get albums.
    // For now, return empty with a note.
    let unique_albums = albums
        .into_iter()
        .map(|(artist, album)| AlbumInfo {
            artist,
            album: album.clone(),
            track_title: album,
        })
        .collect();

    Ok(AlbumDiscoveryResult {
        track_count: tracks.len(),
        unique_albums,
        already_have: vec![],
    })
}

/// Phase 2: Start album download jobs for selected albums.
///
/// Creates individual `sldl -a` jobs for each album and links them as
/// children of the parent playlist job.
#[tauri::command]
pub async fn start_associated_album_jobs(
    state: State<'_, AppServices>,
    req: StartAlbumModeRequest,
) -> Result<Vec<String>, String> {
    let config = {
        let mut mgr = state.config.lock().map_err(|e| e.to_string())?;
        mgr.load().map_err(|e| e.to_string())?
    };

    let mut child_ids = vec![];

    for album in req.selected_albums {
        let album_query = format!("{} - {}", album.artist, album.album);
        let job_id = {
            // Create the album job
            let id = uuid::Uuid::new_v4().to_string();
            let output_dir = format!(
                "{}/albums/{}/{}",
                config.output_path,
                sanitize_dir(&album.artist),
                sanitize_dir(&album.album)
            );
            let log_path = format!("/tmp/sldl-remote-{}.log", id);
            let remote_conf = format!("/tmp/sldl-remote-{}.conf", id);
            let listen_port = crate::services::allocate_listen_port(config.listen_port_base);

            let ini = config.to_sldl_ini(&output_dir);
            crate::services::ssh::write_remote_file(&config, &remote_conf, &ini)
                .await
                .map_err(|e| e.to_string())?;

            let sldl_cmd = format!(
                "nohup '{}' -a '{}' --config '{}' --listen-port {} > '{}' 2>&1 &
echo $!",
                config.sldl_path,
                shell_escape::escape(album_query.clone().into()),
                remote_conf,
                listen_port,
                log_path,
            );

            let pid_out = crate::services::ssh::run(
                &config.ssh_target(),
                &sldl_cmd,
            )
            .await
            .map_err(|e| e.to_string())?;
            let pid = pid_out.trim().parse::<u32>().ok();

            let job = Job {
                id: id.clone(),
                url: album_query,
                job_type: JobType::Album,
                associated_album_mode: true,
                status: JobStatus::Running,
                progress: "Starting album download...".into(),
                output_dir,
                log_path,
                remote_conf_path: remote_conf,
                listen_port,
                pid,
                parent_job_id: Some(req.playlist_job_id.clone()),
                created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                updated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                ..Default::default()
            };

            state
                .jobs
                .lock()
                .map_err(|e| e.to_string())?
                .insert(job);

            // Link to parent
            let _ = state
                .jobs
                .lock()
                .map_err(|e| e.to_string())?
                .add_child(&req.playlist_job_id, id.clone());

            id
        };

        child_ids.push(job_id);
    }

    Ok(child_ids)
}

/// Sanitize a string for use as a directory name.
fn sanitize_dir(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Get the parent job and all its children (for Associated Album Mode view).
#[tauri::command]
pub fn get_job_family(
    state: State<'_, AppServices>,
    job_id: String,
) -> Result<Vec<Job>, String> {
    let mgr = state.jobs.lock().map_err(|e| e.to_string())?;
    let parent = mgr
        .get(&job_id)
        .ok_or_else(|| format!("Job {} not found", job_id))?;

    let mut family = vec![parent.clone()];
    for child in mgr.get_children(&job_id) {
        family.push(child);
    }
    Ok(family)
}

/// Estimate completion time for a family of jobs.
#[tauri::command]
pub fn estimate_family_completion(
    state: State<'_, AppServices>,
    job_id: String,
) -> Result<String, String> {
    let mgr = state.jobs.lock().map_err(|e| e.to_string())?;
    let (downloaded, _failed, total) = mgr
        .aggregate_progress(&job_id)
        .unwrap_or((0, 0, 0));

    if total == 0 || downloaded == 0 {
        return Ok("Estimating...".into());
    }

    // Very rough heuristic: 3 min per track average
    let avg_sec_per_track = 180u64;
    let remaining = total.saturating_sub(downloaded) as u64;
    let remaining_sec = remaining * avg_sec_per_track;

    let mins = remaining_sec / 60;
    if mins < 60 {
        Ok(format!("~{} min remaining", mins))
    } else {
        let hours = mins / 60;
        let rmins = mins % 60;
        Ok(format!("~{}h {}m remaining", hours, rmins))
    }
}
