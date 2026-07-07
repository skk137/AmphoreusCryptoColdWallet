use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit};
use anyhow::{anyhow, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use super::secret::{wipe, SecretBytes, SecretString};

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
// Argon2id params: 19 MiB memory, 2 iterations, 1 lane — OWASP's minimum
// recommendation for interactive login-style KDF use.
const ARGON2_MEM_KIB: u32 = 19_456;
const ARGON2_ITERATIONS: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;

/// On-disk format for the encrypted seed file written to the USB drive.
/// All binary fields are base64 so the file can be safely stored as JSON.
#[derive(Serialize, Deserialize)]
pub struct EncryptedSeedFile {
    pub version: u8,
    pub salt: String,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn encrypt_phrase(phrase: &SecretString, pin: &str) -> Result<EncryptedSeedFile> {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    let key = derive_key(pin, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).context("invalid key length")?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce_bytes.into(), phrase.as_str().as_bytes())
        .map_err(|_| anyhow!("encryption failed"))?;

    Ok(EncryptedSeedFile {
        version: 1,
        salt: B64.encode(salt),
        nonce: B64.encode(nonce_bytes),
        ciphertext: B64.encode(ciphertext),
    })
}

pub fn decrypt_phrase(file: &EncryptedSeedFile, pin: &str) -> Result<SecretString> {
    if file.version != 1 {
        return Err(anyhow!("unsupported seed file version: {}", file.version));
    }

    let salt = B64.decode(&file.salt).context("corrupted seed file (salt)")?;
    let nonce_vec = B64.decode(&file.nonce).context("corrupted seed file (nonce)")?;
    let nonce_bytes: [u8; NONCE_LEN] = nonce_vec
        .try_into()
        .map_err(|_| anyhow!("corrupted seed file (wrong nonce length)"))?;
    let ciphertext = B64
        .decode(&file.ciphertext)
        .context("corrupted seed file (ciphertext)")?;

    let key = derive_key(pin, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes()).context("invalid key length")?;

    let plaintext = cipher
        .decrypt(&nonce_bytes.into(), ciphertext.as_ref())
        .map_err(|_| anyhow!("wrong PIN or corrupted seed file"))?;

    let phrase = String::from_utf8(plaintext).context("decrypted data was not valid UTF-8")?;
    Ok(SecretString::new(phrase))
}

fn derive_key(pin: &str, salt: &[u8]) -> Result<SecretBytes> {
    let params = Params::new(ARGON2_MEM_KIB, ARGON2_ITERATIONS, ARGON2_PARALLELISM, Some(KEY_LEN))
        .map_err(|e| anyhow!("bad argon2 params: {e}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(pin.as_bytes(), salt, &mut key)
        .map_err(|e| anyhow!("key derivation failed: {e}"))?;

    let secret = SecretBytes::new(key.to_vec());
    wipe(&mut key);
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let phrase = SecretString::new("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string());
        let encrypted = encrypt_phrase(&phrase, "correct-horse").unwrap();
        let decrypted = decrypt_phrase(&encrypted, "correct-horse").unwrap();
        assert_eq!(decrypted.as_str(), phrase.as_str());
    }

    #[test]
    fn wrong_pin_fails_to_decrypt() {
        let phrase = SecretString::new("test phrase".to_string());
        let encrypted = encrypt_phrase(&phrase, "correct-horse").unwrap();
        let result = decrypt_phrase(&encrypted, "wrong-pin");
        assert!(result.is_err());
    }

    #[test]
    fn each_encryption_uses_a_fresh_salt_and_nonce() {
        let phrase = SecretString::new("test phrase".to_string());
        let a = encrypt_phrase(&phrase, "pin123456").unwrap();
        let b = encrypt_phrase(&phrase, "pin123456").unwrap();
        assert_ne!(a.salt, b.salt);
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ciphertext, b.ciphertext);
    }
}
