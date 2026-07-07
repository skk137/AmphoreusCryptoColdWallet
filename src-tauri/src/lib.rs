mod commands;
mod crypto;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::usb::list_removable_drives,
            commands::usb::local_folder_info,
            commands::wallet::create_wallet,
            commands::wallet::import_wallet,
            commands::wallet::unlock_wallet,
            commands::wallet::lock_wallet,
            commands::wallet::wallet_status,
            commands::wallet::wallet_source_present,
            commands::chain::get_addresses,
            commands::chain::get_balances,
            commands::chain::get_history,
            commands::send::send_btc,
            commands::send::send_sol,
            commands::send::send_usdc,
            commands::send::estimate_btc_fee,
            commands::evm::send_evm,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
