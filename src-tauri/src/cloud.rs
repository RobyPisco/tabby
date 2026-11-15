//! Cartelle dei servizi cloud installati sul PC (Google Drive per desktop, OneDrive,
//! Dropbox), da proporre per la sincronizzazione: così non bisogna cercarle a mano.
//! Tabby non parla con questi servizi: scrive nella loro cartella e ci pensa la loro app.

use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize, PartialEq, Debug)]
pub struct CloudFolder {
    /// "Google Drive", "OneDrive" o "Dropbox".
    pub service: String,
    pub path: String,
}

/// Nome della cartella principale di Google Drive per desktop nelle varie lingue di Windows.
const GOOGLE_MY_DRIVE: [&str; 6] = ["Il mio Drive", "My Drive", "Mon Drive", "Mi unidad", "Meine Ablage", "Meu Drive"];

fn push(list: &mut Vec<CloudFolder>, service: &str, path: PathBuf) {
    if !path.is_dir() {
        return;
    }
    let text = path.to_string_lossy().into_owned();
    if !list.iter().any(|f| f.path.eq_ignore_ascii_case(&text)) {
        list.push(CloudFolder { service: service.into(), path: text });
    }
}

/// Lettere delle unità da guardare per Google Drive per desktop (che crea un'unità, di solito G:).
#[cfg(windows)]
fn drive_letters() -> Vec<char> {
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    use windows_sys::Win32::System::Diagnostics::Debug::{SetErrorMode, SEM_FAILCRITICALERRORS};
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;
    // Niente finestre "Inserire un disco" per i lettori di schede vuoti.
    unsafe { SetErrorMode(SEM_FAILCRITICALERRORS) };
    let mask = unsafe { GetLogicalDrives() };
    (0..26u32)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| (b'A' + i as u8) as char)
        .filter(|&letter| letter > 'C')
        .filter(|&letter| {
            let root: Vec<u16> = format!("{letter}:\\").encode_utf16().chain(Some(0)).collect();
            let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
            kind == DRIVE_FIXED || kind == DRIVE_REMOTE
        })
        .collect()
}

#[cfg(not(windows))]
fn drive_letters() -> Vec<char> {
    vec![]
}

/// Percorso di Dropbox scritto dalla sua app in `info.json`.
fn dropbox_from_info(text: &str) -> Vec<PathBuf> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return vec![] };
    ["personal", "business"]
        .iter()
        .filter_map(|k| json.get(k)?.get("path")?.as_str().map(PathBuf::from))
        .collect()
}

pub fn find() -> Vec<CloudFolder> {
    let mut list = Vec::new();
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from);

    for letter in drive_letters() {
        for name in GOOGLE_MY_DRIVE {
            push(&mut list, "Google Drive", PathBuf::from(format!("{letter}:\\{name}")));
        }
    }
    if let Some(home) = &home {
        // Versione vecchia ("Backup e sincronizzazione") o Drive montato come cartella.
        push(&mut list, "Google Drive", home.join("Google Drive"));
        for name in GOOGLE_MY_DRIVE {
            push(&mut list, "Google Drive", home.join("Google Drive").join(name));
        }
    }

    for var in ["OneDriveConsumer", "OneDrive", "OneDriveCommercial"] {
        if let Some(path) = std::env::var_os(var) {
            push(&mut list, "OneDrive", PathBuf::from(path));
        }
    }

    for base in ["LOCALAPPDATA", "APPDATA"] {
        if let Some(dir) = std::env::var_os(base) {
            if let Ok(text) = std::fs::read_to_string(Path::new(&dir).join("Dropbox").join("info.json")) {
                for path in dropbox_from_info(&text) {
                    push(&mut list, "Dropbox", path);
                }
            }
        }
    }
    if let Some(home) = &home {
        push(&mut list, "Dropbox", home.join("Dropbox"));
    }
    list
}

/// Il servizio cloud che sincronizza questa cartella, se è dentro una di quelle trovate.
pub fn service_of(dir: &Path, folders: &[CloudFolder]) -> Option<String> {
    let dir = dir.to_string_lossy().to_lowercase();
    folders
        .iter()
        .find(|f| {
            let base = f.path.to_lowercase();
            dir == base || dir.starts_with(&format!("{}\\", base.trim_end_matches('\\')))
        })
        .map(|f| f.service.clone())
}

#[tauri::command]
pub fn sync_cloud_folders() -> Vec<CloudFolder> {
    find()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mano: `cargo test show_cloud_folders -- --ignored --nocapture` mostra cosa trova su questo PC.
    #[test]
    #[ignore]
    fn show_cloud_folders() {
        for f in find() {
            println!("{} -> {}", f.service, f.path);
        }
    }

    #[test]
    fn dropbox_info_is_read() {
        let paths = dropbox_from_info(r#"{"personal": {"path": "C:\\Users\\R\\Dropbox", "host": 1}}"#);
        assert_eq!(paths, vec![PathBuf::from("C:\\Users\\R\\Dropbox")]);
        assert!(dropbox_from_info("non json").is_empty());
    }

    #[test]
    fn the_service_is_recognised_from_the_path() {
        let folders = vec![
            CloudFolder { service: "Google Drive".into(), path: "G:\\Il mio Drive".into() },
            CloudFolder { service: "OneDrive".into(), path: "C:\\Users\\R\\OneDrive".into() },
        ];
        assert_eq!(service_of(Path::new("G:\\Il mio Drive\\Tabby Sync"), &folders).as_deref(), Some("Google Drive"));
        assert_eq!(service_of(Path::new("c:\\users\\r\\onedrive"), &folders).as_deref(), Some("OneDrive"));
        assert_eq!(service_of(Path::new("C:\\Users\\R\\OneDriveVecchio"), &folders), None);
        assert_eq!(service_of(Path::new("D:\\Note"), &folders), None);
    }
}
