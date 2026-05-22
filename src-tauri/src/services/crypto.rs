//! AES-256-GCM encryption for the config file.
//!
//! The encryption key is a random 32-byte value stored alongside the
//! encrypted file. On Unix, the key file is chmod 600.
//!
//! Each config write generates a fresh random 96-bit nonce.
//!
//! SECURITY NOTE: This is NOT a password-protected vault. Anyone with
//! filesystem access to both the key and the encrypted file can decrypt.
//! It protects against casual snooping, not a determined attacker.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;

use crate::error::{AppError, AppResult};

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;

fn key_path() -> AppResult<std::path::PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "spoon", "sldl-remote")
        .ok_or_else(|| AppError::Config("Cannot determine config directory".into()))?;
    let dir = dirs.config_dir();
    std::fs::create_dir_all(dir)?;
    Ok(dir.join(".key"))
}

fn config_path() -> AppResult<std::path::PathBuf> {
    let dirs = directories::ProjectDirs::from("com", "spoon", "sldl-remote")
        .ok_or_else(|| AppError::Config("Cannot determine config directory".into()))?;
    let dir = dirs.config_dir();
    std::fs::create_dir_all(dir)?;
    Ok(dir.join("config.enc"))
}

/// Load or create the encryption key.
pub fn load_or_create_key() -> AppResult<[u8; KEY_LEN]> {
    let path = key_path()?;
    if path.exists() {
        let bytes = std::fs::read(&path)?;
        if bytes.len() == KEY_LEN {
            let mut key = [0u8; KEY_LEN];
            key.copy_from_slice(&bytes);
            return Ok(key);
        }
    }
    let mut key = [0u8; KEY_LEN];
    rand::thread_rng().fill_bytes(&mut key);
    std::fs::write(&path, &key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(key)
}

/// Decrypt the config file.
pub fn decrypt() -> AppResult<Vec<u8>> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(vec![]);
    }
    let key = load_or_create_key()?;
    let cipher = Aes256Gcm::new(aes_gcm::aead::generic_array::GenericArray::from_slice(&key));
    let encrypted = std::fs::read(&path)?;
    if encrypted.len() < NONCE_LEN {
        return Err(AppError::Crypto("Config file too short (truncated)".into()));
    }
    let nonce = Nonce::from_slice(&encrypted[..NONCE_LEN]);
    let plaintext = cipher
        .decrypt(nonce, &encrypted[NONCE_LEN..])
        .map_err(|e| AppError::Crypto(format!("Decryption failed: {:?}", e)))?;
    Ok(plaintext)
}

/// Encrypt and write the config file.
pub fn encrypt(plaintext: &[u8]) -> AppResult<()> {
    let path = config_path()?;
    let key = load_or_create_key()?;
    let cipher = Aes256Gcm::new(aes_gcm::aead::generic_array::GenericArray::from_slice(&key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| AppError::Crypto(format!("Encryption failed: {:?}", e)))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    std::fs::write(&path, &out)?;
    Ok(())
}

/// Export the encrypted config to a portable file (for backup/migration).
pub fn export_to(path: &std::path::Path) -> AppResult<()> {
    let src = config_path()?;
    if !src.exists() {
        return Err(AppError::Config("No config to export".into()));
    }
    std::fs::copy(&src, path)?;
    Ok(())
}

/// Import an encrypted config from a portable file.
pub fn import_from(path: &std::path::Path) -> AppResult<()> {
    let dst = config_path()?;
    std::fs::copy(path, &dst)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_fresh_nonce() {
        let plaintext = b"hello world this is a test config";
        encrypt(plaintext).unwrap();
        let decrypted = decrypt().unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);

        // fresh nonce each write
        let first = std::fs::read(&config_path().unwrap()).unwrap();
        encrypt(plaintext).unwrap();
        let second = std::fs::read(&config_path().unwrap()).unwrap();
        assert_ne!(first, second, "Nonce must be fresh each write");
    }
}
