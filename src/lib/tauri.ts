import { invoke } from "@tauri-apps/api/core";

export interface DriveInfo {
  mount_point: string;
  name: string;
  file_system: string;
  total_space: number;
  available_space: number;
  has_wallet: boolean;
}

export function listRemovableDrives(): Promise<DriveInfo[]> {
  return invoke("list_removable_drives");
}

export function localFolderInfo(path: string): Promise<DriveInfo> {
  return invoke("local_folder_info", { path });
}

export function createWallet(mountPoint: string, pin: string): Promise<string> {
  return invoke("create_wallet", { mountPoint, pin });
}

export function importWallet(mountPoint: string, pin: string, phrase: string): Promise<void> {
  return invoke("import_wallet", { mountPoint, pin, phrase });
}

export function unlockWallet(mountPoint: string, pin: string): Promise<void> {
  return invoke("unlock_wallet", { mountPoint, pin });
}

export function lockWallet(): Promise<void> {
  return invoke("lock_wallet");
}

export function walletStatus(): Promise<boolean> {
  return invoke("wallet_status");
}

export function walletSourcePresent(): Promise<boolean> {
  return invoke("wallet_source_present");
}

export interface Addresses {
  btc: string;
  sol: string;
  evm: string;
  ltc: string;
  doge: string;
  network: string;
}

export interface TokenBalance {
  symbol: string;
  amount: number;
}

export interface EvmBalance {
  network: string;
  tokens: TokenBalance[];
  native: number;
  native_symbol: string;
  explorer_tx: string;
}

export interface Balances {
  btc_sats: number;
  btc_pending_sats: number;
  sol_lamports: number;
  stablecoin: number;
  stablecoin_label: string;
  evm: EvmBalance[];
  ltc_sats: number;
  doge_koinu: number;
}

export function getAddresses(): Promise<Addresses> {
  return invoke("get_addresses");
}

export function getBalances(
  btcAddress: string,
  solAddress: string,
  evmAddress: string,
  ltcAddress: string,
  dogeAddress: string
): Promise<Balances> {
  return invoke("get_balances", { btcAddress, solAddress, evmAddress, ltcAddress, dogeAddress });
}

export interface HistoryTx {
  chain: string;
  txid: string;
  status: string; // "confirmed" | "pending" | "failed"
  explorer_url: string;
}

export function getHistory(btcAddress: string, solAddress: string): Promise<HistoryTx[]> {
  return invoke("get_history", { btcAddress, solAddress });
}

export function sendBtc(to: string, amountSats: number): Promise<string> {
  return invoke("send_btc", { to, amountSats });
}

export function sendSol(to: string, amountLamports: number): Promise<string> {
  return invoke("send_sol", { to, amountLamports });
}

export function sendUsdc(to: string, amountBase: number): Promise<string> {
  return invoke("send_usdc", { to, amountBase });
}

// asset = "native" or a token symbol; amount is a human decimal string.
export function sendEvm(network: string, asset: string, to: string, amount: string): Promise<string> {
  return invoke("send_evm", { network, asset, to, amount });
}

export interface BtcFeeEstimate {
  fee_sats: number;
  total_sats: number;
}

export function estimateBtcFee(amountSats: number): Promise<BtcFeeEstimate> {
  return invoke("estimate_btc_fee", { amountSats });
}
