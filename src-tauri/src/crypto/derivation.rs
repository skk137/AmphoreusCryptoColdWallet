use std::str::FromStr;

use anyhow::{Context, Result};
use bitcoin::bip32::{DerivationPath, Xpriv};
use bitcoin::secp256k1::{PublicKey, Secp256k1};
use bitcoin::Network;
use ed25519_dalek::SigningKey;
use ed25519_dalek_bip32::{DerivationPath as EdDerivationPath, ExtendedSigningKey};
use sha3::{Digest, Keccak256};

use super::secret::SecretBytes;

/// Derives a Bitcoin BIP32 extended private key for a native-segwit (BIP84)
/// account: `m/84'/{0 or 1}'/0'/0/{index}`. Coin type 0' is mainnet, 1' is
/// the shared testnet/signet/regtest coin type.
pub fn derive_bitcoin_xpriv(seed: &SecretBytes, network: Network, index: u32) -> Result<Xpriv> {
    let secp = Secp256k1::new();
    let master =
        Xpriv::new_master(network, seed.as_bytes()).context("failed to derive BTC master key")?;
    let coin_type = if network == Network::Bitcoin { 0 } else { 1 };
    let path = DerivationPath::from_str(&format!("m/84'/{coin_type}'/0'/0/{index}"))
        .context("invalid BTC derivation path")?;
    master
        .derive_priv(&secp, &path)
        .context("failed to derive BTC child key")
}

/// Derives a Solana ed25519 signing key at `m/44'/501'/{index}'/0'`
/// (SLIP-0010, fully hardened — ed25519 does not support non-hardened
/// child derivation). This matches the path convention used by Phantom
/// and Solflare.
pub fn derive_solana_signing_key(seed: &SecretBytes, index: u32) -> Result<SigningKey> {
    let path = EdDerivationPath::from_str(&format!("m/44'/501'/{index}'/0'"))
        .map_err(|e| anyhow::anyhow!("invalid SOL derivation path: {e}"))?;
    let extended = ExtendedSigningKey::from_seed(seed.as_bytes())
        .map_err(|e| anyhow::anyhow!("failed to derive SOL master key: {e}"))?
        .derive(&path)
        .map_err(|e| anyhow::anyhow!("failed to derive SOL child key: {e}"))?;
    Ok(extended.signing_key)
}

/// Derives the EVM (Ethereum-compatible) address at `m/44'/60'/0'/0/{index}`.
/// The same address works on every EVM chain (Polygon, Arbitrum, Ethereum,
/// Base, …) because they all share the secp256k1 keypair — only the network
/// and token contract differ. Returns the checksummed-lowercase `0x…` string.
pub fn derive_evm_address(seed: &SecretBytes, index: u32) -> Result<String> {
    let secp = Secp256k1::new();
    let secret = derive_evm_secret_key(seed, index)?;
    let pubkey = PublicKey::from_secret_key(&secp, &secret);

    // Ethereum address = last 20 bytes of keccak256(uncompressed pubkey without
    // the 0x04 prefix, i.e. the raw X||Y coordinates).
    let uncompressed = pubkey.serialize_uncompressed();
    let hash = Keccak256::digest(&uncompressed[1..]);
    Ok(format!("0x{}", hex::encode(&hash[12..])))
}

/// Derives the compressed secp256k1 public key at a BIP32 path (network flag
/// is irrelevant to the key bytes). Shared by the UTXO-fork chains.
fn derive_compressed_pubkey(seed: &SecretBytes, path: &str) -> Result<[u8; 33]> {
    let secp = Secp256k1::new();
    let master =
        Xpriv::new_master(Network::Bitcoin, seed.as_bytes()).context("failed to derive master key")?;
    let dp = DerivationPath::from_str(path).context("invalid derivation path")?;
    let child = master.derive_priv(&secp, &dp).context("failed to derive child key")?;
    let pk = PublicKey::from_secret_key(&secp, &child.private_key);
    Ok(pk.serialize())
}

/// Litecoin testnet native-segwit address (`tltc1…`) at `m/84'/1'/0'/0/0`.
pub fn derive_litecoin_address(seed: &SecretBytes) -> Result<String> {
    use bitcoin::hashes::Hash as _;
    let pk = derive_compressed_pubkey(seed, "m/84'/1'/0'/0/0")?;
    let h160 = bitcoin::hashes::hash160::Hash::hash(&pk);
    let hrp = bech32::Hrp::parse("tltc").map_err(|e| anyhow::anyhow!("bad hrp: {e}"))?;
    bech32::segwit::encode_v0(hrp, &h160.to_byte_array())
        .map_err(|e| anyhow::anyhow!("LTC address encode failed: {e}"))
}

/// Dogecoin **mainnet** legacy P2PKH address (`D…`) at `m/44'/3'/0'/0/0`.
/// Mainnet because Dogecoin testnet has no usable public API — receive-only.
pub fn derive_dogecoin_address(seed: &SecretBytes) -> Result<String> {
    use bitcoin::hashes::Hash as _;
    let pk = derive_compressed_pubkey(seed, "m/44'/3'/0'/0/0")?;
    let h160 = bitcoin::hashes::hash160::Hash::hash(&pk);
    // Base58Check with Dogecoin's mainnet P2PKH version byte 0x1e.
    Ok(bs58::encode(h160.to_byte_array())
        .with_check_version(0x1e)
        .into_string())
}

/// Returns the secp256k1 private key for the EVM account at
/// `m/44'/60'/0'/0/{index}` — needed for signing transactions. Same key that
/// backs `derive_evm_address`.
pub fn derive_evm_secret_key(
    seed: &SecretBytes,
    index: u32,
) -> Result<bitcoin::secp256k1::SecretKey> {
    let secp = Secp256k1::new();
    // The network flag only affects xpub/xprv serialization, not the derived
    // key bytes, so Bitcoin's mainnet flag is fine here.
    let master =
        Xpriv::new_master(Network::Bitcoin, seed.as_bytes()).context("failed to derive EVM master key")?;
    let path = DerivationPath::from_str(&format!("m/44'/60'/0'/0/{index}"))
        .context("invalid EVM derivation path")?;
    let child = master
        .derive_priv(&secp, &path)
        .context("failed to derive EVM child key")?;
    Ok(child.private_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_seed() -> SecretBytes {
        SecretBytes::new(vec![0x42; 64])
    }

    #[test]
    fn bitcoin_derivation_is_deterministic() {
        let seed = dummy_seed();
        let a = derive_bitcoin_xpriv(&seed, Network::Testnet, 0).unwrap();
        let b = derive_bitcoin_xpriv(&seed, Network::Testnet, 0).unwrap();
        assert_eq!(a.private_key.secret_bytes(), b.private_key.secret_bytes());
    }

    #[test]
    fn bitcoin_derivation_differs_per_index() {
        let seed = dummy_seed();
        let a = derive_bitcoin_xpriv(&seed, Network::Testnet, 0).unwrap();
        let b = derive_bitcoin_xpriv(&seed, Network::Testnet, 1).unwrap();
        assert_ne!(a.private_key.secret_bytes(), b.private_key.secret_bytes());
    }

    #[test]
    fn solana_derivation_is_deterministic() {
        let seed = dummy_seed();
        let a = derive_solana_signing_key(&seed, 0).unwrap();
        let b = derive_solana_signing_key(&seed, 0).unwrap();
        assert_eq!(a.to_bytes(), b.to_bytes());
    }

    #[test]
    fn solana_derivation_differs_per_index() {
        let seed = dummy_seed();
        let a = derive_solana_signing_key(&seed, 0).unwrap();
        let b = derive_solana_signing_key(&seed, 1).unwrap();
        assert_ne!(a.to_bytes(), b.to_bytes());
    }

    #[test]
    fn ltc_and_doge_addresses_have_correct_format() {
        use crate::crypto::secret::SecretString;
        use crate::crypto::seed::phrase_to_seed;
        let phrase = SecretString::new(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
        );
        let seed = phrase_to_seed(&phrase, "").unwrap();
        let ltc = derive_litecoin_address(&seed).unwrap();
        let doge = derive_dogecoin_address(&seed).unwrap();
        println!("LTC(testnet)={ltc}");
        println!("DOGE(mainnet)={doge}");
        assert!(ltc.starts_with("tltc1"), "LTC addr wrong prefix: {ltc}");
        assert!(doge.starts_with('D'), "DOGE addr wrong prefix: {doge}");
    }

    #[test]
    fn evm_address_matches_known_test_vector() {
        // The canonical BIP39 test mnemonic with an empty passphrase derives
        // this Ethereum address at m/44'/60'/0'/0/0 — matches MetaMask/hardware
        // wallets. Confirms the keccak + derivation path are correct.
        use crate::crypto::seed::phrase_to_seed;
        use crate::crypto::secret::SecretString;
        let phrase = SecretString::new(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
                .to_string(),
        );
        let seed = phrase_to_seed(&phrase, "").unwrap();
        let addr = derive_evm_address(&seed, 0).unwrap();
        assert_eq!(addr.to_lowercase(), "0x9858effd232b4033e47d90003d41ec34ecaeda94");
    }
}
