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

export interface Addresses {
  btc: string;
  sol: string;
  evm: string;
  network: string;
}

export interface EvmBalance {
  network: string;
  usdc: number;
  explorer_tx: string;
}

export interface Balances {
  btc_sats: number;
  btc_pending_sats: number;
  sol_lamports: number;
  stablecoin: number;
  stablecoin_label: string;
  evm: EvmBalance[];
}

export function getAddresses(): Promise<Addresses> {
  return invoke("get_addresses");
}

export function getBalances(
  btcAddress: string,
  solAddress: string,
  evmAddress: string
): Promise<Balances> {
  return invoke("get_balances", { btcAddress, solAddress, evmAddress });
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
