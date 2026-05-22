use crate::models::{LibraryFile, LibraryStats, Paginated};

/// List files in the remote music library.
#[tauri::command]
pub async fn list_downloads(
    subdir: Option<String>,
    search: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
) -> Result<Paginated<LibraryFile>, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    let base_path = if let Some(sub) = subdir {
        format!("{}/{}", config.output_path, sub)
    } else {
        config.output_path.clone()
    };

    let search_filter = search
        .map(|s| format!(" | grep -i '{}'", s))
        .unwrap_or_default();
    let cmd = format!(
        "find '{}' -maxdepth 4 -type f {} | sort",
        base_path, search_filter
    );

    let out = crate::services::ssh::run(&host, &cmd)
        .await
        .map_err(|e| e.to_string())?;

    let files: Vec<LibraryFile> = out
        .lines()
        .filter(|l| !l.is_empty())
        .map(|path| {
            let relative = path
                .strip_prefix(&format!("{}/", config.output_path))
                .unwrap_or(path)
                .to_string();
            LibraryFile {
                path: path.to_string(),
                relative_path: relative,
                size_bytes: 0,
                modified: String::new(),
            }
        })
        .collect();

    let p = page.unwrap_or(0);
    let pp = per_page.unwrap_or(100);
    Ok(Paginated::new(files, p, pp))
}

/// Get library statistics (file counts, sizes, by category).
#[tauri::command]
pub async fn get_download_stats() -> Result<LibraryStats, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    let mut by_category = vec![];
    for category in ["playlists", "albums", "artists", "tracks"] {
        let path = format!("{}/{}", config.output_path, category);
        let cmd = format!(
            "find '{}' -maxdepth 3 -type f 2>/dev/null | wc -l; du -sm '{}' 2>/dev/null | awk '{{print $1}}'",
            path, path
        );
        match crate::services::ssh::run(&host, &cmd).await {
            Ok(out) => {
                let lines: Vec<&str> = out.lines().collect();
                let count = lines
                    .get(0)
                    .and_then(|l| l.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let size = lines
                    .get(1)
                    .and_then(|l| l.trim().parse::<u64>().ok())
                    .unwrap_or(0);
                by_category.push(crate::models::CategoryStat {
                    name: category.into(),
                    file_count: count,
                    size_mb: size,
                });
            }
            Err(_) => {
                by_category.push(crate::models::CategoryStat {
                    name: category.into(),
                    file_count: 0,
                    size_mb: 0,
                });
            }
        }
    }

    let total_files: usize = by_category.iter().map(|c| c.file_count).sum();
    let total_size_mb: u64 = by_category.iter().map(|c| c.size_mb).sum();

    // Count unique artists (top-level dirs under artists/)
    let artist_count = crate::services::ssh::run(
        &host,
        &format!(
            "find '{}/artists' -maxdepth 1 -mindepth 1 -type d 2>/dev/null | wc -l",
            config.output_path
        ),
    )
    .await
    .ok()
    .and_then(|o| o.trim().parse::<usize>().ok())
    .unwrap_or(0);

    // Count unique albums (all album-level dirs)
    let album_count = crate::services::ssh::run(
        &host,
        &format!(
            "find '{}' -mindepth 2 -maxdepth 3 -type d 2>/dev/null | wc -l",
            config.output_path
        ),
    )
    .await
    .ok()
    .and_then(|o| o.trim().parse::<usize>().ok())
    .unwrap_or(0);

    Ok(LibraryStats {
        total_files,
        total_artists: artist_count,
        total_albums: album_count,
        total_size_mb,
        by_category,
    })
}

/// Get details for a specific file.
#[tauri::command]
pub async fn get_file_info(remote_path: String) -> Result<LibraryFile, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    let cmd = format!(
        "ls -l --time-style=+'%Y-%m-%d %H:%M:%S' '{}' | awk '{{print $5, $6}}'",
        remote_path
    );
    let out = crate::services::ssh::run(&host, &cmd)
        .await
        .map_err(|e| e.to_string())?;
    let parts: Vec<&str> = out.trim().split_whitespace().collect();

    let size = parts
        .get(0)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let modified = parts.get(1..).map(|p| p.join(" ")).unwrap_or_default();

    let relative = remote_path
        .strip_prefix(&format!("{}/", config.output_path))
        .unwrap_or(&remote_path)
        .to_string();

    Ok(LibraryFile {
        path: remote_path,
        relative_path: relative,
        size_bytes: size,
        modified,
    })
}

/// Delete a file from the remote library.
#[tauri::command]
pub async fn delete_file(remote_path: String) -> Result<(), String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    crate::services::ssh::run(&host, &format!("rm -f '{}'", remote_path))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Reveal a file in the remote filesystem (macOS: open Finder, Linux: xdg-open).
/// Returns the command that would be run (since we can't open remote GUI).
#[tauri::command]
pub async fn reveal_file(remote_path: String) -> Result<String, String> {
    Ok(format!(
        "On the remote host: xdg-open '{}' || open '{}'",
        remote_path, remote_path
    ))
}

/// Play a file via remote mpv/vlc (returns the command).
#[tauri::command]
pub async fn play_file(remote_path: String) -> Result<String, String> {
    Ok(format!(
        "On the remote host: mpv '{}' || vlc '{}' || ffplay '{}'",
        remote_path, remote_path, remote_path
    ))
}

/// Get recently added files (last N hours).
#[tauri::command]
pub async fn get_recent_files(hours: u64) -> Result<Vec<LibraryFile>, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    let cmd = format!(
        "find '{}' -type f -mmin -{} -print 2>/dev/null | sort",
        config.output_path,
        hours * 60
    );
    let out = crate::services::ssh::run(&host, &cmd)
        .await
        .map_err(|e| e.to_string())?;

    let files: Vec<LibraryFile> = out
        .lines()
        .filter(|l| !l.is_empty())
        .map(|path| {
            let relative = path
                .strip_prefix(&format!("{}/", config.output_path))
                .unwrap_or(path)
                .to_string();
            LibraryFile {
                path: path.to_string(),
                relative_path: relative,
                size_bytes: 0,
                modified: String::new(),
            }
        })
        .collect();

    Ok(files)
}

/// Search the library by artist/album/title.
#[tauri::command]
pub async fn search_library(query: String) -> Result<Vec<LibraryFile>, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    let cmd = format!(
        "find '{}' -type f -iname '*{}*' 2>/dev/null | sort | head -200",
        config.output_path,
        query.replace("'", "'\"'\"'")
    );
    let out = crate::services::ssh::run(&host, &cmd)
        .await
        .map_err(|e| e.to_string())?;

    let files: Vec<LibraryFile> = out
        .lines()
        .filter(|l| !l.is_empty())
        .map(|path| {
            let relative = path
                .strip_prefix(&format!("{}/", config.output_path))
                .unwrap_or(path)
                .to_string();
            LibraryFile {
                path: path.to_string(),
                relative_path: relative,
                size_bytes: 0,
                modified: String::new(),
            }
        })
        .collect();

    Ok(files)
}

/// List top-level categories (playlists, albums, artists, tracks).
#[tauri::command]
pub async fn list_categories() -> Result<Vec<String>, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    let cmd = format!("ls -1 '{}' 2>/dev/null | sort", config.output_path);
    let out = crate::services::ssh::run(&host, &cmd)
        .await
        .map_err(|e| e.to_string())?;

    Ok(out
        .lines()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

/// List subdirectories inside a category (e.g. playlists/playlist-id).
#[tauri::command]
pub async fn list_subdirs(category: String) -> Result<Vec<String>, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();

    let path = format!("{}/{}", config.output_path, category);
    let cmd = format!(
        "find '{}' -maxdepth 2 -mindepth 1 -type d 2>/dev/null | sort",
        path
    );
    let out = crate::services::ssh::run(&host, &cmd)
        .await
        .map_err(|e| e.to_string())?;

    Ok(out
        .lines()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .collect())
}
