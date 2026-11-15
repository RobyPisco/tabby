//! Sincronizzazione tra PC tramite una cartella che un servizio cloud tiene allineata
//! (Google Drive, OneDrive, Dropbox…) o una cartella di rete. Nessun server.
//!
//! Nella cartella condivisa:
//! - `tabby-sync.json`: segna che la cartella è di Tabby;
//! - `devices/<id-pc>.json`: lo stato di ogni PC; **ogni PC scrive solo il suo file**,
//!   quindi due PC non scrivono mai lo stesso file;
//! - `media/`: immagini e allegati citati dalle note.
//!
//! Il database SQLite resta sempre locale. L'unione delle note è in `syncdb.rs`.

use crate::{
    db::Db,
    reminders::Scheduler,
    settings::{Settings, SettingsState},
    syncdb::{now_ms, Snapshot},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Sender},
        Mutex,
    },
    time::{Duration, SystemTime},
};
use tauri::{AppHandle, Emitter, Manager, State};

const FORMAT: u32 = 1;
const MARKER: &str = "tabby-sync.json";
const SUBFOLDER: &str = "Tabby Sync";
/// Ogni quanto controllare se ci sono modifiche di questo PC da scrivere.
const TICK: Duration = Duration::from_secs(3);
/// Ogni quanti giri leggere i file degli altri PC (5 × 3 s = 15 s).
const READ_EVERY: u32 = 5;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
struct SyncPin {
    hash: Option<String>,
    salt: Option<String>,
    changed_at: i64,
}

/// Il file di un PC nella cartella condivisa.
#[derive(Serialize, Deserialize)]
struct DeviceFile {
    format: u32,
    device: String,
    /// Nome del computer, solo da mostrare.
    name: String,
    /// Ultima scrittura (millisecondi Unix).
    written_at: i64,
    pin: Option<SyncPin>,
    #[serde(flatten)]
    snapshot: Snapshot,
}

#[derive(Clone, Serialize, Default, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    /// Ultima modifica scritta da quel PC (secondi Unix).
    pub seen_at: i64,
    pub this_pc: bool,
}

#[derive(Clone, Serialize, Default, PartialEq)]
pub struct SyncStatus {
    pub dir: Option<String>,
    /// Ultimo giro completato (secondi Unix).
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub devices: Vec<DeviceInfo>,
}

pub struct SyncState {
    status: Mutex<SyncStatus>,
    wake: Mutex<Sender<()>>,
    /// La cartella è cambiata: si riparte da zero (rilettura di tutti i file).
    reset: AtomicBool,
}

impl SyncState {
    fn wake(&self) {
        let _ = self.wake.lock().unwrap().send(());
    }
}

/// La cartella da usare davvero partendo da quella scelta: se contiene già i dati di Tabby
/// (direttamente o in "Tabby Sync") si usa quella, altrimenti si crea "Tabby Sync" dentro.
/// Così su ogni PC basta scegliere la stessa cartella di Drive.
pub fn resolve_dir(chosen: &Path) -> PathBuf {
    if chosen.join(MARKER).is_file() {
        return chosen.to_path_buf();
    }
    let inner = chosen.join(SUBFOLDER);
    if inner.join(MARKER).is_file() || chosen.file_name().is_some_and(|n| n == SUBFOLDER) {
        return if chosen.file_name().is_some_and(|n| n == SUBFOLDER) { chosen.to_path_buf() } else { inner };
    }
    inner
}

fn prepare_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir.join("devices")).map_err(|e| format!("cartella non creata: {e}"))?;
    fs::create_dir_all(dir.join("media")).map_err(|e| format!("cartella non creata: {e}"))?;
    let marker = dir.join(MARKER);
    if !marker.is_file() {
        let text = serde_json::json!({ "app": "Tabby", "format": FORMAT }).to_string();
        fs::write(&marker, text).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Scrive prima un file temporaneo e poi lo rinomina: chi legge non trova mai un file a metà.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into())
}

/// Stato del giro di sincronizzazione (vive nel thread).
#[derive(Default)]
struct Engine {
    dir: Option<PathBuf>,
    /// Firma dell'ultimo stato scritto.
    written: Option<String>,
    /// File degli altri PC già letti: data di modifica e dimensione.
    seen: HashMap<PathBuf, (SystemTime, u64)>,
    /// Fin quando le modifiche di questo PC sono già nel suo file.
    shared_until: i64,
    devices: HashMap<String, DeviceInfo>,
    /// I file degli altri PC sono già stati letti almeno una volta.
    read_once: bool,
}

impl Engine {
    fn round(&mut self, app: &AppHandle, dir: &Path, settings: &Settings, read: bool) -> Result<(), String> {
        if self.dir.as_deref() != Some(dir) {
            *self = Engine { dir: Some(dir.to_path_buf()), shared_until: now_ms(), ..Engine::default() };
        }
        if !dir.is_dir() {
            return Err(format!(
                "Cartella non raggiungibile: {}. Google Drive (o il servizio che usi) è avviato?",
                dir.display()
            ));
        }
        prepare_dir(dir)?;
        let db = app.state::<Db>();
        let own = dir.join("devices").join(format!("{}.json", settings.device_id));

        if read || !self.read_once {
            self.read_others(app, dir, &own)?;
            copy_media(app, dir)?;
            self.read_once = true;
        }

        let settings = app.state::<SettingsState>().get();
        let signature = format!("{}|{}", db.sync_signature().map_err(|e| e.to_string())?, settings.pin_changed_at);
        if self.written.as_deref() != Some(signature.as_str()) || !own.is_file() {
            let taken_at = now_ms();
            let snapshot = db.sync_export().map_err(|e| e.to_string())?;
            let file = DeviceFile {
                format: FORMAT,
                device: settings.device_id.clone(),
                name: computer_name(),
                written_at: taken_at,
                pin: Some(SyncPin {
                    hash: settings.pin_hash.clone(),
                    salt: settings.pin_salt.clone(),
                    changed_at: settings.pin_changed_at,
                }),
                snapshot,
            };
            let bytes = serde_json::to_vec(&file).map_err(|e| e.to_string())?;
            write_atomic(&own, &bytes).map_err(|e| format!("scrittura nella cartella condivisa non riuscita: {e}"))?;
            self.written = Some(signature);
            self.shared_until = taken_at;
            self.devices.insert(
                settings.device_id.clone(),
                DeviceInfo { name: file.name, seen_at: taken_at / 1000, this_pc: true },
            );
            copy_media(app, dir)?;
        }
        Ok(())
    }

    /// Legge i file degli altri PC cambiati dall'ultima volta e ne applica le modifiche.
    fn read_others(&mut self, app: &AppHandle, dir: &Path, own: &Path) -> Result<(), String> {
        let db = app.state::<Db>();
        let entries = fs::read_dir(dir.join("devices")).map_err(|e| e.to_string())?;
        let mut changed = 0;
        let mut conflicts = 0;
        let mut pin_changed = None;
        for entry in entries.flatten() {
            let path = entry.path();
            if path == own || path.extension().is_none_or(|e| e != "json") {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let stamp = (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
            if self.seen.get(&path) == Some(&stamp) {
                continue;
            }
            // Un file ancora in arrivo (scaricato a metà) non si legge: si riprova al giro dopo.
            let Ok(file) = fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice::<DeviceFile>(&b).map_err(|e| e.to_string()))
            else {
                continue;
            };
            if file.format > FORMAT {
                return Err("un altro PC usa una versione più nuova di Tabby: aggiorna anche questo".into());
            }
            let applied = db.sync_apply(&file.snapshot, self.shared_until).map_err(|e| e.to_string())?;
            changed += applied.changed;
            conflicts += applied.conflicts;
            if let Some(pin) = file.pin {
                let state = app.state::<SettingsState>();
                if let Some(saved) = state.apply_remote_pin(pin.hash, pin.salt, pin.changed_at)? {
                    pin_changed = Some(saved);
                }
            }
            self.devices.insert(
                file.device,
                DeviceInfo { name: file.name, seen_at: file.written_at / 1000, this_pc: false },
            );
            self.seen.insert(path, stamp);
        }
        if changed > 0 {
            app.state::<Scheduler>().wake();
            crate::notify_changed_from(app, "sync");
        }
        if let Some(saved) = pin_changed {
            let _ = app.emit("settings-changed", saved);
        }
        if conflicts > 0 {
            eprintln!("sincronizzazione: {conflicts} note modificate su due PC, la versione di qui è nella cronologia");
        }
        Ok(())
    }

    fn device_list(&self) -> Vec<DeviceInfo> {
        let mut list: Vec<DeviceInfo> = self.devices.values().cloned().collect();
        list.sort_by(|a, b| b.this_pc.cmp(&a.this_pc).then_with(|| a.name.cmp(&b.name)));
        list
    }
}

/// Immagini e allegati: quelli citati dalle note che mancano da una parte vengono copiati
/// dall'altra. Il nome dipende dal contenuto, quindi un file non va mai sovrascritto.
fn copy_media(app: &AppHandle, dir: &Path) -> Result<(), String> {
    let local = crate::media::media_dir(app)?;
    let shared = dir.join("media");
    let names: HashSet<String> = app
        .state::<Db>()
        .all_texts()
        .map_err(|e| e.to_string())?
        .iter()
        .flat_map(|t| crate::export::referenced_media(t))
        .filter(|n| !n.contains(['/', '\\']) && !n.starts_with('.'))
        .collect();
    for name in names {
        let (here, there) = (local.join(&name), shared.join(&name));
        match (here.is_file(), there.is_file()) {
            (true, false) => {
                let _ = fs::copy(&here, &there);
            }
            (false, true) => {
                let _ = fs::copy(&there, &here);
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn start(app: &AppHandle) -> SyncState {
    let (tx, rx) = channel::<()>();
    let app = app.clone();
    std::thread::spawn(move || {
        let mut engine = Engine::default();
        let mut tick: u32 = 0;
        loop {
            let forced = rx.recv_timeout(TICK).is_ok();
            while rx.try_recv().is_ok() {}
            let Some(state) = app.try_state::<SyncState>() else { continue };
            if state.reset.swap(false, Ordering::SeqCst) {
                engine = Engine::default();
            }
            let settings = app.state::<SettingsState>().get();
            let status = match settings.sync_dir.as_deref() {
                None => {
                    engine = Engine::default();
                    SyncStatus::default()
                }
                Some(dir) => {
                    let dir = PathBuf::from(dir);
                    let result = engine.round(&app, &dir, &settings, forced || tick % READ_EVERY == 0);
                    let previous = state.status.lock().unwrap().clone();
                    SyncStatus {
                        dir: Some(dir.to_string_lossy().into_owned()),
                        last_sync: if result.is_ok() { Some(crate::db::now()) } else { previous.last_sync },
                        last_error: result.err(),
                        devices: engine.device_list(),
                    }
                }
            };
            let mut current = state.status.lock().unwrap();
            // "Ultima sincronizzazione" cambia a ogni giro: la UI si avvisa solo quando cambia altro,
            // o al massimo una volta al minuto.
            let notify = current.dir != status.dir
                || current.last_error != status.last_error
                || current.devices != status.devices
                || status.last_sync.unwrap_or(0) - current.last_sync.unwrap_or(0) >= 60;
            *current = status.clone();
            drop(current);
            if notify {
                let _ = app.emit("sync-status-changed", status);
            }
            tick = tick.wrapping_add(1);
        }
    });
    SyncState { status: Mutex::new(SyncStatus::default()), wake: Mutex::new(tx), reset: AtomicBool::new(false) }
}

#[tauri::command]
pub fn sync_status(state: State<SyncState>) -> SyncStatus {
    state.status.lock().unwrap().clone()
}

/// Collega la sincronizzazione alla cartella scelta (vedi `resolve_dir`).
#[tauri::command]
pub fn sync_choose_folder(app: AppHandle, path: String) -> Result<String, String> {
    let chosen = PathBuf::from(path.trim());
    if !chosen.is_dir() {
        return Err("la cartella scelta non esiste".into());
    }
    let dir = resolve_dir(&chosen);
    prepare_dir(&dir)?;
    let dir_text = dir.to_string_lossy().into_owned();
    let saved = app.state::<SettingsState>().set_sync_dir(Some(dir_text.clone()))?;
    let _ = app.emit("settings-changed", saved);
    let state = app.state::<SyncState>();
    state.reset.store(true, Ordering::SeqCst);
    state.wake();
    Ok(dir_text)
}

/// Smette di sincronizzare: le note restano su questo PC, la cartella condivisa non si tocca.
#[tauri::command]
pub fn sync_disconnect(app: AppHandle) -> Result<(), String> {
    let saved = app.state::<SettingsState>().set_sync_dir(None)?;
    let _ = app.emit("settings-changed", saved);
    let state = app.state::<SyncState>();
    state.reset.store(true, Ordering::SeqCst);
    state.wake();
    Ok(())
}

#[tauri::command]
pub fn sync_now(state: State<SyncState>) {
    state.wake();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tabby-sync-{name}-{}", now_ms()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_same_drive_folder_works_on_every_pc() {
        let drive = temp("drive");
        // Primo PC: sceglie la radice di Drive, Tabby crea "Tabby Sync".
        let first = resolve_dir(&drive);
        assert_eq!(first, drive.join(SUBFOLDER));
        prepare_dir(&first).unwrap();
        // Secondo PC: stessa scelta, trova la cartella esistente.
        assert_eq!(resolve_dir(&drive), first);
        // Chi sceglie direttamente "Tabby Sync" ottiene la stessa cartella, senza annidarla.
        assert_eq!(resolve_dir(&first), first);
        fs::remove_dir_all(&drive).unwrap();
    }

    #[test]
    fn device_file_round_trip() {
        let file = DeviceFile {
            format: FORMAT,
            device: "abc".into(),
            name: "PC-CUCINA".into(),
            written_at: 1,
            pin: Some(SyncPin { hash: Some("h".into()), salt: Some("s".into()), changed_at: 5 }),
            snapshot: Snapshot::default(),
        };
        let back: DeviceFile = serde_json::from_slice(&serde_json::to_vec(&file).unwrap()).unwrap();
        assert_eq!(back.name, "PC-CUCINA");
        assert_eq!(back.pin, file.pin);
    }
}
