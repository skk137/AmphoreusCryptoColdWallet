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
/// An ERC-20 token we track on an EVM chain.
pub struct EvmToken {
    pub symbol: &'static str,
    pub contract: &'static str,
    pub decimals: u32,
}

pub struct EvmChain {
    pub name: &'static str,
    pub rpc: &'static str,
    pub explorer_tx: &'static str,
    /// EIP-155 chain id, used when signing so a tx can't be replayed on
    /// another network.
    pub chain_id: u64,
    /// Native gas token symbol (POL on Polygon, ETH on Arbitrum). Fees are
    /// paid in this.
    pub native_symbol: &'static str,
    /// Native token decimals — always 18 on EVM chains.
    pub native_decimals: u32,
    /// ERC-20 tokens to display balances for on this chain.
    pub tokens: &'static [EvmToken],
}

pub const EVM_CHAINS: &[EvmChain] = &[
    EvmChain {
        name: "Polygon Network",
        rpc: "https://polygon-amoy-bor-rpc.publicnode.com",
        explorer_tx: "https://amoy.polygonscan.com/tx/",
        chain_id: 80002,
        native_symbol: "POL",
        native_decimals: 18,
        tokens: &[
            EvmToken {
                symbol: "USDC",
                contract: "0x41E94Eb019C0762f9Bfcf9Fb1E58725BfB0e7582",
                decimals: 6,
            },
            EvmToken {
                symbol: "LINK",
                contract: "0x53e0bca35ec356bd5dddfebbd1fc0fd03fabad39",
                decimals: 18,
            },
            EvmToken { symbol: "USDT", contract: "0xc2132D05D31c914a87C6611C10748AEb04B58e8F", decimals: 6 },
            EvmToken { symbol: "DAI",  contract: "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063", decimals: 18 },
            EvmToken { symbol: "WETH", contract: "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619", decimals: 18 },
            EvmToken { symbol: "WBTC", contract: "0x1BFD67037B42Cf73acF2047067bd4F2C47D9BfD6", decimals: 8 },
            EvmToken { symbol: "AAVE", contract: "0xD6DF932A45C0f255f85145f286eA0b292B21C90B", decimals: 18 },
            EvmToken { symbol: "UNI",  contract: "0xb33EaAd8d922B1083446DC23f610c2567fB5180f", decimals: 18 },
        ],
    },
    EvmChain {
        name: "Arbitrum Network",
        rpc: "https://sepolia-rollup.arbitrum.io/rpc",
        explorer_tx: "https://sepolia.arbiscan.io/tx/",
        chain_id: 421614,
        native_symbol: "ETH",
        native_decimals: 18,
        tokens: &[
            EvmToken {
                symbol: "USDC",
                contract: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d",
                decimals: 6,
            },
            // ARB governance token. This is the Arbitrum One (mainnet) contract;
            // there is no canonical ARB on Sepolia testnet, so it reads 0 here
            // until we flip to mainnet.
            EvmToken {
                symbol: "ARB",
                contract: "0x912CE59144191C1204E64559FE8253a0e49E6548",
                decimals: 18,
            },
        ],
    },
];

#[derive(Serialize)]
pub struct Addresses {
    pub btc: String,
    pub sol: String,
    pub evm: String,
    pub network: String,
}

pub fn find_evm_chain(name: &str) -> Option<&'static EvmChain> {
    EVM_CHAINS.iter().find(|c| c.name == name)
}

#[derive(Serialize)]
pub struct TokenBalance {
    pub symbol: String,
    pub amount: f64,
}

#[derive(Serialize)]
pub struct EvmBalance {
    pub network: String,
    pub tokens: Vec<TokenBalance>,
    pub native: f64,
    pub native_symbol: String,
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
        let mut tokens = Vec::new();
        for token in chain.tokens {
            let amount = fetch_evm_token(&client, chain.rpc, token, evm_address)
                .await
                .unwrap_or(0.0);
            tokens.push(TokenBalance {
                symbol: token.symbol.to_string(),
                amount,
            });
        }
        let native = fetch_evm_native(&client, chain, evm_address)
            .await
            .unwrap_or(0.0);
        evm.push(EvmBalance {
            network: chain.name.to_string(),
            tokens,
            native,
            native_symbol: chain.native_symbol.to_string(),
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

/// Reads an ERC-20 token balance via `eth_call` to the token contract's
/// `balanceOf(address)` (selector 0x70a08231). No secrets involved.
async fn fetch_evm_token(
    client: &reqwest::Client,
    rpc: &str,
    token: &EvmToken,
    address: &str,
) -> Result<f64> {
    let addr_hex = address.trim_start_matches("0x").to_lowercase();
    // ABI-encode: 4-byte selector + address left-padded to 32 bytes.
    let data = format!("0x70a08231{addr_hex:0>64}");
    let resp: serde_json::Value = client
        .post(rpc)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "eth_call",
            "params": [{ "to": token.contract, "data": data }, "latest"]
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
    Ok(raw as f64 / 10f64.powi(token.decimals as i32))
}

/// Reads the native gas-token balance (POL/ETH) via `eth_getBalance`. Native
/// EVM tokens always have 18 decimals (wei).
async fn fetch_evm_native(
    client: &reqwest::Client,
    chain: &EvmChain,
    address: &str,
) -> Result<f64> {
    let resp: serde_json::Value = client
        .post(chain.rpc)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "eth_getBalance",
            "params": [address, "latest"]
        }))
        .send()
        .await
        .context("EVM native balance request failed")?
        .json()
        .await
        .context("EVM native balance response was not JSON")?;

    if let Some(err) = resp.get("error") {
        return Err(anyhow!("EVM RPC error: {err}"));
    }
    let hex = resp["result"].as_str().unwrap_or("0x0");
    let raw = u128::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(0);
    Ok(raw as f64 / 1e18)
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
