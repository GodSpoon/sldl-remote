//! Sldl Remote — Core library
//!
//! Architecture:
//! - `models`: Shared data types (Config, Job, etc.)
//! - `services`: Business logic (crypto, SSH, config persistence, job manager)
//! - `commands`: Tauri IPC command handlers (thin layer over services)
//! - `error`: Application-wide error types

pub mod commands;
pub mod error;
pub mod models;
pub mod services;

use tauri::generate_handler;

/// Build the Tauri application with all commands wired up.
pub fn run() {
    tauri::Builder::default()
        .manage(services::job_manager::JobManager::default())
        .invoke_handler(generate_handler![
            commands::config::get_config,
            commands::config::save_config,
            commands::config::test_connection,
            commands::jobs::start_job,
            commands::jobs::list_jobs,
            commands::jobs::get_job,
            commands::jobs::refresh_job_status,
            commands::jobs::get_job_logs,
            commands::jobs::stop_job,
            commands::jobs::clear_completed,
            commands::library::list_downloads,
            commands::library::get_download_stats,
            commands::album_mode::discover_albums_from_playlist,
            commands::album_mode::start_associated_album_jobs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_test() {
        // Verify modules compile and types are coherent.
        let _ = models::AppConfig::default();
        let _ = models::Job::default();
    }
}
