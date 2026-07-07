use anyhow::{anyhow, Context, Result};
use bitcoin::secp256k1::{PublicKey, Secp256k1};
use bitcoin::{Address, CompressedPublicKey, Network};
use serde::Serialize;
use serde_json::json;
use tauri::State;

use crate::crypto::{derivation, seed};
use crate::state::AppState;

// Testnet/devnet for development. Flip these (and only these) to go mainnet
// after the flows are verified end-to-end.
pub const BTC_NETWORK: Network = Network::Testnet;
pub const ESPLORA_BASE: &str = "https://blockstream.info/testnet/api";
pub const SOLANA_RPC: &str = "https://api.devnet.solana.com";
// Circle's official devnet USDC mint (faucet.circle.com). On mainnet this
// becomes the USDT/USDC mainnet mint.
pub const STABLECOIN_MINT: &str = "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU";
pub const STABLECOIN_LABEL: &str = "USDC";
pub const STABLECOIN_DECIMALS: u8 = 6;

/// An EVM chain we track USDC on. All share one wallet address; only the RPC
/// endpoint and USDC contract differ. Testnets for now — swap the four fields
/// per network to go mainnet.
pub struct EvmChain {
    pub name: &'static str,
    pub rpc: &'static str,
    pub usdc_contract: &'static str,
    pub usdc_decimals: u32,
    pub explorer_tx: &'static str,
}

pub const EVM_CHAINS: &[EvmChain] = &[
    EvmChain {
        name: "Polygon Network",
        rpc: "https://polygon-amoy-bor-rpc.publicnode.com",
        usdc_contract: "0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582",
        usdc_decimals: 6,
        explorer_tx: "https://amoy.polygonscan.com/tx/",
    },
    EvmChain {
        name: "Arbitrum Network",
        rpc: "https://sepolia-rollup.arbitrum.io/rpc",
        usdc_contract: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d",
        usdc_decimals: 6,
        explorer_tx: "https://sepolia.arbiscan.io/tx/",
    },
];

#[derive(Serialize)]
pub struct Addresses {
    pub btc: String,
    pub sol: String,
    pub evm: String,
    pub network: String,
}

#[derive(Serialize)]
pub struct EvmBalance {
    pub network: String,
    pub usdc: f64,
    pub explorer_tx: String,
}

#[derive(Serialize)]
pub struct Balances {
    pub btc_sats: u64,
    /// Net unconfirmed amount sitting in the mempool (can be negative when
    /// an outgoing spend is pending).
    pub btc_pending_sats: i64,
    pub sol_lamports: u64,
    pub stablecoin: f64,
    pub stablecoin_label: String,
    pub evm: Vec<EvmBalance>,
}

/// Derives the receive addresses (account 0) from the unlocked seed. Only
/// public keys leave this function — the derived private keys drop out of
/// scope (and out of memory) before it returns.
#[tauri::command]
pub fn get_addresses(state: State<AppState>) -> Result<Addresses, String> {
    inner_get_addresses(&state).map_err(|e| e.to_string())
}

fn inner_get_addresses(state: &AppState) -> Result<Addresses> {
    let guard = state.0.lock().expect("wallet state mutex poisoned");
    let unlocked = guard.as_ref().ok_or_else(|| anyhow!("wallet is locked"))?;
    let seed_bytes = seed::phrase_to_seed(&unlocked.mnemonic, "")?;

    let secp = Secp256k1::new();
    let xpriv = derivation::derive_bitcoin_xpriv(&seed_bytes, BTC_NETWORK, 0)?;
    let pubkey = PublicKey::from_secret_key(&secp, &xpriv.private_key);
    let btc = Address::p2wpkh(&CompressedPublicKey(pubkey), BTC_NETWORK).to_string();

    let sol_key = derivation::derive_solana_signing_key(&seed_bytes, 0)?;
    let sol = bs58::encode(sol_key.verifying_key().to_bytes()).into_string();

    let evm = derivation::derive_evm_address(&seed_bytes, 0)?;

    Ok(Addresses {
        btc,
        sol,
        evm,
        network: format!("{BTC_NETWORK:?} / devnet / EVM testnets"),
    })
}

/// Fetches balances from public APIs. Takes the addresses as plain args so
/// no lock is held across network calls — this command never touches secrets.
#[tauri::command]
pub async fn get_balances(
    btc_address: String,
    sol_address: String,
    evm_address: String,
) -> Result<Balances, String> {
    inner_get_balances(&btc_address, &sol_address, &evm_address)
        .await
        .map_err(|e| e.to_string())
}

async fn inner_get_balances(
    btc_address: &str,
    sol_address: &str,
    evm_address: &str,
) -> Result<Balances> {
    let client = reqwest::Client::new();

    let (btc_sats, btc_pending_sats) = fetch_btc_balance(&client, btc_address).await?;
    let sol_lamports = fetch_sol_balance(&client, sol_address).await?;
    let stablecoin = fetch_stablecoin_balance(&client, sol_address).await?;

    // Fetch each EVM chain independently; a single failing RPC reports 0 for
    // that chain rather than sinking the whole request.
    let mut evm = Vec::new();
    for chain in EVM_CHAINS {
        let usdc = fetch_evm_usdc(&client, chain, evm_address)
            .await
            .unwrap_or(0.0);
        evm.push(EvmBalance {
            network: chain.name.to_string(),
            usdc,
            explorer_tx: chain.explorer_tx.to_string(),
        });
    }

    Ok(Balances {
        btc_sats,
        btc_pending_sats,
        sol_lamports,
        stablecoin,
        stablecoin_label: STABLECOIN_LABEL.to_string(),
        evm,
    })
}

/// Reads an EVM address's USDC balance via `eth_call` to the token contract's
/// `balanceOf(address)` (selector 0x70a08231). No secrets involved.
async fn fetch_evm_usdc(
    client: &reqwest::Client,
    chain: &EvmChain,
    address: &str,
) -> Result<f64> {
    let addr_hex = address.trim_start_matches("0x").to_lowercase();
    // ABI-encode: 4-byte selector + address left-padded to 32 bytes.
    let data = format!("0x70a08231{addr_hex:0>64}");
    let resp: serde_json::Value = client
        .post(chain.rpc)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "eth_call",
            "params": [{ "to": chain.usdc_contract, "data": data }, "latest"]
        }))
        .send()
        .await
        .context("EVM balance request failed")?
        .json()
        .await
        .context("EVM balance response was not JSON")?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow!("EVM RPC error: {err}"));
    }
    let hex = resp["result"].as_str().unwrap_or("0x0");
    let raw = u128::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(0);
    Ok(raw as f64 / 10f64.powi(chain.usdc_decimals as i32))
}

/// Returns (confirmed, pending) sats for the address.
async fn fetch_btc_balance(client: &reqwest::Client, address: &str) -> Result<(u64, i64)> {
    let url = format!("{ESPLORA_BASE}/address/{address}");
    let resp: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .context("BTC balance request failed")?
        .error_for_status()
        .context("BTC balance API returned an error")?
        .json()
        .await
        .context("BTC balance response was not JSON")?;

    let chain = &resp["chain_stats"];
    let confirmed = chain["funded_txo_sum"].as_u64().unwrap_or(0)
        .saturating_sub(chain["spent_txo_sum"].as_u64().unwrap_or(0));

    let mempool = &resp["mempool_stats"];
    let pending = mempool["funded_txo_sum"].as_u64().unwrap_or(0) as i64
        - mempool["spent_txo_sum"].as_u64().unwrap_or(0) as i64;

    Ok((confirmed, pending))
}

async fn fetch_sol_balance(client: &reqwest::Client, address: &str) -> Result<u64> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getBalance",
        "params": [address]
    });
    let resp: serde_json::Value = client
        .post(SOLANA_RPC)
        .json(&body)
        .send()
        .await
        .context("SOL balance request failed")?
        .json()
        .await
        .context("SOL balance response was not JSON")?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow!("Solana RPC error: {err}"));
    }
    Ok(resp["result"]["value"].as_u64().unwrap_or(0))
}

async fn fetch_stablecoin_balance(client: &reqwest::Client, address: &str) -> Result<f64> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "getTokenAccountsByOwner",
        "params": [
            address,
            { "mint": STABLECOIN_MINT },
            { "encoding": "jsonParsed" }
        ]
    });
    let resp: serde_json::Value = client
        .post(SOLANA_RPC)
        .json(&body)
        .send()
        .await
        .context("stablecoin balance request failed")?
        .json()
        .await
        .context("stablecoin balance response was not JSON")?;

    if let Some(err) = resp.get("error") {
        // If the mint doesn't exist on this cluster the RPC rejects the query
        // instead of returning an empty list. Report a zero balance.
        if err.to_string().contains("could not be unpacked") {
            return Ok(0.0);
        }
        return Err(anyhow!("Solana RPC error: {err}"));
    }

    let total = resp["result"]["value"]
        .as_array()
        .map(|accounts| {
            accounts
                .iter()
                .filter_map(|a| {
                    a["account"]["data"]["parsed"]["info"]["tokenAmount"]["uiAmount"].as_f64()
                })
                .sum()
        })
        .unwrap_or(0.0);
    Ok(total)
}
