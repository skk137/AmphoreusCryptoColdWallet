use anyhow::{Context, Result};
use bip39::Mnemonic;
use rand::rngs::OsRng;
use rand::RngCore;

use super::secret::{wipe, SecretBytes, SecretString};

/// Generates a fresh 24-word BIP39 mnemonic from 256 bits of OS randomness.
pub fn generate_mnemonic() -> Result<SecretString> {
    let mut entropy = [0u8; 32];
    OsRng.fill_bytes(&mut entropy);
    let mnemonic = Mnemonic::from_entropy(&entropy).context("failed to build mnemonic from entropy")?;
    wipe(&mut entropy);
    Ok(SecretString::new(mnemonic.to_string()))
}

/// Validates a mnemonic phrase (checksum + wordlist membership).
pub fn validate_mnemonic(phrase: &str) -> Result<()> {
    Mnemonic::parse(phrase.trim()).context("invalid mnemonic phrase")?;
    Ok(())
}

/// Expands a mnemonic phrase into the 64-byte BIP39 seed used for HD key derivation.
pub fn phrase_to_seed(phrase: &SecretString, passphrase: &str) -> Result<SecretBytes> {
    let mnemonic = Mnemonic::parse(phrase.as_str().trim()).context("invalid mnemonic phrase")?;
    let seed = mnemonic.to_seed(passphrase);
    Ok(SecretBytes::new(seed.to_vec()))
}
