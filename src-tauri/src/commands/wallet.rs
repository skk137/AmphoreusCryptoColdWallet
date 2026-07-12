use std::fs;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use tauri::State;

use super::usb::seed_file_path;
use crate::crypto::{encryption, seed};
use crate::state::{AppState, UnlockedWallet};

/// Generates a new mnemonic, encrypts it with the given PIN, and writes it
/// to the selected USB drive. Returns the plaintext mnemonic exactly once
/// so the UI can show the mandatory backup screen — after this call
/// returns, the phrase is not retrievable again except by unlocking with
/// the USB drive + PIN. The frontend must not persist this value anywhere.
#[tauri::command]
pub fn create_wallet(mount_point: String, pin: String) -> Result<String, String> {
    inner_create_wallet(&mount_point, &pin).map_err(|e| e.to_string())
}

fn inner_create_wallet(mount_point: &str, pin: &str) -> Result<String> {
    if pin.len() < 6 {
        bail!("PIN must be at least 6 characters");
    }

    let path = seed_file_path(mount_point);
    if path.exists() {
        bail!("a wallet already exists at this location");
    }

    let mnemonic = seed::generate_mnemonic()?;
    let encrypted = encryption::encrypt_phrase(&mnemonic, pin)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create coldwallet folder on USB")?;
    }
    let json = serde_json::to_string_pretty(&encrypted)?;
    fs::write(&path, json).context("failed to write seed file to USB")?;

    Ok(mnemonic.as_str().to_string())
}

/// Restores a wallet from a user-supplied mnemonic (instead of generating a
/// new one) and writes it to the USB drive the same way `create_wallet` does.
#[tauri::command]
pub fn import_wallet(mount_point: String, pin: String, phrase: String) -> Result<(), String> {
    inner_import_wallet(&mount_point, &pin, &phrase).map_err(|e| e.to_string())
}

fn inner_import_wallet(mount_point: &str, pin: &str, phrase: &str) -> Result<()> {
    if pin.len() < 6 {
        bail!("PIN must be at least 6 characters");
    }
    seed::validate_mnemonic(phrase)?;

    let path = seed_file_path(mount_point);
    if path.exists() {
        bail!("a wallet already exists at this location");
    }

    let mnemonic = crate::crypto::secret::SecretString::new(phrase.trim().to_string());
    let encrypted = encryption::encrypt_phrase(&mnemonic, pin)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create coldwallet folder on USB")?;
    }
    let json = serde_json::to_string_pretty(&encrypted)?;
    fs::write(&path, json).context("failed to write seed file to USB")?;
    Ok(())
}

/// Reads the encrypted seed file from the USB drive, decrypts it with the
/// given PIN, and holds the mnemonic in memory for this session.
#[tauri::command]
pub fn unlock_wallet(state: State<AppState>, mount_point: String, pin: String) -> Result<(), String> {
    inner_unlock(&state, &mount_point, &pin).map_err(|e| e.to_string())
}

fn inner_unlock(state: &AppState, mount_point: &str, pin: &str) -> Result<()> {
    let path = seed_file_path(mount_point);
    let json = fs::read_to_string(&path).context("no wallet found on this drive")?;
    let encrypted: encryption::EncryptedSeedFile =
        serde_json::from_str(&json).context("corrupted seed file")?;
    let mnemonic = encryption::decrypt_phrase(&encrypted, pin)?;
    seed::validate_mnemonic(mnemonic.as_str())?;

    let mut guard = state.0.lock().expect("wallet state mutex poisoned");
    *guard = Some(UnlockedWallet {
        mnemonic,
        usb_mount_point: mount_point.to_string(),
        unlocked_at: Instant::now(),
    });
    Ok(())
}

/// Wipes the in-memory mnemonic. Called explicitly by the user, on idle
/// timeout, or when the USB drive is detected as removed.
#[tauri::command]
pub fn lock_wallet(state: State<AppState>) {
    let mut guard = state.0.lock().expect("wallet state mutex poisoned");
    *guard = None;
}

#[tauri::command]
pub fn wallet_status(state: State<AppState>) -> bool {
    state.0.lock().expect("wallet state mutex poisoned").is_some()
}

/// True if the storage that holds the encrypted seed (the USB drive, or the
/// chosen local folder) is still accessible. The frontend polls this and
/// auto-locks the moment the USB is unplugged. Returns true when already
/// locked — there's nothing to guard. Using file existence (rather than a
/// "removable drive" check) means it works for the local-folder dev mode too.
#[tauri::command]
pub fn wallet_source_present(state: State<AppState>) -> bool {
    let guard = state.0.lock().expect("wallet state mutex poisoned");
    match guard.as_ref() {
        Some(w) => seed_file_path(&w.usb_mount_point).exists(),
        None => true,
    }
}

/// Re-encrypts the seed file on the USB with a new PIN. Verifies the current
/// PIN first (by decrypting the existing file), then writes a fresh
/// {salt,nonce,ciphertext} derived from `new_pin`. The wallet must be unlocked.
#[tauri::command]
pub fn change_pin(state: State<AppState>, old_pin: String, new_pin: String) -> Result<(), String> {
    inner_change_pin(&state, &old_pin, &new_pin).map_err(|e| e.to_string())
}

fn inner_change_pin(state: &AppState, old_pin: &str, new_pin: &str) -> Result<()> {
    if new_pin.chars().count() < 6 {
        bail!("Το νέο PIN πρέπει να έχει τουλάχιστον 6 χαρακτήρες");
    }
    let guard = state.0.lock().expect("wallet state mutex poisoned");
    let unlocked = guard.as_ref().ok_or_else(|| anyhow::anyhow!("wallet is locked"))?;
    let path = seed_file_path(&unlocked.usb_mount_point);

    let json = fs::read_to_string(&path).context("δεν βρέθηκε αρχείο wallet")?;
    let encrypted: encryption::EncryptedSeedFile =
        serde_json::from_str(&json).context("κατεστραμμένο αρχείο seed")?;
    // Verify the current PIN by decrypting; the recovered phrase is what we
    // re-encrypt (identical to the in-memory mnemonic).
    let phrase = encryption::decrypt_phrase(&encrypted, old_pin)
        .map_err(|_| anyhow::anyhow!("Λάθος τρέχον PIN"))?;

    let re = encryption::encrypt_phrase(&phrase, new_pin)?;
    let out = serde_json::to_string_pretty(&re)?;
    fs::write(&path, out).context("αποτυχία εγγραφής στο USB")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty directory standing in for a USB drive's mount point.
    fn fake_drive(name: &str) -> String {
        let dir = std::env::temp_dir().join(format!("cold-wallet-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().to_string()
    }

    #[test]
    fn create_then_unlock_round_trips_the_mnemonic() {
        let drive = fake_drive("create-unlock");
        let mnemonic = inner_create_wallet(&drive, "correct-horse").unwrap();

        let state = AppState::default();
        inner_unlock(&state, &drive, "correct-horse").unwrap();

        let guard = state.0.lock().unwrap();
        let unlocked = guard.as_ref().unwrap();
        assert_eq!(unlocked.mnemonic.as_str(), mnemonic);
        assert_eq!(unlocked.usb_mount_point, drive);
    }

    #[test]
    fn unlock_with_wrong_pin_fails_and_leaves_state_locked() {
        let drive = fake_drive("wrong-pin");
        inner_create_wallet(&drive, "correct-horse").unwrap();

        let state = AppState::default();
        let result = inner_unlock(&state, &drive, "incorrect-pin");
        assert!(result.is_err());
        assert!(state.0.lock().unwrap().is_none());
    }

    #[test]
    fn create_wallet_refuses_to_overwrite_existing_wallet() {
        let drive = fake_drive("no-overwrite");
        inner_create_wallet(&drive, "correct-horse").unwrap();
        let second = inner_create_wallet(&drive, "another-pin");
        assert!(second.is_err());
    }

    #[test]
    fn create_wallet_rejects_short_pin() {
        let drive = fake_drive("short-pin");
        let result = inner_create_wallet(&drive, "123");
        assert!(result.is_err());
    }

    #[test]
    fn import_then_unlock_recovers_the_supplied_phrase() {
        let drive = fake_drive("import");
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        inner_import_wallet(&drive, "correct-horse", phrase).unwrap();

        let state = AppState::default();
        inner_unlock(&state, &drive, "correct-horse").unwrap();
        let guard = state.0.lock().unwrap();
        assert_eq!(guard.as_ref().unwrap().mnemonic.as_str(), phrase);
    }
}
