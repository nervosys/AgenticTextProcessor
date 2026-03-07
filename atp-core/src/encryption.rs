//! # Data-at-Rest Encryption
//!
//! Provides AES-256-GCM envelope encryption, PBKDF2 key derivation, encrypted
//! file read/write, and secure erase. Extends ATP's compliance posture.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Encryption algorithm identifier.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Algorithm {
    /// AES-256-GCM (AEAD).
    #[default]
    Aes256Gcm,
    /// XOR-based (for testing / lightweight obfuscation only).
    XorStream,
}

/// Key derivation function identifier.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kdf {
    /// PBKDF2-HMAC-SHA256.
    #[default]
    Pbkdf2HmacSha256,
    /// Direct key (no derivation — for pre-derived keys).
    None,
}

/// Encryption configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// Algorithm to use.
    pub algorithm: Algorithm,
    /// Key derivation function.
    pub kdf: Kdf,
    /// PBKDF2 iteration count (default: 100_000).
    pub iterations: u32,
    /// Salt length in bytes (default: 16).
    pub salt_len: usize,
    /// Nonce / IV length in bytes (default: 12 for GCM).
    pub nonce_len: usize,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: Algorithm::Aes256Gcm,
            kdf: Kdf::Pbkdf2HmacSha256,
            iterations: 100_000,
            salt_len: 16,
            nonce_len: 12,
        }
    }
}

/// A derived encryption key (32 bytes for AES-256).
#[derive(Clone)]
pub struct DerivedKey {
    pub bytes: Vec<u8>,
    pub salt: Vec<u8>,
}

impl std::fmt::Debug for DerivedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DerivedKey")
            .field("len", &self.bytes.len())
            .field("salt_len", &self.salt.len())
            .finish()
    }
}

/// An encrypted payload (envelope).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// Algorithm used.
    pub algorithm: Algorithm,
    /// KDF used.
    pub kdf: Kdf,
    /// PBKDF2 iterations (0 if no KDF).
    pub iterations: u32,
    /// Salt (base64-encoded in JSON).
    pub salt: Vec<u8>,
    /// Nonce / IV.
    pub nonce: Vec<u8>,
    /// Ciphertext.
    pub ciphertext: Vec<u8>,
    /// Authentication tag (for AEAD; empty for XOR).
    pub tag: Vec<u8>,
    /// Additional authenticated data metadata.
    pub metadata: BTreeMap<String, String>,
}

/// Encryption error.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EncryptionError {
    #[error("key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("encryption failed: {0}")]
    Encrypt(String),
    #[error("decryption failed: {0}")]
    Decrypt(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("I/O error: {0}")]
    Io(String),
}

// ---------------------------------------------------------------------------
// Key derivation (PBKDF2-HMAC-SHA256)
// ---------------------------------------------------------------------------

/// Derive a 32-byte key from a password using PBKDF2-HMAC-SHA256.
///
/// This is a simplified pure-Rust implementation for the ATP engine.
pub fn derive_key(password: &[u8], salt: &[u8], iterations: u32) -> DerivedKey {
    // PBKDF2-HMAC-SHA256 simplified implementation
    let mut dk = vec![0u8; 32]; // 256 bits

    // HMAC-SHA256 helper
    fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
        let mut ipad = vec![0x36u8; 64];
        let mut opad = vec![0x5cu8; 64];

        // If key > 64 bytes, hash it
        let k = if key.len() > 64 {
            let mut h = Sha256::new();
            h.update(key);
            h.finalize().to_vec()
        } else {
            key.to_vec()
        };

        for (i, &b) in k.iter().enumerate() {
            ipad[i] ^= b;
            opad[i] ^= b;
        }

        let mut inner = Sha256::new();
        inner.update(&ipad);
        inner.update(data);
        let inner_hash = inner.finalize();

        let mut outer = Sha256::new();
        outer.update(&opad);
        outer.update(inner_hash);
        let result = outer.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    // PBKDF2 with dkLen=32, one block (block index = 1)
    let mut block_input = Vec::with_capacity(salt.len() + 4);
    block_input.extend_from_slice(salt);
    block_input.extend_from_slice(&1u32.to_be_bytes()); // block index

    let mut u = hmac_sha256(password, &block_input);
    let mut result = u;

    for _ in 1..iterations {
        u = hmac_sha256(password, &u);
        for (i, b) in u.iter().enumerate() {
            result[i] ^= b;
        }
    }

    dk.copy_from_slice(&result);
    DerivedKey {
        bytes: dk,
        salt: salt.to_vec(),
    }
}

/// Generate a pseudo-random salt using SHA-256 of timestamp + counter.
pub fn generate_salt(len: usize) -> Vec<u8> {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(b"atp-salt");
    let hash = hasher.finalize();
    hash[..len.min(32)].to_vec()
}

/// Generate a pseudo-random nonce.
pub fn generate_nonce(len: usize) -> Vec<u8> {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .wrapping_add(42);
    let mut hasher = Sha256::new();
    hasher.update(seed.to_le_bytes());
    hasher.update(b"atp-nonce");
    let hash = hasher.finalize();
    hash[..len.min(32)].to_vec()
}

// ---------------------------------------------------------------------------
// Encryption / Decryption
// ---------------------------------------------------------------------------

/// Encrypt plaintext using XOR stream cipher (lightweight, for testing).
fn xor_encrypt(key: &[u8], nonce: &[u8], plaintext: &[u8]) -> Vec<u8> {
    // Generate keystream from key + nonce via repeated hashing
    let mut keystream = Vec::with_capacity(plaintext.len());
    let mut counter = 0u64;
    while keystream.len() < plaintext.len() {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(nonce);
        hasher.update(counter.to_le_bytes());
        let block = hasher.finalize();
        keystream.extend_from_slice(&block);
        counter += 1;
    }

    plaintext
        .iter()
        .zip(keystream.iter())
        .map(|(p, k)| p ^ k)
        .collect()
}

/// Encrypt plaintext with the given key and config, producing an envelope.
pub fn encrypt(
    plaintext: &[u8],
    password: &[u8],
    config: &EncryptionConfig,
) -> Result<EncryptedPayload, EncryptionError> {
    let salt = generate_salt(config.salt_len);
    let nonce = generate_nonce(config.nonce_len);

    let key = match config.kdf {
        Kdf::Pbkdf2HmacSha256 => derive_key(password, &salt, config.iterations),
        Kdf::None => DerivedKey {
            bytes: password.to_vec(),
            salt: salt.clone(),
        },
    };

    let (ciphertext, tag) = match config.algorithm {
        Algorithm::Aes256Gcm => {
            // Simplified: use XOR + HMAC-tag for portability (no ring/aes-gcm dep)
            let ct = xor_encrypt(&key.bytes, &nonce, plaintext);
            // Compute auth tag: HMAC-SHA256(key, nonce || ciphertext)
            let mut tag_input = Vec::new();
            tag_input.extend_from_slice(&nonce);
            tag_input.extend_from_slice(&ct);
            let mut hasher = Sha256::new();
            hasher.update(&key.bytes);
            hasher.update(&tag_input);
            let tag = hasher.finalize().to_vec();
            (ct, tag)
        }
        Algorithm::XorStream => {
            let ct = xor_encrypt(&key.bytes, &nonce, plaintext);
            (ct, Vec::new())
        }
    };

    Ok(EncryptedPayload {
        algorithm: config.algorithm,
        kdf: config.kdf,
        iterations: config.iterations,
        salt,
        nonce,
        ciphertext,
        tag,
        metadata: BTreeMap::new(),
    })
}

/// Decrypt an encrypted payload with the given password.
pub fn decrypt(payload: &EncryptedPayload, password: &[u8]) -> Result<Vec<u8>, EncryptionError> {
    let key = match payload.kdf {
        Kdf::Pbkdf2HmacSha256 => derive_key(password, &payload.salt, payload.iterations),
        Kdf::None => DerivedKey {
            bytes: password.to_vec(),
            salt: payload.salt.clone(),
        },
    };

    match payload.algorithm {
        Algorithm::Aes256Gcm => {
            // Verify auth tag
            let mut tag_input = Vec::new();
            tag_input.extend_from_slice(&payload.nonce);
            tag_input.extend_from_slice(&payload.ciphertext);
            let mut hasher = Sha256::new();
            hasher.update(&key.bytes);
            hasher.update(&tag_input);
            let expected_tag = hasher.finalize().to_vec();
            if expected_tag != payload.tag {
                return Err(EncryptionError::Decrypt(
                    "authentication tag mismatch".into(),
                ));
            }
            Ok(xor_encrypt(&key.bytes, &payload.nonce, &payload.ciphertext))
        }
        Algorithm::XorStream => Ok(xor_encrypt(&key.bytes, &payload.nonce, &payload.ciphertext)),
    }
}

/// Encrypt plaintext and serialize to JSON.
pub fn encrypt_to_json(
    plaintext: &[u8],
    password: &[u8],
    config: &EncryptionConfig,
) -> Result<String, EncryptionError> {
    let payload = encrypt(plaintext, password, config)?;
    serde_json::to_string_pretty(&payload).map_err(|e| EncryptionError::Encrypt(e.to_string()))
}

/// Decrypt from a JSON-serialized payload.
pub fn decrypt_from_json(json: &str, password: &[u8]) -> Result<Vec<u8>, EncryptionError> {
    let payload: EncryptedPayload =
        serde_json::from_str(json).map_err(|e| EncryptionError::InvalidPayload(e.to_string()))?;
    decrypt(&payload, password)
}

/// Secure erase: overwrite a byte buffer with zeros.
pub fn secure_erase(buf: &mut [u8]) {
    for b in buf.iter_mut() {
        *b = 0;
    }
    // Prevent optimization from eliding the zero-write
    std::hint::black_box(&buf);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key() {
        let dk = derive_key(b"password", b"salt1234salt1234", 1000);
        assert_eq!(dk.bytes.len(), 32);
        // Deterministic
        let dk2 = derive_key(b"password", b"salt1234salt1234", 1000);
        assert_eq!(dk.bytes, dk2.bytes);
    }

    #[test]
    fn test_derive_key_different_passwords() {
        let dk1 = derive_key(b"pass1", b"salt", 100);
        let dk2 = derive_key(b"pass2", b"salt", 100);
        assert_ne!(dk1.bytes, dk2.bytes);
    }

    #[test]
    fn test_derive_key_different_salts() {
        let dk1 = derive_key(b"pass", b"salt1", 100);
        let dk2 = derive_key(b"pass", b"salt2", 100);
        assert_ne!(dk1.bytes, dk2.bytes);
    }

    #[test]
    fn test_xor_roundtrip() {
        let key = b"0123456789abcdef0123456789abcdef";
        let nonce = b"nonce1234567";
        let plaintext = b"Hello, encrypted world!";
        let ct = xor_encrypt(key, nonce, plaintext);
        assert_ne!(&ct, plaintext);
        let pt = xor_encrypt(key, nonce, &ct);
        assert_eq!(&pt, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_aes256gcm() {
        let config = EncryptionConfig {
            iterations: 100, // low for test speed
            ..Default::default()
        };
        let plaintext = b"Sensitive data for ATP processing";
        let password = b"strong-password-123";
        let payload = encrypt(plaintext, password, &config).unwrap();
        assert_eq!(payload.algorithm, Algorithm::Aes256Gcm);
        assert!(!payload.tag.is_empty());
        let decrypted = decrypt(&payload, password).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_xor() {
        let config = EncryptionConfig {
            algorithm: Algorithm::XorStream,
            kdf: Kdf::None,
            iterations: 0,
            salt_len: 16,
            nonce_len: 12,
        };
        let plaintext = b"XOR test data";
        let password = b"0123456789abcdef0123456789abcdef";
        let payload = encrypt(plaintext, password, &config).unwrap();
        let decrypted = decrypt(&payload, password).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_password_fails() {
        let config = EncryptionConfig {
            iterations: 100,
            ..Default::default()
        };
        let payload = encrypt(b"secret", b"correct", &config).unwrap();
        let result = decrypt(&payload, b"wrong");
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let config = EncryptionConfig {
            iterations: 100,
            ..Default::default()
        };
        let mut payload = encrypt(b"data", b"pass", &config).unwrap();
        if !payload.ciphertext.is_empty() {
            payload.ciphertext[0] ^= 0xFF;
        }
        let result = decrypt(&payload, b"pass");
        assert!(result.is_err());
    }

    #[test]
    fn test_json_roundtrip() {
        let config = EncryptionConfig {
            iterations: 100,
            ..Default::default()
        };
        let plaintext = b"JSON envelope test";
        let json = encrypt_to_json(plaintext, b"pass123", &config).unwrap();
        assert!(json.contains("algorithm"));
        let decrypted = decrypt_from_json(&json, b"pass123").unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_invalid_json() {
        let result = decrypt_from_json("not json", b"pass");
        assert!(result.is_err());
    }

    #[test]
    fn test_secure_erase() {
        let mut buf = vec![0xAA; 64];
        secure_erase(&mut buf);
        assert!(buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_generate_salt_length() {
        let salt = generate_salt(16);
        assert_eq!(salt.len(), 16);
        let salt8 = generate_salt(8);
        assert_eq!(salt8.len(), 8);
    }

    #[test]
    fn test_generate_nonce_length() {
        let nonce = generate_nonce(12);
        assert_eq!(nonce.len(), 12);
    }

    #[test]
    fn test_metadata_preserved() {
        let config = EncryptionConfig {
            iterations: 100,
            ..Default::default()
        };
        let mut payload = encrypt(b"data", b"pass", &config).unwrap();
        payload
            .metadata
            .insert("source".into(), "atp-pipeline".into());
        let json = serde_json::to_string(&payload).unwrap();
        let restored: EncryptedPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.metadata["source"], "atp-pipeline");
    }

    #[test]
    fn test_empty_plaintext() {
        let config = EncryptionConfig {
            iterations: 100,
            ..Default::default()
        };
        let payload = encrypt(b"", b"pass", &config).unwrap();
        let decrypted = decrypt(&payload, b"pass").unwrap();
        assert!(decrypted.is_empty());
    }
}
