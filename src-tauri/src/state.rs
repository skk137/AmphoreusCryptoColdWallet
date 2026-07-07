use std::sync::Mutex;
use std::time::Instant;

use crate::crypto::secret::SecretString;

/// Holds the decrypted mnemonic in memory for the duration of an unlocked
/// session. Zeroized automatically on drop (via `SecretString`) whenever
/// the wallet is locked, the USB drive is removed, or the app exits.
pub struct UnlockedWallet {
    pub mnemonic: SecretString,
    pub usb_mount_point: String,
    pub unlocked_at: Instant,
}

#[derive(Default)]
pub struct AppState(pub Mutex<Option<UnlockedWallet>>);
