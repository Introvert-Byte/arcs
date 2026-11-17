//! AES-256-GCM encryption with Argon2id key derivation.
//! Password is never stored; only a random salt + nonce are kept inside the archive header.

use crate::error::{ArcsError, Result};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use rand::RngCore;
use zeroize::Zeroize;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

/// Header written at the beginning of an encrypted .arcs file
#[derive(Debug)]
pub struct CryptoHeader {
    pub salt: [u8; SALT_LEN],
    pub nonce: [u8; NONCE_LEN],
}

impl CryptoHeader {
    pub const SIZE: usize = SALT_LEN + NONCE_LEN;

    pub fn new() -> Self {
        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut salt);
        OsRng.fill_bytes(&mut nonce);
        Self { salt, nonce }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(Self::SIZE);
        v.extend_from_slice(&self.salt);
        v.extend_from_slice(&self.nonce);
        v
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < Self::SIZE {
            return Err(ArcsError::Crypto("truncated header".into()));
        }
        let mut salt = [0u8; SALT_LEN];
        let mut nonce = [0u8; NONCE_LEN];
        salt.copy_from_slice(&data[..SALT_LEN]);
        nonce.copy_from_slice(&data[SALT_LEN..SALT_LEN + NONCE_LEN]);
        Ok(Self { salt, nonce })
    }
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    let salt = SaltString::encode_b64(salt)
        .map_err(|e| ArcsError::Crypto(format!("salt encode: {e}")))?;
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| ArcsError::Crypto(format!("kdf: {e}")))?;
    let hash_bytes = hash.hash.ok_or_else(|| ArcsError::Crypto("no hash".into()))?;
    let mut key = [0u8; KEY_LEN];
    let bytes = hash_bytes.as_bytes();
    let len = bytes.len().min(KEY_LEN);
    key[..len].copy_from_slice(&bytes[..len]);
    Ok(key)
}

/// Encrypt plaintext bytes with the given password.
/// Returns: header_bytes || ciphertext
pub fn encrypt(plaintext: &[u8], password: &str) -> Result<Vec<u8>> {
    let header = CryptoHeader::new();
    let mut key = derive_key(password, &header.salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| ArcsError::Crypto(format!("cipher init: {e}")))?;
    let nonce = Nonce::from_slice(&header.nonce);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| ArcsError::Crypto("encryption failed".into()))?;
    key.zeroize();

    let mut out = header.to_bytes();
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Decrypt data that starts with a CryptoHeader.
pub fn decrypt(data: &[u8], password: &str) -> Result<Vec<u8>> {
    if data.len() < CryptoHeader::SIZE + 16 {
        // 16 = min GCM tag
        return Err(ArcsError::Crypto("data too short".into()));
    }
    let header = CryptoHeader::from_bytes(data)?;
    let ciphertext = &data[CryptoHeader::SIZE..];
    let mut key = derive_key(password, &header.salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| ArcsError::Crypto(format!("cipher init: {e}")))?;
    let nonce = Nonce::from_slice(&header.nonce);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| ArcsError::AuthenticationFailed)?;
    key.zeroize();
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let msg = b"hello arcs encryption";
        let enc = encrypt(msg, "test-pass").unwrap();
        let dec = decrypt(&enc, "test-pass").unwrap();
        assert_eq!(dec, msg);
    }

    #[test]
    fn bad_password() {
        let enc = encrypt(b"secret", "correct").unwrap();
        assert!(matches!(
            decrypt(&enc, "wrong"),
            Err(ArcsError::AuthenticationFailed)
        ));
    }
}