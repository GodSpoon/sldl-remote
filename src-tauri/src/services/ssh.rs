//! SSH remote execution layer.
//!
//! All remote commands go through `ssh::run(host, cmd)`.
//! SSH options are derived from the loaded AppConfig (keys, ports, timeouts).
//!
//! DESIGN: We spawn the `ssh` binary rather than using an async SSH library
//! to keep the dependency tree small and avoid vendoring libssl / libcrypto.
//! The tradeoff is we need the `ssh` CLI installed (true on macOS/Linux,
//! Windows users need OpenSSH or WSL).

use tokio::process::Command;

use crate::error::{AppError, AppResult};
use crate::models::AppConfig;

/// Default SSH timeout (seconds) for connection establishment.
const CONNECT_TIMEOUT: u32 = 10;

/// Build the full ssh command with all options.
pub fn build_ssh_command(config: &AppConfig, remote_cmd: &str) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.arg("-o").arg(format!("ConnectTimeout={}", CONNECT_TIMEOUT));
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg("-o").arg("BatchMode=yes");
    cmd.arg("-o").arg("ServerAliveInterval=60");
    cmd.arg("-o").arg("ServerAliveCountMax=3");

    if config.ssh_port != 22 {
        cmd.arg("-p").arg(config.ssh_port.to_string());
    }
    if !config.ssh_key_path.is_empty() {
        cmd.arg("-i").arg(&config.ssh_key_path);
    }

    cmd.arg(config.ssh_target());
    cmd.arg(remote_cmd);
    cmd
}

/// Run a single command on the remote host and return stdout.
/// Stderr is included in the error message on non-zero exit.
pub async fn run(host: &str, remote_cmd: &str) -> AppResult<String> {
    // Load config to get SSH options
    let config = crate::services::config_manager::ConfigManager::load_fresh()
        .unwrap_or_default();

    if config.ssh_target() != host && config.remote_host != host {
        // Host mismatch — caller passed a raw host string without config context.
        // Fall back to minimal SSH options.
        let mut cmd = Command::new("ssh");
        cmd.arg("-o").arg(format!("ConnectTimeout={}", CONNECT_TIMEOUT));
        cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
        cmd.arg("-o").arg("BatchMode=yes");
        cmd.arg(host).arg(remote_cmd);
        return exec(cmd, host, remote_cmd).await;
    }

    let cmd = build_ssh_command(&config, remote_cmd);
    exec(cmd, host, remote_cmd).await
}

/// Run a command with a specific config (for testing / overrides).
pub async fn run_with_config(config: &AppConfig, remote_cmd: &str) -> AppResult<String> {
    let cmd = build_ssh_command(config, remote_cmd);
    exec(cmd, &config.ssh_target(), remote_cmd).await
}

async fn exec(mut cmd: Command, host: &str, remote_cmd: &str) -> AppResult<String> {
    let output = cmd
        .output()
        .await
        .map_err(|e| AppError::Ssh {
            host: host.into(),
            cmd: remote_cmd.into(),
            cause: format!("Failed to spawn ssh: {}", e),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        // Detect ban patterns in stderr
        if let Some((msg, _)) = super::detect_ban(&stderr) {
            return Err(AppError::SoulseekBanned {
                ip: None,
                message: msg,
            });
        }
        return Err(AppError::Ssh {
            host: host.into(),
            cmd: remote_cmd.into(),
            cause: format!("Exit {} — stderr: {}", output.status, stderr.trim()),
        });
    }

    Ok(stdout)
}

/// Run a command and stream stdout/stderr line-by-line.
/// Returns a channel receiver for log lines.
pub async fn stream(
    config: &AppConfig,
    remote_cmd: &str,
) -> AppResult<tokio::sync::mpsc::Receiver<String>> {
    let mut cmd = build_ssh_command(config, remote_cmd);
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| AppError::Ssh {
            host: config.ssh_target(),
            cmd: remote_cmd.into(),
            cause: format!("Failed to spawn ssh: {}", e),
        })?;

    let stdout = child.stdout.take().ok_or_else(|| AppError::Ssh {
        host: config.ssh_target(),
        cmd: remote_cmd.into(),
        cause: "Failed to capture stdout".into(),
    })?;

    let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);

    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = tx.send(line).await;
        }
    });

    Ok(rx)
}

/// Write a file on the remote host via SSH heredoc.
pub async fn write_remote_file(
    config: &AppConfig,
    remote_path: &str,
    content: &str,
) -> AppResult<()> {
    // Escape single quotes in content for the heredoc
    let safe = content.replace("'", "'\"'\"'");
    let cmd = format!(
        "cat > '{}' << 'HEREDOC_EOF'\n{}\nHEREDOC_EOF",
        remote_path, safe
    );
    run_with_config(config, &cmd).await?;
    Ok(())
}

/// Delete a remote file.
pub async fn delete_remote_file(config: &AppConfig, remote_path: &str) -> AppResult<()> {
    run_with_config(config, &format!("rm -f '{}'", remote_path)).await?;
    Ok(())
}

/// Check if a process is still running on the remote host.
pub async fn is_process_running(config: &AppConfig, pid: u32) -> AppResult<bool> {
    let out = run_with_config(config, &format!("kill -0 {} 2>/dev/null && echo yes || echo no", pid)).await?;
    Ok(out.trim() == "yes")
}

/// Send a signal to a remote process.
pub async fn signal_process(config: &AppConfig, pid: u32, sig: &str) -> AppResult<()> {
    run_with_config(config, &format!("kill -{} {} 2>/dev/null || true", sig, pid)).await?;
    Ok(())
}

/// Get system info from the remote host.
pub async fn get_system_info(config: &AppConfig) -> AppResult<crate::models::RemoteHostInfo> {
    let _host = config.ssh_target();
    let uname = run_with_config(config, "uname -sm").await?;
    let hostname = run_with_config(config, "hostname").await?;
    let version = run_with_config(config, &format!("{} --version 2>&1 || echo 'unknown'", config.sldl_path)).await?;
    let df = run_with_config(config, "df -BG . | tail -1 | awk '{print $2,$4}'").await?;
    let mem = run_with_config(config, "free -m | awk 'NR==2{print $2}'").await.unwrap_or_default();
    let cpu = run_with_config(config, "nproc").await.unwrap_or_default();

    let disk_parts: Vec<&str> = df.trim().split_whitespace().collect();
    let (total, free) = if disk_parts.len() >= 2 {
        (
            disk_parts[0].trim_end_matches('G').parse::<f64>().unwrap_or(0.0),
            disk_parts[1].trim_end_matches('G').parse::<f64>().unwrap_or(0.0),
        )
    } else {
        (0.0, 0.0)
    };

    Ok(crate::models::RemoteHostInfo {
        hostname: hostname.trim().to_string(),
        os: uname.trim().to_string(),
        arch: "unknown".into(),
        sldl_version: version.trim().to_string(),
        disk_total_gb: total,
        disk_free_gb: free,
        cpu_count: cpu.trim().parse().unwrap_or(1),
        memory_mb: mem.trim().parse().unwrap_or(0),
        last_seen: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_command_building() {
        let mut cfg = AppConfig::default();
        cfg.ssh_port = 2222;
        cfg.ssh_key_path = "/path/to/key".into();
        let cmd = build_ssh_command(&cfg, "echo hello");
        // Just verify it doesn't panic; full exec testing requires a real SSH host.
        let _ = cmd;
    }
}
