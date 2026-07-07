use anyhow::{anyhow, bail, Context, Result};
use rlp::RlpStream;
use secp256k1::{Message, Secp256k1, SecretKey};
use serde_json::json;
use sha3::{Digest, Keccak256};
use tauri::State;

use super::chain::find_evm_chain;
use crate::crypto::{derivation, seed};
use crate::state::AppState;

/// Builds, signs (EIP-155 legacy tx), and broadcasts an EVM transaction —
/// either a native transfer (POL/ETH) or an ERC-20 token transfer (USDC, …).
/// `asset` is "native" or a token symbol. `amount` is a human decimal string
/// (e.g. "0.5") so 18-decimal values never lose precision through JS floats.
#[tauri::command]
pub async fn send_evm(
    state: State<'_, AppState>,
    network: String,
    asset: String,
    to: String,
    amount: String,
) -> Result<String, String> {
    inner_send_evm(&state, &network, &asset, &to, &amount)
        .await
        .map_err(|e| e.to_string())
}

async fn inner_send_evm(
    state: &AppState,
    network: &str,
    asset: &str,
    to: &str,
    amount: &str,
) -> Result<String> {
    let chain = find_evm_chain(network).ok_or_else(|| anyhow!("άγνωστο δίκτυο: {network}"))?;

    // Derive the signing key + sender address. Key stays in this function.
    let secret = {
        let guard = state.0.lock().expect("wallet state mutex poisoned");
        let unlocked = guard.as_ref().ok_or_else(|| anyhow!("wallet is locked"))?;
        let seed_bytes = seed::phrase_to_seed(&unlocked.mnemonic, "")?;
        derivation::derive_evm_secret_key(&seed_bytes, 0)?
    };
    let secp = Secp256k1::new();
    let from = evm_address_of(&secp, &secret);
    let recipient = parse_addr(to).context("μη έγκυρη διεύθυνση παραλήπτη")?;

    // Decide native vs token, and build (to, value, data).
    let native = asset.eq_ignore_ascii_case("native") || asset.eq_ignore_ascii_case(chain.native_symbol);
    let (tx_to, value, data) = if native {
        let value = parse_amount(amount, chain.native_decimals)?;
        (recipient, value, Vec::new())
    } else {
        let token = chain
            .tokens
            .iter()
            .find(|t| t.symbol.eq_ignore_ascii_case(asset))
            .ok_or_else(|| anyhow!("άγνωστο token: {asset}"))?;
        let base = parse_amount(amount, token.decimals)?;
        let contract = parse_addr(token.contract)?;
        (contract, 0u128, erc20_transfer_data(&recipient, base))
    };

    let client = reqwest::Client::new();
    let from_hex = format!("0x{}", hex::encode(from));

    let nonce = hex_to_u128(
        rpc(&client, chain.rpc, "eth_getTransactionCount", json!([from_hex, "pending"])).await?,
    ) as u64;
    let gas_price = hex_to_u128(rpc(&client, chain.rpc, "eth_gasPrice", json!([])).await?);

    // Estimate gas; fall back to safe defaults if the node refuses.
    let call = json!([{
        "from": from_hex,
        "to": format!("0x{}", hex::encode(tx_to)),
        "value": format!("0x{:x}", value),
        "data": format!("0x{}", hex::encode(&data)),
    }]);
    let gas_limit = match rpc(&client, chain.rpc, "eth_estimateGas", call).await {
        Ok(v) => (hex_to_u128(v) as f64 * 1.2).ceil() as u128,
        Err(_) if native => 21_000,
        Err(_) => 120_000,
    };

    // EIP-155 signing hash: rlp([nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0]).
    let unsigned = rlp_tx(nonce, gas_price, gas_limit, &tx_to, value, &data, |s| {
        s.append(&be(chain.chain_id as u128));
        s.append_empty_data();
        s.append_empty_data();
    });
    let hash = keccak(&unsigned);

    let sig = secp.sign_ecdsa_recoverable(&Message::from_digest(hash), &secret);
    let (recid, compact) = sig.serialize_compact();
    let r = &compact[0..32];
    let s_val = &compact[32..64];
    let v = recid.to_i32() as u128 + (chain.chain_id as u128) * 2 + 35;

    // Signed tx: rlp([nonce, gasPrice, gasLimit, to, value, data, v, r, s]).
    let signed = rlp_tx(nonce, gas_price, gas_limit, &tx_to, value, &data, |st| {
        st.append(&be(v));
        st.append(&strip(r));
        st.append(&strip(s_val));
    });

    let raw = format!("0x{}", hex::encode(&signed));
    let result = rpc(&client, chain.rpc, "eth_sendRawTransaction", json!([raw])).await?;
    result
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("μη έγκυρη απάντηση από το δίκτυο"))
}

// ---- helpers ----

fn evm_address_of(secp: &Secp256k1<secp256k1::All>, secret: &SecretKey) -> [u8; 20] {
    let pubkey = secp256k1::PublicKey::from_secret_key(secp, secret);
    let uncompressed = pubkey.serialize_uncompressed();
    let hash = Keccak256::digest(&uncompressed[1..]);
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[12..]);
    out
}

fn parse_addr(s: &str) -> Result<[u8; 20]> {
    let bytes = hex::decode(s.trim().trim_start_matches("0x")).context("μη έγκυρη διεύθυνση")?;
    let arr: [u8; 20] = bytes
        .try_into()
        .map_err(|_| anyhow!("η διεύθυνση πρέπει να είναι 20 bytes"))?;
    Ok(arr)
}

/// `transfer(address,uint256)` calldata: selector 0xa9059cbb + padded args.
fn erc20_transfer_data(to: &[u8; 20], amount: u128) -> Vec<u8> {
    let mut data = Vec::with_capacity(4 + 32 + 32);
    data.extend_from_slice(&[0xa9, 0x05, 0x9c, 0xbb]);
    data.extend_from_slice(&[0u8; 12]);
    data.extend_from_slice(to);
    let mut amt = [0u8; 32];
    amt[16..].copy_from_slice(&amount.to_be_bytes());
    data.extend_from_slice(&amt);
    data
}

/// Converts a human decimal string ("0.5") to integer base units.
fn parse_amount(s: &str, decimals: u32) -> Result<u128> {
    let s = s.trim();
    let (int_part, frac_part) = s.split_once('.').unwrap_or((s, ""));
    let decimals = decimals as usize;
    if frac_part.len() > decimals {
        bail!("πάρα πολλά δεκαδικά ψηφία για αυτό το token");
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
        || (int_part.is_empty() && frac_part.is_empty())
    {
        bail!("μη έγκυρο ποσό");
    }
    let mut digits = String::from(int_part);
    digits.push_str(frac_part);
    digits.push_str(&"0".repeat(decimals - frac_part.len()));
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        return Ok(0);
    }
    trimmed.parse::<u128>().context("το ποσό είναι πολύ μεγάλο")
}

/// Minimal big-endian bytes (RLP integer form).
fn strip(bytes: &[u8]) -> Vec<u8> {
    let first = bytes.iter().position(|&b| b != 0).unwrap_or(bytes.len());
    bytes[first..].to_vec()
}

fn be(v: u128) -> Vec<u8> {
    strip(&v.to_be_bytes())
}

fn keccak(bytes: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Keccak256::digest(bytes));
    out
}

/// RLP-encodes the six common tx fields, then runs `tail` to append the
/// remaining three (chainId/0/0 for signing, or v/r/s once signed).
fn rlp_tx(
    nonce: u64,
    gas_price: u128,
    gas_limit: u128,
    to: &[u8; 20],
    value: u128,
    data: &[u8],
    tail: impl FnOnce(&mut RlpStream),
) -> Vec<u8> {
    let mut s = RlpStream::new_list(9);
    s.append(&be(nonce as u128));
    s.append(&be(gas_price));
    s.append(&be(gas_limit));
    s.append(&to.to_vec());
    s.append(&be(value));
    s.append(&data.to_vec());
    tail(&mut s);
    s.out().to_vec()
}

fn hex_to_u128(v: serde_json::Value) -> u128 {
    u128::from_str_radix(v.as_str().unwrap_or("0x0").trim_start_matches("0x"), 16).unwrap_or(0)
}

async fn rpc(
    client: &reqwest::Client,
    rpc_url: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value> {
    let resp: serde_json::Value = client
        .post(rpc_url)
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .send()
        .await
        .context("αποτυχία σύνδεσης με το δίκτυο")?
        .json()
        .await
        .context("μη έγκυρη απάντηση δικτύου")?;
    if let Some(err) = resp.get("error") {
        bail!("EVM RPC error: {err}");
    }
    Ok(resp["result"].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_amount_handles_decimals() {
        assert_eq!(parse_amount("1", 18).unwrap(), 1_000_000_000_000_000_000);
        assert_eq!(parse_amount("0.5", 6).unwrap(), 500_000);
        assert_eq!(parse_amount("1.25", 2).unwrap(), 125);
        assert_eq!(parse_amount("0", 18).unwrap(), 0);
        assert!(parse_amount("0.1234567", 6).is_err()); // too many decimals
        assert!(parse_amount("abc", 6).is_err());
    }

    #[test]
    fn signing_recovers_to_sender_address() {
        // Full round trip: derive key → sign a hash → recover pubkey → address.
        // If RLP/keccak/recovery were wrong this would not match.
        use crate::crypto::secret::SecretString;
        use secp256k1::ecdsa::{RecoverableSignature, RecoveryId};
        let phrase = SecretString::new(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string(),
        );
        let seed = crate::crypto::seed::phrase_to_seed(&phrase, "").unwrap();
        let secret = derivation::derive_evm_secret_key(&seed, 0).unwrap();
        let secp = Secp256k1::new();
        let expected = evm_address_of(&secp, &secret);

        let hash = keccak(b"hello amphoreus");
        let sig = secp.sign_ecdsa_recoverable(&Message::from_digest(hash), &secret);
        let (recid, compact) = sig.serialize_compact();

        let recovered = RecoverableSignature::from_compact(&compact, RecoveryId::from_i32(recid.to_i32()).unwrap())
            .and_then(|rs| secp.recover_ecdsa(&Message::from_digest(hash), &rs))
            .unwrap();
        let unc = recovered.serialize_uncompressed();
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&Keccak256::digest(&unc[1..])[12..]);
        assert_eq!(addr, expected);
    }
}
