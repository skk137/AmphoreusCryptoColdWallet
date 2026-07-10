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
// Litecoin testnet (Esplora-compatible). Dogecoin has no usable testnet API,
// so it's mainnet read-only via Blockcypher.
pub const LTC_ESPLORA: &str = "https://litecoinspace.org/testnet/api";
pub const DOGE_API: &str = "https://api.blockcypher.com/v1/doge/main";

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
    EvmChain {
        name: "Base Network",
        rpc: "https://base-sepolia-rpc.publicnode.com",
        explorer_tx: "https://sepolia.basescan.org/tx/",
        chain_id: 84532,
        native_symbol: "ETH",
        native_decimals: 18,
        tokens: &[
            EvmToken {
                symbol: "USDC",
                contract: "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
                decimals: 6,
            },
            // WETH predeploy — exists on Base Sepolia (real balance possible).
            EvmToken {
                symbol: "WETH",
                contract: "0x4200000000000000000000000000000000000006",
                decimals: 18,
            },
            // DAI mainnet contract — placeholder, reads 0 on testnet.
            EvmToken {
                symbol: "DAI",
                contract: "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb",
                decimals: 18,
            },
        ],
    },
    EvmChain {
        name: "Optimism Network",
        rpc: "https://optimism-sepolia-rpc.publicnode.com",
        explorer_tx: "https://sepolia-optimism.etherscan.io/tx/",
        chain_id: 11155420,
        native_symbol: "ETH",
        native_decimals: 18,
        tokens: &[
            EvmToken {
                symbol: "USDC",
                contract: "0x5fD84259d66Cd46123540766Be93DFE6D43130D7",
                decimals: 6,
            },
            // WETH + OP governance token both exist on OP Sepolia (real).
            EvmToken {
                symbol: "WETH",
                contract: "0x4200000000000000000000000000000000000006",
                decimals: 18,
            },
            EvmToken {
                symbol: "OP",
                contract: "0x4200000000000000000000000000000000000042",
                decimals: 18,
            },
            // DAI mainnet contract — placeholder, reads 0 on testnet.
            EvmToken {
                symbol: "DAI",
                contract: "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1",
                decimals: 18,
            },
        ],
    },
    EvmChain {
        name: "Ethereum Network",
        rpc: "https://ethereum-sepolia-rpc.publicnode.com",
        explorer_tx: "https://sepolia.etherscan.io/tx/",
        chain_id: 11155111,
        native_symbol: "ETH",
        native_decimals: 18,
        // USDC is the Sepolia testnet contract (real). The rest are Ethereum
        // MAINNET ERC-20 contracts — placeholders that read 0 on Sepolia and
        // light up once we flip Ethereum Network to mainnet.
        tokens: &[
            EvmToken { symbol: "USDC",   contract: "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238", decimals: 6 },
            EvmToken { symbol: "USDT",   contract: "0xdAC17F958D2ee523a2206206994597C13D831ec7", decimals: 6 },
            EvmToken { symbol: "WBTC",   contract: "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599", decimals: 8 },
            EvmToken { symbol: "DAI",    contract: "0x6B175474E89094C44Da98b954EedeAC495271d0F", decimals: 18 },
            EvmToken { symbol: "WETH",   contract: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", decimals: 18 },
            EvmToken { symbol: "wstETH", contract: "0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0", decimals: 18 },
            EvmToken { symbol: "LINK",   contract: "0x514910771AF9Ca656af840dff83E8264EcF986CA", decimals: 18 },
            EvmToken { symbol: "UNI",    contract: "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984", decimals: 18 },
            EvmToken { symbol: "AAVE",   contract: "0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9", decimals: 18 },
            EvmToken { symbol: "LDO",    contract: "0x5A98FcBEA516Cf06857215779Fd812CA3beF1B32", decimals: 18 },
            EvmToken { symbol: "PEPE",   contract: "0x6982508145454Ce325dDbE47a25d4ec3d2311933", decimals: 18 },
            EvmToken { symbol: "SHIB",   contract: "0x95aD61b0a150d79219dCF64E1E6Cc01f0B64C4cE", decimals: 18 },
            EvmToken { symbol: "OKB",    contract: "0x75231F58b43240C9718Dd58B4967c5114342a86c", decimals: 18 },
            EvmToken { symbol: "PAXG",   contract: "0x45804880De22913dAFE09f4980848ECE6EcbAf78", decimals: 18 },
            EvmToken { symbol: "XAUT",   contract: "0x68749665FF8D2d112Fa859AA293F07A622782F38", decimals: 6 },
            EvmToken { symbol: "CRO",    contract: "0xA0b73E1Ff0B80914AB6fe0444E65848C4C34450b", decimals: 8 },
            EvmToken { symbol: "ENA",    contract: "0x57e114B691Db790C35207b2e685D4A43181e6061", decimals: 18 },
            EvmToken { symbol: "ONDO",   contract: "0xfAbA6f8e4a5E8Ab82F62fe7C39859FA577269BE3", decimals: 18 },
        ],
    },
    EvmChain {
        name: "Avalanche Network",
        rpc: "https://avalanche-fuji-c-chain-rpc.publicnode.com",
        explorer_tx: "https://testnet.snowtrace.io/tx/",
        chain_id: 43113,
        native_symbol: "AVAX",
        native_decimals: 18,
        tokens: &[EvmToken {
            symbol: "USDC",
            contract: "0x5425890298aed601595a70AB815c96711a31Bc65",
            decimals: 6,
        }],
    },
];

#[derive(Serialize)]
pub struct Addresses {
    pub btc: String,
    pub sol: String,
    pub evm: String,
    pub ltc: String,
    pub doge: String,
    pub network: String,
}

pub fn find_evm_chain(name: &str) -> Option<&'static EvmChain> {
    EVM_CHAINS.iter().find(|c| c.name == name)
}

#[derive(Serialize)]
pub struct HistoryTx {
    pub chain: String,
    pub txid: String,
    pub status: String, // "confirmed" | "pending" | "failed"
    pub explorer_url: String,
}

/// Recent transaction history for the BTC and SOL addresses, straight from the
/// public explorer/RPC endpoints (no API key, same sources as balances). EVM
/// history isn't included — raw RPC can't list by address; the UI links out to
/// the block explorer instead.
#[tauri::command]
pub async fn get_history(
    btc_address: String,
    sol_address: String,
) -> Result<Vec<HistoryTx>, String> {
    inner_get_history(&btc_address, &sol_address)
        .await
        .map_err(|e| e.to_string())
}

async fn inner_get_history(btc: &str, sol: &str) -> Result<Vec<HistoryTx>> {
    let client = reqwest::Client::new();
    let mut out = Vec::new();
    if let Ok(mut txs) = fetch_btc_history(&client, btc).await {
        out.append(&mut txs);
    }
    if let Ok(mut txs) = fetch_sol_history(&client, sol).await {
        out.append(&mut txs);
    }
    Ok(out)
}

async fn fetch_btc_history(client: &reqwest::Client, addr: &str) -> Result<Vec<HistoryTx>> {
    let arr: serde_json::Value = client
        .get(format!("{ESPLORA_BASE}/address/{addr}/txs"))
        .send()
        .await?
        .json()
        .await?;
    let mut v = Vec::new();
    if let Some(txs) = arr.as_array() {
        for tx in txs.iter().take(10) {
            let txid = tx["txid"].as_str().unwrap_or("").to_string();
            let confirmed = tx["status"]["confirmed"].as_bool().unwrap_or(false);
            v.push(HistoryTx {
                chain: "Bitcoin".into(),
                explorer_url: format!("https://blockstream.info/testnet/tx/{txid}"),
                status: if confirmed { "confirmed" } else { "pending" }.into(),
                txid,
            });
        }
    }
    Ok(v)
}

async fn fetch_sol_history(client: &reqwest::Client, addr: &str) -> Result<Vec<HistoryTx>> {
    let resp: serde_json::Value = client
        .post(SOLANA_RPC)
        .json(&json!({
            "jsonrpc": "2.0", "id": 1,
            "method": "getSignaturesForAddress",
            "params": [addr, { "limit": 10 }]
        }))
        .send()
        .await?
        .json()
        .await?;
    let mut v = Vec::new();
    if let Some(arr) = resp["result"].as_array() {
        for s in arr {
            let sig = s["signature"].as_str().unwrap_or("").to_string();
            let status = if !s["err"].is_null() {
                "failed"
            } else if s["confirmationStatus"].as_str() == Some("finalized") {
                "confirmed"
            } else {
                "pending"
            };
            v.push(HistoryTx {
                chain: "Solana".into(),
                explorer_url: format!("https://explorer.solana.com/tx/{sig}?cluster=devnet"),
                status: status.into(),
                txid: sig,
            });
        }
    }
    Ok(v)
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
    pub ltc_sats: u64,
    pub doge_koinu: u64,
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
    let ltc = derivation::derive_litecoin_address(&seed_bytes)?;
    let doge = derivation::derive_dogecoin_address(&seed_bytes)?;

    Ok(Addresses {
        btc,
        sol,
        evm,
        ltc,
        doge,
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
    ltc_address: String,
    doge_address: String,
) -> Result<Balances, String> {
    inner_get_balances(&btc_address, &sol_address, &evm_address, &ltc_address, &doge_address)
        .await
        .map_err(|e| e.to_string())
}

async fn inner_get_balances(
    btc_address: &str,
    sol_address: &str,
    evm_address: &str,
    ltc_address: &str,
    doge_address: &str,
) -> Result<Balances> {
    // Per-request timeout so one slow RPC (across 6+ EVM networks) can't hang
    // the whole balance load — a stalled chain just reports 0.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let (btc_sats, btc_pending_sats) = fetch_btc_balance(&client, btc_address).await?;
    let sol_lamports = fetch_sol_balance(&client, sol_address).await?;
    let stablecoin = fetch_stablecoin_balance(&client, sol_address).await?;
    let ltc_sats = fetch_ltc_balance(&client, ltc_address).await.unwrap_or(0);
    let doge_koinu = fetch_doge_balance(&client, doge_address).await.unwrap_or(0);

    // Fetch all EVM chains — and all tokens within each chain — concurrently,
    // so ~40 balance calls take ~1 round-trip instead of dozens sequentially.
    // A single failing RPC reports 0 for that entry rather than sinking it.
    let evm = futures::future::join_all(EVM_CHAINS.iter().map(|chain| {
        let client = &client;
        async move {
            let token_futs = chain.tokens.iter().map(|token| async move {
                TokenBalance {
                    symbol: token.symbol.to_string(),
                    amount: fetch_evm_token(client, chain.rpc, token, evm_address)
                        .await
                        .unwrap_or(0.0),
                }
            });
            let (tokens, native) = futures::join!(
                futures::future::join_all(token_futs),
                fetch_evm_native(client, chain, evm_address)
            );
            EvmBalance {
                network: chain.name.to_string(),
                tokens,
                native: native.unwrap_or(0.0),
                native_symbol: chain.native_symbol.to_string(),
                explorer_tx: chain.explorer_tx.to_string(),
            }
        }
    }))
    .await;

    Ok(Balances {
        btc_sats,
        btc_pending_sats,
        sol_lamports,
        stablecoin,
        stablecoin_label: STABLECOIN_LABEL.to_string(),
        evm,
        ltc_sats,
        doge_koinu,
    })
}

/// Litecoin testnet balance (litoshis) via the Esplora-compatible API.
async fn fetch_ltc_balance(client: &reqwest::Client, addr: &str) -> Result<u64> {
    let resp: serde_json::Value = client
        .get(format!("{LTC_ESPLORA}/address/{addr}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let s = &resp["chain_stats"];
    Ok(s["funded_txo_sum"].as_u64().unwrap_or(0).saturating_sub(s["spent_txo_sum"].as_u64().unwrap_or(0)))
}

/// Dogecoin mainnet balance (koinu) via Blockcypher.
async fn fetch_doge_balance(client: &reqwest::Client, addr: &str) -> Result<u64> {
    let resp: serde_json::Value = client
        .get(format!("{DOGE_API}/addrs/{addr}/balance"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp["balance"].as_u64().unwrap_or(0))
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
