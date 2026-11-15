//! Sincronizzazione tramite cartelle che un servizio cloud tiene allineate (Google Drive,
//! OneDrive, Dropbox…) o cartelle di rete. Nessun server.
//!
//! Due tipi di cartella condivisa:
//! - **personale**: tutte le note, tra i PC della stessa persona (`Settings::sync_dir`);
//! - **lista condivisa**: una sola cartella di note, con altre persone (`Settings::shares`).
//!
//! In ogni cartella condivisa:
//! - `tabby-sync.json`: tipo di cartella, eventuale cifratura, opzioni comuni a tutti i PC;
//! - `devices/<id-pc>.json`: lo stato di ogni PC; **ogni PC scrive solo il suo file**;
//! - `media/`: immagini e allegati citati dalle note;
//! - `Note da leggere/`: copie `.md` per il telefono (solo cartella personale non cifrata).
//!
//! Il database SQLite resta sempre locale. L'unione delle note è in `syncdb.rs`,
//! la cifratura in `crypto.rs`, le copie per il telefono in `phone.rs`.

use crate::{
    crypto::{self, Key},
    db::Db,
    phone,
    reminders::Scheduler,
    settings::{Settings, SettingsState, ShareConfig},
    syncdb::{now_ms, Snapshot},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
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
/// Errore mostrato quando la cartella è cifrata e su questo PC manca la frase segreta.
pub const NEED_PASSPHRASE: &str = "La cartella condivisa è cifrata: inserisci la frase segreta scelta sull'altro PC.";

const KIND_PERSONAL: &str = "personal";
const KIND_SHARE: &str = "share";

fn yes() -> bool {
    true
}

/// `tabby-sync.json`: vale per tutti i PC che usano la cartella.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Marker {
    app: String,
    format: u32,
    /// "personal" o "share" (i file della prima versione non lo avevano: personale).
    #[serde(default)]
    kind: String,
    /// Lista condivisa: `uid` e nome della cartella di note.
    #[serde(default)]
    share_folder: Option<String>,
    #[serde(default)]
    share_name: Option<String>,
    /// Cifratura: sale della frase segreta e controllo per riconoscerla.
    #[serde(default)]
    crypto: Option<CryptoInfo>,
    /// Copie `.md` leggibili dal telefono (solo cartella personale non cifrata).
    #[serde(default = "yes")]
    phone_copy: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct CryptoInfo {
    salt: String,
    check: String,
}

impl Marker {
    fn personal() -> Self {
        Marker {
            app: "Tabby".into(),
            format: FORMAT,
            kind: KIND_PERSONAL.into(),
            share_folder: None,
            share_name: None,
            crypto: None,
            phone_copy: true,
        }
    }

    fn share(folder_uid: &str, name: &str) -> Self {
        Marker {
            kind: KIND_SHARE.into(),
            share_folder: Some(folder_uid.into()),
            share_name: Some(name.into()),
            phone_copy: false,
            ..Marker::personal()
        }
    }

    fn is_share(&self) -> bool {
        self.kind == KIND_SHARE
    }
}

fn read_marker(dir: &Path) -> Result<Option<Marker>, String> {
    let path = dir.join(MARKER);
    if !path.is_file() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map(Some).map_err(|_| "il file tabby-sync.json è rovinato".to_string())
}

fn write_marker(dir: &Path, marker: &Marker) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(marker).map_err(|e| e.to_string())?;
    write_atomic(&dir.join(MARKER), &bytes).map_err(|e| e.to_string())
}

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

#[derive(Clone, Serialize, Default, PartialEq, Debug)]
pub struct DeviceInfo {
    pub name: String,
    /// Ultima modifica scritta da quel PC (secondi Unix).
    pub seen_at: i64,
    pub this_pc: bool,
}

#[derive(Clone, Serialize, Default, PartialEq, Debug)]
pub struct ShareStatus {
    pub folder_uid: String,
    pub name: String,
    pub dir: String,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub devices: Vec<DeviceInfo>,
}

#[derive(Clone, Serialize, Default, PartialEq, Debug)]
pub struct SyncStatus {
    pub dir: Option<String>,
    /// Ultimo giro completato (secondi Unix).
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
    pub devices: Vec<DeviceInfo>,
    /// La cartella personale è cifrata con una frase segreta.
    pub encrypted: bool,
    /// Cifrata, ma su questo PC manca (o è sbagliata) la frase segreta.
    pub needs_passphrase: bool,
    /// Copie per il telefono attive (opzione della cartella).
    pub phone_copy: bool,
    pub shares: Vec<ShareStatus>,
}

pub struct SyncState {
    status: Mutex<SyncStatus>,
    wake: Mutex<Sender<()>>,
    /// Cartelle o chiavi cambiate: si riparte da zero (rilettura di tutti i file).
    reset: AtomicBool,
}

impl SyncState {
    fn restart(&self) {
        self.reset.store(true, Ordering::SeqCst);
        let _ = self.wake.lock().unwrap().send(());
    }
}

/// La cartella da usare davvero partendo da quella scelta: se contiene già i dati di Tabby
/// (direttamente o in "Tabby Sync") si usa quella, altrimenti si crea "Tabby Sync" dentro.
/// Così su ogni PC basta scegliere la stessa cartella di Drive.
pub fn resolve_dir(chosen: &Path) -> PathBuf {
    if chosen.join(MARKER).is_file() || chosen.file_name().is_some_and(|n| n == SUBFOLDER) {
        return chosen.to_path_buf();
    }
    chosen.join(SUBFOLDER)
}

fn prepare_dirs(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir.join("devices")).map_err(|e| format!("cartella non creata: {e}"))?;
    fs::create_dir_all(dir.join("media")).map_err(|e| format!("cartella non creata: {e}"))
}

/// Scrive prima un file temporaneo e poi lo rinomina: chi legge non trova mai un file a metà.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC".into())
}

/// Nome di un file media nella cartella cifrata: non deve rivelare il nome dell'allegato.
fn hidden_media_name(name: &str) -> String {
    let digest = Sha256::digest(name.as_bytes());
    format!("{}.bin", digest.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

/// Chiave da usare per la cartella: `Ok(None)` se non è cifrata, errore se lo è e la frase
/// di questo PC manca o è sbagliata.
fn folder_key(marker: &Marker, settings: &Settings) -> Result<Option<Key>, String> {
    let Some(info) = &marker.crypto else { return Ok(None) };
    let key = settings.sync_key.as_deref().and_then(crypto::decode_key);
    match key {
        Some(key) if crypto::key_matches(&key, &info.check) => Ok(Some(key)),
        _ => Err(NEED_PASSPHRASE.into()),
    }
}

/// Di quale cartella si occupa un giro di sincronizzazione.
#[derive(Clone, PartialEq, Debug)]
enum Scope {
    Personal,
    Share { folder_uid: String },
}

/// Stato della sincronizzazione di una cartella (vive nel thread).
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
    /// Ultima scrittura (ms) dei file di ogni altro PC: chi ha scritto per ultimo aggiorna le copie per il telefono.
    others_written: HashMap<String, i64>,
    last_sync: Option<i64>,
    /// Lettura delle opzioni della cartella dall'ultimo giro (per lo stato).
    encrypted: bool,
    phone_copy: bool,
}

impl Engine {
    fn round(&mut self, app: &AppHandle, dir: &Path, scope: &Scope, read: bool) -> Result<(), String> {
        if self.dir.as_deref() != Some(dir) {
            *self = Engine { dir: Some(dir.to_path_buf()), shared_until: now_ms(), ..Engine::default() };
        }
        if !dir.is_dir() {
            return Err(format!(
                "Cartella non raggiungibile: {}. Google Drive (o il servizio che usi) è avviato?",
                dir.display()
            ));
        }
        let marker = match (read_marker(dir)?, scope) {
            (Some(m), _) => m,
            (None, Scope::Personal) => {
                let m = Marker::personal();
                prepare_dirs(dir)?;
                write_marker(dir, &m)?;
                m
            }
            (None, Scope::Share { .. }) => return Err("la cartella non contiene più la lista condivisa".into()),
        };
        match scope {
            Scope::Personal if marker.is_share() => {
                return Err("questa cartella è di una lista condivisa, non della sincronizzazione dei tuoi PC".into())
            }
            Scope::Share { folder_uid } if marker.share_folder.as_deref() != Some(folder_uid) => {
                return Err("la cartella contiene un'altra lista condivisa".into())
            }
            _ => {}
        }
        prepare_dirs(dir)?;
        self.encrypted = marker.crypto.is_some();
        self.phone_copy = marker.phone_copy;
        let settings = app.state::<SettingsState>().get();
        let key = folder_key(&marker, &settings)?;
        let own = dir.join("devices").join(format!("{}.json", settings.device_id));

        if read || !self.read_once {
            self.read_others(app, dir, &own, scope, key.as_ref())?;
            self.read_once = true;
        }

        let db = app.state::<Db>();
        let settings = app.state::<SettingsState>().get();
        let signature = format!(
            "{}|{}|{}",
            db.sync_signature().map_err(|e| e.to_string())?,
            settings.pin_changed_at,
            key.is_some()
        );
        if self.written.as_deref() != Some(signature.as_str()) || !own.is_file() {
            let taken_at = now_ms();
            let full = db.sync_export().map_err(|e| e.to_string())?;
            let (snapshot, pin) = match scope {
                Scope::Personal => (
                    full,
                    Some(SyncPin {
                        hash: settings.pin_hash.clone(),
                        salt: settings.pin_salt.clone(),
                        changed_at: settings.pin_changed_at,
                    }),
                ),
                Scope::Share { folder_uid } => (full.only_folder(folder_uid), None),
            };
            let file = DeviceFile {
                format: FORMAT,
                device: settings.device_id.clone(),
                name: computer_name(),
                written_at: taken_at,
                pin,
                snapshot,
            };
            let json = serde_json::to_vec(&file).map_err(|e| e.to_string())?;
            let bytes = match &key {
                Some(key) => crypto::encrypt(key, &json),
                None => json,
            };
            write_atomic(&own, &bytes).map_err(|e| format!("scrittura nella cartella condivisa non riuscita: {e}"))?;
            self.written = Some(signature);
            self.shared_until = taken_at;
            self.devices.insert(
                settings.device_id.clone(),
                DeviceInfo { name: file.name.clone(), seen_at: taken_at / 1000, this_pc: true },
            );
            // Copie per il telefono: le scrive solo il PC che ha scritto per ultimo, così due PC
            // non scrivono gli stessi file insieme (Google Drive creerebbe doppioni).
            if *scope == Scope::Personal {
                if key.is_none() && marker.phone_copy {
                    if self.others_written.values().all(|&t| t <= taken_at) {
                        phone::write(dir, &file.snapshot).map_err(|e| format!("copie per il telefono non scritte: {e}"))?;
                    }
                } else {
                    phone::remove(dir);
                }
            }
        }
        if read || self.last_sync.is_none() {
            self.copy_media(app, dir, scope, key.as_ref())?;
        }
        self.last_sync = Some(crate::db::now());
        Ok(())
    }

    /// Legge i file degli altri PC cambiati dall'ultima volta e ne applica le modifiche.
    fn read_others(&mut self, app: &AppHandle, dir: &Path, own: &Path, scope: &Scope, key: Option<&Key>) -> Result<(), String> {
        let db = app.state::<Db>();
        let entries = fs::read_dir(dir.join("devices")).map_err(|e| e.to_string())?;
        let mut changed = 0;
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
            let Ok(raw) = fs::read(&path) else { continue };
            let json = if crypto::is_encrypted(&raw) {
                let Some(key) = key else {
                    // File cifrato ma cartella non più cifrata: è di un PC che non si è ancora aggiornato.
                    continue;
                };
                match crypto::decrypt(key, &raw) {
                    Ok(plain) => plain,
                    Err(_) => continue,
                }
            } else {
                raw
            };
            // Un file ancora in arrivo (scaricato a metà) non si legge: si riprova al giro dopo.
            let Ok(file) = serde_json::from_slice::<DeviceFile>(&json) else { continue };
            if file.format > FORMAT {
                return Err("un altro PC usa una versione più nuova di Tabby: aggiorna anche questo".into());
            }
            let snapshot = match scope {
                Scope::Personal => file.snapshot,
                // Da una lista condivisa entra solo quella cartella, qualunque cosa ci sia nel file.
                Scope::Share { folder_uid } => file.snapshot.only_folder(folder_uid),
            };
            let applied = db.sync_apply(&snapshot, self.shared_until).map_err(|e| e.to_string())?;
            changed += applied.changed;
            if applied.conflicts > 0 {
                eprintln!("sincronizzazione: {} note modificate in due posti, la versione di qui è nella cronologia", applied.conflicts);
            }
            if let (Scope::Personal, Some(pin)) = (scope, file.pin) {
                let state = app.state::<SettingsState>();
                if let Some(saved) = state.apply_remote_pin(pin.hash, pin.salt, pin.changed_at)? {
                    pin_changed = Some(saved);
                }
            }
            self.others_written.insert(file.device.clone(), file.written_at);
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
        Ok(())
    }

    /// Immagini e allegati: quelli citati dalle note che mancano da una parte vengono copiati
    /// dall'altra. Il nome dipende dal contenuto, quindi un file non va mai sovrascritto.
    fn copy_media(&self, app: &AppHandle, dir: &Path, scope: &Scope, key: Option<&Key>) -> Result<(), String> {
        let local = crate::media::media_dir(app)?;
        let shared = dir.join("media");
        let db = app.state::<Db>();
        let texts: Vec<String> = match scope {
            Scope::Personal => db.all_texts().map_err(|e| e.to_string())?,
            Scope::Share { folder_uid } => db
                .sync_export()
                .map_err(|e| e.to_string())?
                .only_folder(folder_uid)
                .notes
                .into_iter()
                .map(|n| n.body)
                .collect(),
        };
        let names: HashSet<String> = texts
            .iter()
            .flat_map(|t| crate::export::referenced_media(t))
            .filter(|n| !n.contains(['/', '\\']) && !n.starts_with('.'))
            .collect();
        for name in &names {
            let here = local.join(name);
            let there = shared.join(match key {
                Some(_) => hidden_media_name(name),
                None => name.clone(),
            });
            match (here.is_file(), there.is_file(), key) {
                (true, false, None) => {
                    let _ = fs::copy(&here, &there);
                }
                (true, false, Some(key)) => {
                    if let Ok(bytes) = fs::read(&here) {
                        let _ = write_atomic(&there, &crypto::encrypt(key, &bytes));
                    }
                }
                (false, true, None) => {
                    let _ = fs::copy(&there, &here);
                }
                (false, true, Some(key)) => {
                    if let Ok(plain) = fs::read(&there).map_err(|e| e.to_string()).and_then(|b| crypto::decrypt(key, &b)) {
                        let _ = fs::write(&here, plain);
                    }
                }
                _ => {}
            }
        }
        // Passando da cartella in chiaro a cifrata (o viceversa) restano i file dell'altro tipo:
        // vanno tolti, altrimenti gli allegati resterebbero leggibili.
        for entry in fs::read_dir(&shared).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_hidden = name.ends_with(".bin") && name.len() == 68;
            if key.is_some() != is_hidden && !name.ends_with(".tmp") {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }

    fn device_list(&self) -> Vec<DeviceInfo> {
        let mut list: Vec<DeviceInfo> = self.devices.values().cloned().collect();
        list.sort_by(|a, b| b.this_pc.cmp(&a.this_pc).then_with(|| a.name.cmp(&b.name)));
        list
    }
}

pub fn start(app: &AppHandle) -> SyncState {
    let (tx, rx) = channel::<()>();
    let app = app.clone();
    std::thread::spawn(move || {
        let mut personal = Engine::default();
        let mut shares: HashMap<String, Engine> = HashMap::new();
        let mut errors: HashMap<String, Option<String>> = HashMap::new();
        let mut tick: u32 = 0;
        loop {
            let forced = rx.recv_timeout(TICK).is_ok();
            while rx.try_recv().is_ok() {}
            let Some(state) = app.try_state::<SyncState>() else { continue };
            if state.reset.swap(false, Ordering::SeqCst) {
                personal = Engine::default();
                shares.clear();
            }
            let read = forced || tick % READ_EVERY == 0;
            let settings = app.state::<SettingsState>().get();
            let mut status = SyncStatus::default();

            if let Some(dir) = settings.sync_dir.as_deref() {
                let dir = PathBuf::from(dir);
                let result = personal.round(&app, &dir, &Scope::Personal, read);
                status.dir = Some(dir.to_string_lossy().into_owned());
                status.needs_passphrase = result.as_ref().err().is_some_and(|e| e == NEED_PASSPHRASE);
                status.last_error = result.err();
                status.last_sync = personal.last_sync;
                status.devices = personal.device_list();
                status.encrypted = personal.encrypted;
                status.phone_copy = personal.phone_copy && !personal.encrypted;
            } else {
                personal = Engine::default();
            }

            shares.retain(|uid, _| settings.shares.iter().any(|s| &s.folder_uid == uid));
            for share in &settings.shares {
                let engine = shares.entry(share.folder_uid.clone()).or_default();
                let scope = Scope::Share { folder_uid: share.folder_uid.clone() };
                let result = engine.round(&app, Path::new(&share.dir), &scope, read);
                let name = app
                    .state::<Db>()
                    .folder_name_by_uid(&share.folder_uid)
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "(cartella eliminata)".into());
                status.shares.push(ShareStatus {
                    folder_uid: share.folder_uid.clone(),
                    name,
                    dir: share.dir.clone(),
                    last_sync: engine.last_sync,
                    last_error: result.err(),
                    devices: engine.device_list(),
                });
            }

            let mut current = state.status.lock().unwrap();
            // L'ora dell'ultimo controllo cambia a ogni giro: la UI si avvisa solo quando
            // cambia altro, o al massimo una volta al minuto.
            let mut key_now: Vec<Option<String>> = vec![status.last_error.clone()];
            key_now.extend(status.shares.iter().map(|s| s.last_error.clone()));
            let errors_changed = errors.get("all") != Some(&Some(format!("{key_now:?}")));
            errors.insert("all".into(), Some(format!("{key_now:?}")));
            let notify = errors_changed
                || current.dir != status.dir
                || current.devices != status.devices
                || current.encrypted != status.encrypted
                || current.phone_copy != status.phone_copy
                || current.shares.len() != status.shares.len()
                || current.shares.iter().zip(&status.shares).any(|(a, b)| a.devices != b.devices || a.name != b.name)
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

fn current_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.state::<SettingsState>()
        .get()
        .sync_dir
        .map(PathBuf::from)
        .ok_or_else(|| "la sincronizzazione non è attiva".to_string())
}

fn saved(app: &AppHandle, settings: Settings) {
    let _ = app.emit("settings-changed", settings);
    app.state::<SyncState>().restart();
}

#[tauri::command]
pub fn sync_status(state: State<SyncState>) -> SyncStatus {
    state.status.lock().unwrap().clone()
}

/// Collega la sincronizzazione dei tuoi PC alla cartella scelta (vedi `resolve_dir`).
#[tauri::command]
pub fn sync_choose_folder(app: AppHandle, path: String) -> Result<String, String> {
    let chosen = PathBuf::from(path.trim());
    if !chosen.is_dir() {
        return Err("la cartella scelta non esiste".into());
    }
    let dir = resolve_dir(&chosen);
    prepare_dirs(&dir)?;
    match read_marker(&dir)? {
        Some(m) if m.is_share() => {
            return Err("questa cartella è di una lista condivisa: scegline un'altra".into());
        }
        Some(_) => {}
        None => write_marker(&dir, &Marker::personal())?,
    }
    let dir_text = dir.to_string_lossy().into_owned();
    let settings = app.state::<SettingsState>().set_sync_dir(Some(dir_text.clone()))?;
    saved(&app, settings);
    Ok(dir_text)
}

/// Smette di sincronizzare: le note restano su questo PC, la cartella condivisa non si tocca.
#[tauri::command]
pub fn sync_disconnect(app: AppHandle) -> Result<(), String> {
    let settings = app.state::<SettingsState>().set_sync_dir(None)?;
    saved(&app, settings);
    Ok(())
}

#[tauri::command]
pub fn sync_now(state: State<SyncState>) {
    let _ = state.wake.lock().unwrap().send(());
}

/// Frase segreta: se la cartella è già cifrata la verifica, altrimenti cifra la cartella.
#[tauri::command]
pub fn sync_set_passphrase(app: AppHandle, passphrase: String) -> Result<(), String> {
    if passphrase.trim().chars().count() < crypto::MIN_PASSPHRASE {
        return Err(format!("la frase segreta deve avere almeno {} caratteri", crypto::MIN_PASSPHRASE));
    }
    let dir = current_dir(&app)?;
    let mut marker = read_marker(&dir)?.unwrap_or_else(Marker::personal);
    let key = match &marker.crypto {
        Some(info) => {
            let salt = crypto::decode(&info.salt).ok_or("il file tabby-sync.json è rovinato")?;
            let key = crypto::derive_key(&passphrase, &salt);
            if !crypto::key_matches(&key, &info.check) {
                return Err("frase segreta sbagliata: deve essere la stessa scelta sull'altro PC".into());
            }
            key
        }
        None => {
            let salt: [u8; 16] = crypto::random_bytes();
            let key = crypto::derive_key(&passphrase, &salt);
            marker.crypto = Some(CryptoInfo { salt: crypto::encode(&salt), check: crypto::make_check(&key) });
            write_marker(&dir, &marker)?;
            phone::remove(&dir);
            key
        }
    };
    let settings = app.state::<SettingsState>().set_sync_key(Some(crypto::encode(&key)))?;
    saved(&app, settings);
    Ok(())
}

/// Toglie la cifratura alla cartella (serve la frase giusta già inserita su questo PC).
#[tauri::command]
pub fn sync_remove_encryption(app: AppHandle) -> Result<(), String> {
    let dir = current_dir(&app)?;
    let mut marker = read_marker(&dir)?.ok_or("cartella di sincronizzazione non trovata")?;
    folder_key(&marker, &app.state::<SettingsState>().get())?;
    marker.crypto = None;
    write_marker(&dir, &marker)?;
    let settings = app.state::<SettingsState>().set_sync_key(None)?;
    saved(&app, settings);
    Ok(())
}

/// Accende o spegne le copie per il telefono (vale per tutti i PC della cartella).
#[tauri::command]
pub fn sync_set_phone_copy(app: AppHandle, enabled: bool) -> Result<(), String> {
    let dir = current_dir(&app)?;
    let mut marker = read_marker(&dir)?.ok_or("cartella di sincronizzazione non trovata")?;
    marker.phone_copy = enabled;
    write_marker(&dir, &marker)?;
    if !enabled {
        phone::remove(&dir);
    }
    app.state::<SyncState>().restart();
    Ok(())
}

/// Condivide una cartella di note con altre persone tramite una cartella cloud scelta
/// (che poi si condivide con loro da Google Drive, OneDrive o Dropbox).
#[tauri::command]
pub fn share_folder(app: AppHandle, folder_id: i64, path: String) -> Result<String, String> {
    let dir = PathBuf::from(path.trim());
    if !dir.is_dir() {
        return Err("la cartella scelta non esiste".into());
    }
    let (uid, name) = app
        .state::<Db>()
        .folder_uid(folder_id)
        .map_err(|e| e.to_string())?
        .ok_or("cartella di note non trovata")?;
    if let Some(personal) = app.state::<SettingsState>().get().sync_dir {
        if dir.starts_with(&personal) || Path::new(&personal).starts_with(&dir) {
            return Err("usa una cartella diversa da quella della sincronizzazione dei tuoi PC".into());
        }
    }
    match read_marker(&dir)? {
        Some(m) if m.share_folder.as_deref() == Some(uid.as_str()) => {}
        Some(_) => return Err("questa cartella è già usata da Tabby per altro: scegline una vuota".into()),
        None => {
            prepare_dirs(&dir)?;
            write_marker(&dir, &Marker::share(&uid, &name))?;
        }
    }
    let settings = app
        .state::<SettingsState>()
        .set_share(ShareConfig { folder_uid: uid, dir: dir.to_string_lossy().into_owned() })?;
    saved(&app, settings);
    Ok(name)
}

/// Si unisce a una lista condivisa da un'altra persona: la sua cartella compare qui.
#[tauri::command]
pub fn share_join(app: AppHandle, path: String) -> Result<String, String> {
    let dir = PathBuf::from(path.trim());
    let marker = read_marker(&dir)?
        .filter(Marker::is_share)
        .ok_or("in questa cartella non c'è una lista condivisa di Tabby: scegli la cartella che ti hanno condiviso")?;
    let uid = marker.share_folder.clone().ok_or("il file tabby-sync.json è rovinato")?;
    let name = marker.share_name.clone().unwrap_or_else(|| "Lista condivisa".into());
    if marker.crypto.is_some() {
        return Err("le liste condivise cifrate non sono ancora supportate".into());
    }
    app.state::<Db>().ensure_folder(&uid, &name).map_err(|e| e.to_string())?;
    crate::notify_changed_from(&app, "sync");
    let settings = app
        .state::<SettingsState>()
        .set_share(ShareConfig { folder_uid: uid, dir: dir.to_string_lossy().into_owned() })?;
    saved(&app, settings);
    Ok(name)
}

/// Smette di condividere una cartella: le note restano qui, la cartella cloud non si tocca.
#[tauri::command]
pub fn share_leave(app: AppHandle, folder_uid: String) -> Result<(), String> {
    let settings = app.state::<SettingsState>().remove_share(&folder_uid)?;
    saved(&app, settings);
    Ok(())
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
        prepare_dirs(&first).unwrap();
        write_marker(&first, &Marker::personal()).unwrap();
        // Secondo PC: stessa scelta, trova la cartella esistente.
        assert_eq!(resolve_dir(&drive), first);
        // Chi sceglie direttamente "Tabby Sync" ottiene la stessa cartella, senza annidarla.
        assert_eq!(resolve_dir(&first), first);
        fs::remove_dir_all(&drive).unwrap();
    }

    #[test]
    fn old_markers_are_personal_with_phone_copies() {
        let m: Marker = serde_json::from_str(r#"{ "app": "Tabby", "format": 1 }"#).unwrap();
        assert!(!m.is_share());
        assert!(m.phone_copy);
        assert!(m.crypto.is_none());
    }

    #[test]
    fn the_key_is_needed_for_an_encrypted_folder() {
        let salt: [u8; 16] = crypto::random_bytes();
        let key = crypto::derive_key("frase segreta lunga", &salt);
        let marker = Marker {
            crypto: Some(CryptoInfo { salt: crypto::encode(&salt), check: crypto::make_check(&key) }),
            ..Marker::personal()
        };
        let mut settings = Settings::default();
        assert_eq!(folder_key(&marker, &settings), Err(NEED_PASSPHRASE.to_string()));
        settings.sync_key = Some(crypto::encode(&crypto::derive_key("frase sbagliata!", &salt)));
        assert!(folder_key(&marker, &settings).is_err());
        settings.sync_key = Some(crypto::encode(&key));
        assert_eq!(folder_key(&marker, &settings), Ok(Some(key)));
        assert_eq!(folder_key(&Marker::personal(), &Settings::default()), Ok(None));
    }

    #[test]
    fn hidden_media_names_do_not_reveal_the_file() {
        let name = hidden_media_name("0123456789abcdef-Fattura luglio.pdf");
        assert!(name.ends_with(".bin") && name.len() == 68);
        assert!(!name.contains("Fattura"));
    }

    /// Solo per la prova a mano (`e2e.py`): cifra la cartella `TABBY_E2E_DIR` e stampa la chiave.
    #[test]
    #[ignore]
    fn e2e_encrypt_fixture() {
        let dir = PathBuf::from(std::env::var("TABBY_E2E_DIR").expect("TABBY_E2E_DIR"));
        let mut marker = read_marker(&dir).unwrap().unwrap();
        let salt: [u8; 16] = crypto::random_bytes();
        let key = crypto::derive_key("frase segreta di prova", &salt);
        marker.crypto = Some(CryptoInfo { salt: crypto::encode(&salt), check: crypto::make_check(&key) });
        write_marker(&dir, &marker).unwrap();
        println!("KEY={}", crypto::encode(&key));
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
