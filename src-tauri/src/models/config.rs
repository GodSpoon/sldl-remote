use serde::{Deserialize, Serialize};

/// The main application configuration. Persisted encrypted to disk.
///
/// This struct maps 1:1 with the UI settings form. Fields with
/// `serde(default)` survive schema migrations gracefully.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    /// Remote SSH host (IP or hostname).
    #[serde(default = "default_remote_host")]
    pub remote_host: String,
    /// Remote SSH username.
    #[serde(default = "default_remote_user")]
    pub remote_user: String,
    /// Remote SSH port.
    #[serde(default = "default_ssh_port")]
    pub ssh_port: u16,
    /// Path to SSH private key (empty = use default `~/.ssh/id_*`).
    #[serde(default)]
    pub ssh_key_path: String,
    /// Absolute path to sldl binary on remote host.
    #[serde(default = "default_sldl_path")]
    pub sldl_path: String,
    /// Base output directory on remote host.
    #[serde(default = "default_output_path")]
    pub output_path: String,

    // --- Soulseek ---
    #[serde(default)]
    pub soulseek_username: String,
    #[serde(default)]
    pub soulseek_password: String,

    // --- Spotify ---
    #[serde(default)]
    pub spotify_id: String,
    #[serde(default)]
    pub spotify_secret: String,

    // --- Quality ---
    #[serde(default = "default_pref_format")]
    pub pref_format: String,
    #[serde(default = "default_pref_min_bitrate")]
    pub pref_min_bitrate: u32,
    #[serde(default = "default_pref_max_bitrate")]
    pub pref_max_bitrate: u32,
    #[serde(default = "default_pref_max_samplerate")]
    pub pref_max_samplerate: u32,

    // --- Rate limiting ---
    #[serde(default = "default_searches_per_time")]
    pub searches_per_time: u32,
    #[serde(default = "default_searches_renew_time")]
    pub searches_renew_time: u32,
    #[serde(default)]
    pub fast_search: bool,

    // --- File naming ---
    #[serde(default = "default_name_format")]
    pub name_format: String,

    // --- Behaviour ---
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub skip_not_found: bool,
    #[serde(default = "default_concurrent_jobs")]
    pub concurrent_jobs: u32,

    // --- UI / App ---
    #[serde(default = "default_listen_port_base")]
    pub listen_port_base: u16,
}

fn default_remote_host() -> String { "192.168.70.12".into() }
fn default_remote_user() -> String { "root".into() }
fn default_ssh_port() -> u16 { 22 }
fn default_sldl_path() -> String { "/usr/local/bin/sldl".into() }
fn default_output_path() -> String { "/media/music".into() }
fn default_pref_format() -> String { "mp3".into() }
fn default_pref_min_bitrate() -> u32 { 320 }
fn default_pref_max_bitrate() -> u32 { 2500 }
fn default_pref_max_samplerate() -> u32 { 48000 }
fn default_searches_per_time() -> u32 { 20 }
fn default_searches_renew_time() -> u32 { 300 }
fn default_name_format() -> String { "{artist(/)album(/)track(. )title|filename}".into() }
fn default_max_retries() -> u32 { 10 }
fn default_concurrent_jobs() -> u32 { 20 }
fn default_listen_port_base() -> u16 { 49900 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            remote_host: default_remote_host(),
            remote_user: default_remote_user(),
            ssh_port: default_ssh_port(),
            ssh_key_path: String::new(),
            sldl_path: default_sldl_path(),
            output_path: default_output_path(),
            soulseek_username: String::new(),
            soulseek_password: String::new(),
            spotify_id: String::new(),
            spotify_secret: String::new(),
            pref_format: default_pref_format(),
            pref_min_bitrate: default_pref_min_bitrate(),
            pref_max_bitrate: default_pref_max_bitrate(),
            pref_max_samplerate: default_pref_max_samplerate(),
            searches_per_time: default_searches_per_time(),
            searches_renew_time: default_searches_renew_time(),
            fast_search: false,
            name_format: default_name_format(),
            max_retries: default_max_retries(),
            skip_not_found: true,
            concurrent_jobs: default_concurrent_jobs(),
            listen_port_base: default_listen_port_base(),
        }
    }
}

impl AppConfig {
    /// Build the per-job sldl.conf INI content.
    pub fn to_sldl_ini(&self, output_dir: &str) -> String {
        let fast = if self.fast_search { "true" } else { "false" };
        let skip = if self.skip_not_found { "true" } else { "false" };
        format!(
            "username = {}\n\
             password = {}\n\
             spotify-id = {}\n\
             spotify-secret = {}\n\
             path = {}\n\
             pref-format = {}\n\
             pref-min-bitrate = {}\n\
             pref-max-bitrate = {}\n\
             pref-max-samplerate = {}\n\
             name-format = {}\n\
             max-retries = {}\n\
             skip-not-found = {}\n\
             searches-per-time = {}\n\
             searches-renew-time = {}\n\
             fast-search = {}\n\
             concurrent-jobs = {}\n",
            self.soulseek_username,
            self.soulseek_password,
            self.spotify_id,
            self.spotify_secret,
            output_dir,
            self.pref_format,
            self.pref_min_bitrate,
            self.pref_max_bitrate,
            self.pref_max_samplerate,
            self.name_format,
            self.max_retries,
            skip,
            self.searches_per_time,
            self.searches_renew_time,
            fast,
            self.concurrent_jobs,
        )
    }

    /// SSH connection string like `root@192.168.70.12:22`.
    pub fn ssh_target(&self) -> String {
        if self.ssh_port == 22 {
            format!("{}@{}", self.remote_user, self.remote_host)
        } else {
            format!("{}@{}:{}", self.remote_user, self.remote_host, self.ssh_port)
        }
    }

    /// SSH command prefix (key + port options).
    pub fn ssh_opts(&self) -> Vec<String> {
        let mut opts = vec![
            "-o".to_string(), "ConnectTimeout=10".to_string(),
            "-o".to_string(), "StrictHostKeyChecking=accept-new".to_string(),
            "-o".to_string(), "BatchMode=yes".to_string(),
        ];
        if self.ssh_port != 22 {
            opts.push("-p".to_string());
            opts.push(self.ssh_port.to_string());
        }
        if !self.ssh_key_path.is_empty() {
            opts.push("-i".to_string());
            opts.push(self.ssh_key_path.clone());
        }
        opts
    }
}

/// Quality preset (simplifies UI for non-technical users).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum QualityPreset {
    Mp3_320,
    LossyFlexible,
    FlacPreferred,
    LosslessOnly,
}

impl QualityPreset {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Mp3_320 => "MP3 320kbps",
            Self::LossyFlexible => "Lossy (flexible)",
            Self::FlacPreferred => "FLAC preferred",
            Self::LosslessOnly => "Lossless only",
        }
    }

    pub fn apply(&self, config: &mut AppConfig) {
        match self {
            Self::Mp3_320 => {
                config.pref_format = "mp3".into();
                config.pref_min_bitrate = 320;
                config.pref_max_bitrate = 2500;
            }
            Self::LossyFlexible => {
                config.pref_format = "mp3,ogg,m4a,opus,aac".into();
                config.pref_min_bitrate = 192;
                config.pref_max_bitrate = 2500;
            }
            Self::FlacPreferred => {
                config.pref_format = "flac,mp3".into();
                config.pref_min_bitrate = 0;
                config.pref_max_bitrate = 2500;
            }
            Self::LosslessOnly => {
                config.pref_format = "flac,wav".into();
                config.pref_min_bitrate = 0;
                config.pref_max_bitrate = 0;
            }
        }
    }
}
