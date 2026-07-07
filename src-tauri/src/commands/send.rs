use std::str::FromStr;

use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use bitcoin::absolute::LockTime;
use bitcoin::bip32::Xpriv;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::hashes::Hash as _;
use bitcoin::secp256k1::{Message, PublicKey, Secp256k1};
use bitcoin::sighash::{EcdsaSighashType, SighashCache};
use bitcoin::transaction::Version;
use bitcoin::{
    Address, Amount, CompressedPublicKey, OutPoint, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Txid, Witness,
};
use ed25519_dalek::SigningKey;
use serde::Deserialize;
use serde_json::json;
use tauri::State;

use super::chain::{BTC_NETWORK, ESPLORA_BASE, SOLANA_RPC, STABLECOIN_DECIMALS, STABLECOIN_MINT};
use crate::crypto::{derivation, seed};
use crate::state::AppState;

/// Outputs below this are uneconomical to spend ("dust") and rejected by nodes.
const DUST_SATS: u64 = 546;

#[derive(Deserialize)]
struct Utxo {
    txid: String,
    vout: u32,
    value: u64,
}

/// Builds, signs, and broadcasts a Bitcoin testnet transaction. The private
/// key is derived from the in-memory seed, used for signing inside this
/// function only, and never leaves the Rust side.
#[tauri::command]
pub async fn send_btc(
    state: State<'_, AppState>,
    to: String,
    amount_sats: u64,
) -> Result<String, String> {
    // Derive the key material synchronously and drop the state lock before
    // any network I/O.
    let (xpriv, from_addr) = {
        let guard = state.0.lock().expect("wallet state mutex poisoned");
        let unlocked = guard.as_ref().ok_or("wallet is locked")?;
        let seed_bytes = seed::phrase_to_seed(&unlocked.mnemonic, "").map_err(|e| e.to_string())?;
        let xpriv = derivation::derive_bitcoin_xpriv(&seed_bytes, BTC_NETWORK, 0)
            .map_err(|e| e.to_string())?;
        let secp = Secp256k1::new();
        let pubkey = PublicKey::from_secret_key(&secp, &xpriv.private_key);
        let addr = Address::p2wpkh(&CompressedPublicKey(pubkey), BTC_NETWORK);
        (xpriv, addr)
    };
    inner_send_btc(xpriv, from_addr, &to, amount_sats)
        .await
        .map_err(|e| e.to_string())
}

async fn inner_send_btc(xpriv: Xpriv, from: Address, to: &str, amount: u64) -> Result<String> {
    if amount < DUST_SATS {
        bail!("το ποσό είναι πολύ μικρό (ελάχιστο {DUST_SATS} sats)");
    }
    let to_addr = Address::from_str(to)
        .map_err(|_| anyhow!("μη έγκυρη διεύθυνση Bitcoin"))?
        .require_network(BTC_NETWORK)
        .map_err(|_| anyhow!("η διεύθυνση δεν είναι testnet διεύθυνση"))?;

    let client = reqwest::Client::new();

    let utxos: Vec<Utxo> = client
        .get(format!("{ESPLORA_BASE}/address/{from}/utxo"))
        .send()
        .await
        .context("αποτυχία λήψης UTXOs")?
        .error_for_status()?
        .json()
        .await
        .context("μη έγκυρη απάντηση UTXO")?;
    if utxos.is_empty() {
        bail!("δεν υπάρχουν διαθέσιμα κεφάλαια σε αυτή τη διεύθυνση");
    }

    // Fee rate: use the 3-block estimate; testnet is often ~1 sat/vB.
    let estimates: serde_json::Value = client
        .get(format!("{ESPLORA_BASE}/fee-estimates"))
        .send()
        .await
        .context("αποτυχία λήψης fee estimates")?
        .json()
        .await
        .unwrap_or_else(|_| json!({}));
    let fee_rate = estimates["3"].as_f64().unwrap_or(1.0).max(1.0);

    // Approximate P2WPKH vsize: 11 overhead + 68 per input + 31 per output.
    let fee_for =
        |n_in: usize, n_out: usize| ((11.0 + 68.0 * n_in as f64 + 31.0 * n_out as f64) * fee_rate).ceil() as u64;

    // Largest-first coin selection.
    let mut candidates = utxos;
    candidates.sort_by(|a, b| b.value.cmp(&a.value));
    let mut selected: Vec<Utxo> = Vec::new();
    let mut total = 0u64;
    for u in candidates {
        total += u.value;
        selected.push(u);
        if total >= amount + fee_for(selected.len(), 2) {
            break;
        }
    }
    let fee = fee_for(selected.len(), 2);
    if total < amount + fee {
        bail!(
            "ανεπαρκές υπόλοιπο: διαθέσιμα {total} sats, χρειάζονται {} (ποσό + fee)",
            amount + fee
        );
    }
        
    let change = total - amount - fee;
    let mut outputs = vec![TxOut {
        value: Amount::from_sat(amount),
        script_pubkey: to_addr.script_pubkey(),
    }];
    // Change below dust is left to the miners instead of creating an
    // unspendable output.
    if change >= DUST_SATS {
        outputs.push(TxOut {
            value: Amount::from_sat(change),
            script_pubkey: from.script_pubkey(),
        });
    }

    let mut inputs = Vec::with_capacity(selected.len());
    for u in &selected {
        inputs.push(TxIn {
            previous_output: OutPoint {
                txid: Txid::from_str(&u.txid).context("μη έγκυρο txid στο UTXO")?,
                vout: u.vout,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
            witness: Witness::default(),
        });
    }

    let mut tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: inputs,
        output: outputs,
    };

    let secp = Secp256k1::new();
    let pubkey = PublicKey::from_secret_key(&secp, &xpriv.private_key);
    let spk = from.script_pubkey();
    let mut cache = SighashCache::new(&mut tx);
    for (i, u) in selected.iter().enumerate() {
        let sighash = cache
            .p2wpkh_signature_hash(i, &spk, Amount::from_sat(u.value), EcdsaSighashType::All)
            .context("αποτυχία υπολογισμού sighash")?;
        let msg = Message::from_digest(sighash.to_byte_array());
        let signature = secp.sign_ecdsa(&msg, &xpriv.private_key);
        let sig = bitcoin::ecdsa::Signature {
            signature,
            sighash_type: EcdsaSighashType::All,
        };
        *cache
            .witness_mut(i)
            .ok_or_else(|| anyhow!("λείπει input {i}"))? = Witness::p2wpkh(&sig, &pubkey);
    }
    drop(cache);

    let hex = serialize_hex(&tx);
    let resp = client
        .post(format!("{ESPLORA_BASE}/tx"))
        .body(hex)
        .send()
        .await
        .context("αποτυχία broadcast")?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("το δίκτυο απέρριψε τη συναλλαγή: {body}");
    }
    Ok(body) // the txid
}

/// Builds, signs, and broadcasts a SOL transfer on devnet.
#[tauri::command]
pub async fn send_sol(
    state: State<'_, AppState>,
    to: String,
    amount_lamports: u64,
) -> Result<String, String> {
    let sol_key = {
        let guard = state.0.lock().expect("wallet state mutex poisoned");
        let unlocked = guard.as_ref().ok_or("wallet is locked")?;
        let seed_bytes = seed::phrase_to_seed(&unlocked.mnemonic, "").map_err(|e| e.to_string())?;
        derivation::derive_solana_signing_key(&seed_bytes, 0).map_err(|e| e.to_string())?
    };
    inner_send_sol(sol_key, &to, amount_lamports)
        .await
        .map_err(|e| e.to_string())
}

async fn inner_send_sol(key: SigningKey, to: &str, lamports: u64) -> Result<String> {
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::transaction::Transaction as SolTransaction;

    if lamports == 0 {
        bail!("το ποσό δεν μπορεί να είναι μηδέν");
    }

    let keypair = Keypair::new_from_array(key.to_bytes());
    let from_addr = keypair.pubkey();
    let to_addr = solana_sdk::pubkey::Pubkey::from_str(to)
        .map_err(|_| anyhow!("μη έγκυρη διεύθυνση Solana"))?;

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(SOLANA_RPC)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "getLatestBlockhash", "params": []
        }))
        .send()
        .await
        .context("αποτυχία λήψης blockhash")?
        .json()
        .await?;
    let blockhash_str = resp["result"]["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| anyhow!("μη έγκυρη απάντηση blockhash"))?;
    let blockhash = solana_sdk::hash::Hash::from_str(blockhash_str)?;

    let ix = solana_system_interface::instruction::transfer(&from_addr, &to_addr, lamports);
    let tx = SolTransaction::new_signed_with_payer(&[ix], Some(&from_addr), &[&keypair], blockhash);

    let tx_b64 = B64.encode(bincode::serialize(&tx).context("αποτυχία serialization")?);
    let resp: serde_json::Value = client
        .post(SOLANA_RPC)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "sendTransaction",
            "params": [tx_b64, { "encoding": "base64" }]
        }))
        .send()
        .await
        .context("αποτυχία broadcast")?
        .json()
        .await?;

    if let Some(err) = resp.get("error") {
        bail!("το δίκτυο απέρριψε τη συναλλαγή: {err}");
    }
    resp["result"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("μη έγκυρη απάντηση από το RPC"))
}

/// Sends the stablecoin (SPL token) on Solana. Creates the recipient's
/// associated token account if it doesn't exist yet (idempotent), then does a
/// checked transfer. Fees are paid in SOL by the sender.
#[tauri::command]
pub async fn send_usdc(
    state: State<'_, AppState>,
    to: String,
    amount_base: u64,
) -> Result<String, String> {
    let sol_key = {
        let guard = state.0.lock().expect("wallet state mutex poisoned");
        let unlocked = guard.as_ref().ok_or("wallet is locked")?;
        let seed_bytes = seed::phrase_to_seed(&unlocked.mnemonic, "").map_err(|e| e.to_string())?;
        derivation::derive_solana_signing_key(&seed_bytes, 0).map_err(|e| e.to_string())?
    };
    inner_send_usdc(sol_key, &to, amount_base)
        .await
        .map_err(|e| e.to_string())
}

async fn inner_send_usdc(key: SigningKey, to: &str, amount: u64) -> Result<String> {
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk::transaction::Transaction as SolTransaction;
    use spl_associated_token_account_interface::address::get_associated_token_address;
    use spl_associated_token_account_interface::instruction::create_associated_token_account_idempotent;

    if amount == 0 {
        bail!("το ποσό δεν μπορεί να είναι μηδέν");
    }

    let keypair = Keypair::new_from_array(key.to_bytes());
    let owner = keypair.pubkey();
    let recipient = solana_sdk::pubkey::Pubkey::from_str(to)
        .map_err(|_| anyhow!("μη έγκυρη διεύθυνση Solana"))?;
    let mint = solana_sdk::pubkey::Pubkey::from_str(STABLECOIN_MINT)?;
    let token_program = spl_token::id();

    let source_ata = get_associated_token_address(&owner, &mint);
    let dest_ata = get_associated_token_address(&recipient, &mint);

    let create_ata_ix =
        create_associated_token_account_idempotent(&owner, &recipient, &mint, &token_program);
    let transfer_ix = spl_token::instruction::transfer_checked(
        &token_program,
        &source_ata,
        &mint,
        &dest_ata,
        &owner,
        &[],
        amount,
        STABLECOIN_DECIMALS,
    )
    .map_err(|e| anyhow!("αποτυχία δημιουργίας instruction: {e}"))?;

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .post(SOLANA_RPC)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "getLatestBlockhash", "params": []
        }))
        .send()
        .await
        .context("αποτυχία λήψης blockhash")?
        .json()
        .await?;
    let blockhash_str = resp["result"]["value"]["blockhash"]
        .as_str()
        .ok_or_else(|| anyhow!("μη έγκυρη απάντηση blockhash"))?;
    let blockhash = solana_sdk::hash::Hash::from_str(blockhash_str)?;

    let tx = SolTransaction::new_signed_with_payer(
        &[create_ata_ix, transfer_ix],
        Some(&owner),
        &[&keypair],
        blockhash,
    );

    let tx_b64 = B64.encode(bincode::serialize(&tx).context("αποτυχία serialization")?);
    let resp: serde_json::Value = client
        .post(SOLANA_RPC)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "sendTransaction",
            "params": [tx_b64, { "encoding": "base64" }]
        }))
        .send()
        .await
        .context("αποτυχία broadcast")?
        .json()
        .await?;

    if let Some(err) = resp.get("error") {
        bail!("το δίκτυο απέρριψε τη συναλλαγή: {err}");
    }
    resp["result"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("μη έγκυρη απάντηση από το RPC"))
}
