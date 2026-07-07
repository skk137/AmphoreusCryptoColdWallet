use std::path::{Path, PathBuf};

use serde::Serialize;
use sysinfo::Disks;

const SEED_FILE_RELATIVE_PATH: [&str; 2] = ["coldwallet", "seed.enc"];

#[derive(Serialize)]
pub struct DriveInfo {
    pub mount_point: String,
    pub name: String,
    pub file_system: String,
    pub total_space: u64,
    pub available_space: u64,
    pub has_wallet: bool,
}

/// Lists removable drives so the user can pick which USB stick holds (or
/// will hold) the encrypted seed file. Only removable media is listed —
/// this app never writes key material to the internal disk.
#[tauri::command]
pub fn list_removable_drives() -> Vec<DriveInfo> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| d.is_removable())
        .map(|d| {
            let mount_point = d.mount_point().to_string_lossy().to_string();
            DriveInfo {
                has_wallet: seed_file_path(&mount_point).exists(),
                mount_point,
                name: d.name().to_string_lossy().to_string(),
                file_system: d.file_system().to_string_lossy().to_string(),
                total_space: d.total_space(),
                available_space: d.available_space(),
            }
        })
        .collect()
}

/// Returns the same info as `list_removable_drives` but for a user-chosen
/// local folder. Development/testing fallback only — a folder on the
/// internal disk defeats the purpose of cold storage, and the UI labels
/// it as not recommended.
#[tauri::command]
pub fn local_folder_info(path: String) -> Result<DriveInfo, String> {
    let folder = Path::new(&path);
    if !folder.is_dir() {
        return Err("ο φάκελος δεν υπάρχει".to_string());
    }
    Ok(DriveInfo {
        has_wallet: seed_file_path(&path).exists(),
        mount_point: path.clone(),
        name: folder
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone()),
        file_system: String::new(),
        total_space: 0,
        available_space: 0,
    })
}

/// True if `mount_point` is still present among the currently attached
/// removable drives. Used to detect USB removal mid-session so the caller
/// can lock the wallet.
pub fn is_drive_present(mount_point: &str) -> bool {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .any(|d| d.mount_point().to_string_lossy() == mount_point)
}

pub fn seed_file_path(mount_point: &str) -> PathBuf {
    let mut path = Path::new(mount_point).to_path_buf();
    for segment in SEED_FILE_RELATIVE_PATH {
        path.push(segment);
    }
    path
}
