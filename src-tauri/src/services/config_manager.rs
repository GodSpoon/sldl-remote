//! Config persistence manager.
//!
//! Loads/saves the AppConfig struct encrypted to disk using AES-256-GCM.
//! Also provides export/import for backup/migration.

use crate::error::AppResult;
use crate::models::AppConfig;

pub struct ConfigManager {
    cached: Option<AppConfig>,
}

impl Default for ConfigManager {
    fn default() -> Self {
        Self { cached: None }
    }
}

impl ConfigManager {
    /// Load config from disk (with caching).
    pub fn load(&mut self) -> AppResult<AppConfig> {
        if let Some(ref cfg) = self.cached {
            return Ok(cfg.clone());
        }
        let plaintext = super::crypto::decrypt()?;
        if plaintext.is_empty() {
            let cfg = AppConfig::default();
            self.cached = Some(cfg.clone());
            return Ok(cfg);
        }
        let cfg: AppConfig = serde_json::from_slice(&plaintext)?;
        self.cached = Some(cfg.clone());
        Ok(cfg)
    }

    /// Save config to disk (and update cache).
    pub fn save(&mut self, config: &AppConfig) -> AppResult<()> {
        let plaintext = serde_json::to_vec(config)?;
        super::crypto::encrypt(&plaintext)?;
        self.cached = Some(config.clone());
        Ok(())
    }

    /// Clear the in-memory cache (forces next load to read from disk).
    pub fn invalidate_cache(&mut self) {
        self.cached = None;
    }

    /// Export encrypted config to a file path.
    pub fn export(&self, path: &std::path::Path) -> AppResult<()> {
        super::crypto::export_to(path)
    }

    /// Import encrypted config from a file path.
    pub fn import(&mut self, path: &std::path::Path) -> AppResult<()> {
        super::crypto::import_from(path)?;
        self.invalidate_cache();
        let _ = self.load()?;
        Ok(())
    }

    /// Convenience: load without mutable access (reads from disk every time).
    pub fn load_fresh() -> AppResult<AppConfig> {
        let plaintext = super::crypto::decrypt()?;
        if plaintext.is_empty() {
            Ok(AppConfig::default())
        } else {
            Ok(serde_json::from_slice(&plaintext)?)
        }
    }
}
