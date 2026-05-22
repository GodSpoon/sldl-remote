use tauri::State;

use crate::commands::ok;
use crate::models::{AppConfig, ConnectionStatus};
use crate::services::AppServices;

#[tauri::command]
pub fn get_config(state: State<AppServices>) -> Result<AppConfig, String> {
    let mut mgr = state.config.lock().map_err(|e| e.to_string())?;
    ok(mgr.load())
}

#[tauri::command]
pub fn save_config(state: State<AppServices>, config: AppConfig) -> Result<(), String> {
    let mut mgr = state.config.lock().map_err(|e| e.to_string())?;
    ok(mgr.save(&config))
}

#[tauri::command]
pub async fn test_connection() -> Result<ConnectionStatus, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;

    let host = config.ssh_target();

    let uname = match crate::services::ssh::run(&host, "uname -sm").await {
        Ok(out) => out.trim().to_string(),
        Err(e) => {
            return Ok(ConnectionStatus {
                ok: false,
                host: host.clone(),
                os_info: String::new(),
                disk_free_gb: 0.0,
                message: format!("SSH failed: {}", e),
            });
        }
    };

    let (total, free) = match crate::services::ssh::run(
        &host,
        &format!(
            "df -BG '{}' | tail -1 | awk '{{print $2,$4}}'",
            config.output_path
        ),
    )
    .await
    {
        Ok(out) => {
            let parts: Vec<&str> = out.trim().split_whitespace().collect();
            if parts.len() >= 2 {
                (
                    parts[0].trim_end_matches('G').parse::<f64>().unwrap_or(0.0),
                    parts[1].trim_end_matches('G').parse::<f64>().unwrap_or(0.0),
                )
            } else {
                (0.0, 0.0)
            }
        }
        Err(_) => (0.0, 0.0),
    };

    Ok(ConnectionStatus {
        ok: true,
        host,
        os_info: uname.clone(),
        disk_free_gb: free,
        message: format!("{} — {:.1} GB free / {:.1} GB total", uname, free, total),
    })
}

#[tauri::command]
pub fn reset_config(state: State<AppServices>) -> Result<AppConfig, String> {
    let mut mgr = state.config.lock().map_err(|e| e.to_string())?;
    let default = AppConfig::default();
    ok(mgr.save(&default))?;
    Ok(default)
}

#[tauri::command]
pub fn apply_quality_preset(
    state: State<AppServices>,
    preset: crate::models::QualityPreset,
) -> Result<AppConfig, String> {
    let mut mgr = state.config.lock().map_err(|e| e.to_string())?;
    let mut config = mgr.load().map_err(|e| e.to_string())?;
    preset.apply(&mut config);
    mgr.save(&config).map_err(|e| e.to_string())?;
    Ok(config)
}

#[tauri::command]
pub async fn get_sldl_help() -> Result<String, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    crate::services::ssh::run(
        &config.ssh_target(),
        &format!("{} --help", config.sldl_path),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_config_dir() -> Result<String, String> {
    let dirs = directories::ProjectDirs::from("com", "spoon", "sldl-remote")
        .ok_or("Cannot determine config directory")?;
    Ok(dirs.config_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub async fn verify_remote_paths() -> Result<Vec<String>, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();
    let mut results = vec![];

    // Check sldl binary
    match crate::services::ssh::run(
        &host,
        &format!("test -x '{}' && echo ok || echo missing", config.sldl_path),
    )
    .await
    {
        Ok(out) => {
            if out.trim() == "ok" {
                results.push(format!("✓ sldl binary at {}", config.sldl_path));
            } else {
                results.push(format!("✗ sldl binary NOT FOUND at {}", config.sldl_path));
            }
        }
        Err(e) => results.push(format!("✗ Cannot check sldl: {}", e)),
    }

    // Check output dir
    match crate::services::ssh::run(
        &host,
        &format!(
            "test -d '{}' && echo ok || echo missing",
            config.output_path
        ),
    )
    .await
    {
        Ok(out) => {
            if out.trim() == "ok" {
                results.push(format!("✓ Output dir at {}", config.output_path));
            } else {
                results.push(format!(
                    "⚠ Output dir missing at {} (will be created)",
                    config.output_path
                ));
            }
        }
        Err(e) => results.push(format!("✗ Cannot check output dir: {}", e)),
    }

    // Check ssh key
    if !config.ssh_key_path.is_empty() {
        match std::fs::metadata(&config.ssh_key_path) {
            Ok(_) => results.push(format!("✓ SSH key at {}", config.ssh_key_path)),
            Err(_) => results.push(format!("✗ SSH key NOT FOUND at {}", config.ssh_key_path)),
        }
    } else {
        results.push("ℹ Using default SSH key (agent or ~/.ssh)".into());
    }

    Ok(results)
}

#[tauri::command]
pub async fn ensure_output_dirs() -> Result<(), String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();
    crate::services::ssh::run(
        &host,
        &format!(
            "mkdir -p {}/playlists {}/albums {}/artists {}/tracks",
            config.output_path, config.output_path, config.output_path, config.output_path
        ),
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn setup_remote_sldl() -> Result<Vec<String>, String> {
    let config =
        crate::services::config_manager::ConfigManager::load_fresh().map_err(|e| e.to_string())?;
    let host = config.ssh_target();
    let mut log = vec![];

    // Check if sldl already exists
    match crate::services::ssh::run(
        &host,
        &format!("{} --version 2>/dev/null || echo missing", config.sldl_path),
    )
    .await
    {
        Ok(out) => {
            if out.trim() != "missing" {
                log.push(format!("sldl already installed: {}", out.trim()));
                return Ok(log);
            }
        }
        Err(_) => {}
    }

    log.push("sldl not found, attempting install...".into());

    // Download latest release
    let dl = format!(
        "cd /tmp && curl -sL -o sldl.zip 'https://github.com/fiso64/sldl/releases/download/v2.6.0/sldl_linux-x64.zip' \
        && unzip -o sldl.zip && mv sldl '{}' && chmod +x '{}' && rm -f sldl.zip sldl.pdb LICENSE",
        config.sldl_path, config.sldl_path
    );

    match crate::services::ssh::run(&host, &dl).await {
        Ok(_) => {
            // Verify
            match crate::services::ssh::run(&host, &format!("{} --version", config.sldl_path)).await
            {
                Ok(ver) => log.push(format!("✓ Installed: {}", ver.trim())),
                Err(e) => log.push(format!("✗ Install verification failed: {}", e)),
            }
        }
        Err(e) => log.push(format!("✗ Install failed: {}", e)),
    }

    Ok(log)
}
