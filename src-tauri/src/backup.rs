//! Backup automatico: una copia al giorno di database, immagini e impostazioni
//! in una cartella a scelta, tenendo solo le ultime `backup_keep`.
//!
//! Ogni backup è una cartella `tabby-AAAA-MM-GG_HHMMSS` autosufficiente; le immagini
//! uguali a quelle del backup precedente sono collegamenti fisici (hard link),
//! così sette backup non occupano sette volte lo spazio.

use chrono::{Local, NaiveDateTime, TimeZone};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
use tauri::{AppHandle, Manager};

use crate::db::{now, Db};
use crate::reminders::Scheduler;
use crate::settings::SettingsState;

const PREFIX: &str = "tabby-";
const NAME_FORMAT: &str = "%Y-%m-%d_%H%M%S";
/// Un backup al giorno.
const INTERVAL_SECS: i64 = 24 * 60 * 60;
/// Il primo controllo aspetta un po', per non rallentare l'avvio.
const FIRST_CHECK: Duration = Duration::from_secs(60);
const CHECK_EVERY: Duration = Duration::from_secs(60 * 60);

#[derive(Clone, Serialize)]
pub struct BackupInfo {
    pub name: String,
    pub created_at: i64,
    /// Note non nel cestino al momento del backup.
    pub notes: i64,
    pub media_files: usize,
}

pub fn backup_dir(app: &AppHandle) -> Result<PathBuf, String> {
    match app.state::<SettingsState>().get().backup_dir {
        Some(dir) => Ok(dir.into()),
        None => Ok(crate::data_dir(app).map_err(|e| e.to_string())?.join("backups")),
    }
}

fn created_at(name: &str) -> Option<i64> {
    let stamp = name.strip_prefix(PREFIX)?;
    let local = NaiveDateTime::parse_from_str(stamp, NAME_FORMAT).ok()?;
    Some(Local.from_local_datetime(&local).earliest()?.timestamp())
}

/// Nomi dei backup completi nella cartella, dal più recente.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("notes.db").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| created_at(n).is_some())
        .collect();
    names.sort_unstable_by(|a, b| b.cmp(a));
    names
}

fn info(dir: &Path, name: &str) -> BackupInfo {
    let path = dir.join(name);
    let notes = rusqlite::Connection::open_with_flags(path.join("notes.db"), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .and_then(|c| c.query_row("SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL", [], |r| r.get(0)))
        .unwrap_or(0);
    let media_files = fs::read_dir(path.join("media")).map_or(0, |d| d.count());
    BackupInfo {
        name: name.to_string(),
        created_at: created_at(name).unwrap_or(0),
        notes,
        media_files,
    }
}

pub fn list(app: &AppHandle) -> Result<Vec<BackupInfo>, String> {
    let dir = backup_dir(app)?;
    Ok(names(&dir).iter().map(|n| info(&dir, n)).collect())
}

/// Collegamento fisico al file uguale del backup precedente, oppure copia.
fn link_or_copy(src: &Path, previous: Option<&Path>, dest: &Path) -> std::io::Result<()> {
    if let Some(prev) = previous {
        let same = fs::metadata(prev)
            .and_then(|p| fs::metadata(src).map(|s| p.len() == s.len()))
            .unwrap_or(false);
        if same && fs::hard_link(prev, dest).is_ok() {
            return Ok(());
        }
    }
    fs::copy(src, dest).map(|_| ())
}

/// Esegue un backup e cancella i più vecchi oltre il limite (tranne `protect`).
fn run_protecting(app: &AppHandle, protect: Option<&str>) -> Result<BackupInfo, String> {
    let dir = backup_dir(app)?;
    let data = crate::data_dir(app).map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("cartella dei backup non disponibile: {e}"))?;
    let previous = names(&dir).into_iter().next().map(|n| dir.join(n));

    let mut name = format!("{PREFIX}{}", Local::now().format(NAME_FORMAT));
    if dir.join(&name).exists() {
        // Due backup nello stesso secondo (es. "Esegui ora" subito prima di un ripristino).
        thread::sleep(Duration::from_secs(1));
        name = format!("{PREFIX}{}", Local::now().format(NAME_FORMAT));
    }
    let tmp = dir.join(format!("{name}.tmp"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(tmp.join("media")).map_err(|e| e.to_string())?;

    let result = (|| -> Result<(), String> {
        app.state::<Db>()
            .copy_to(&tmp.join("notes.db"))
            .map_err(|e| format!("copia del database non riuscita: {e}"))?;
        let settings = data.join("settings.json");
        if settings.is_file() {
            fs::copy(&settings, tmp.join("settings.json")).map_err(|e| e.to_string())?;
        }
        for entry in fs::read_dir(data.join("media")).into_iter().flatten().flatten() {
            if !entry.path().is_file() {
                continue;
            }
            let file = entry.file_name();
            let prev = previous.as_ref().map(|p| p.join("media").join(&file));
            link_or_copy(&entry.path(), prev.as_deref(), &tmp.join("media").join(&file))
                .map_err(|e| format!("copia di {} non riuscita: {e}", file.to_string_lossy()))?;
        }
        fs::rename(&tmp, dir.join(&name)).map_err(|e| e.to_string())
    })();
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }

    let keep = app.state::<SettingsState>().get().backup_keep as usize;
    for old in names(&dir).iter().skip(keep) {
        if Some(old.as_str()) != protect {
            let _ = fs::remove_dir_all(dir.join(old));
        }
    }
    Ok(info(&dir, &name))
}

pub fn run(app: &AppHandle) -> Result<BackupInfo, String> {
    run_protecting(app, None)
}

/// Rimette note e immagini di un backup. Prima salva lo stato attuale in un nuovo backup,
/// così anche il ripristino si può annullare.
pub fn restore(app: &AppHandle, name: &str) -> Result<(), String> {
    if created_at(name).is_none() || name.contains(['/', '\\']) {
        return Err("backup non valido".into());
    }
    let dir = backup_dir(app)?;
    let source = dir.join(name);
    if !source.join("notes.db").is_file() {
        return Err("backup non trovato".into());
    }
    run_protecting(app, Some(name)).map_err(|e| format!("backup di sicurezza non riuscito: {e}"))?;

    // Le immagini non vengono mai sovrascritte: il nome è il contenuto, basta aggiungere quelle mancanti.
    let media = crate::media::media_dir(app)?;
    for entry in fs::read_dir(source.join("media")).into_iter().flatten().flatten() {
        let dest = media.join(entry.file_name());
        if !dest.exists() {
            fs::copy(entry.path(), &dest).map_err(|e| e.to_string())?;
        }
    }
    app.state::<Db>()
        .restore_from(&source.join("notes.db"))
        .map_err(|e| format!("ripristino del database non riuscito: {e}"))?;
    app.state::<Scheduler>().wake();
    crate::notify_changed_from(app, "restore");
    Ok(())
}

/// Controlla ogni ora se è passato un giorno dall'ultimo backup.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(FIRST_CHECK);
        loop {
            if app.state::<SettingsState>().get().backup_enabled {
                let last = backup_dir(&app)
                    .ok()
                    .and_then(|dir| names(&dir).first().and_then(|n| created_at(n)));
                if last.is_none_or(|t| now() - t >= INTERVAL_SECS) {
                    if let Err(e) = run(&app) {
                        eprintln!("backup automatico non riuscito: {e}");
                    }
                }
            }
            thread::sleep(CHECK_EVERY);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_parsed_and_sorted() {
        assert!(created_at("tabby-2026-10-02_093000").is_some());
        assert!(created_at("tabby-2026-10-02_093000.tmp").is_none());
        assert!(created_at("foto").is_none());

        let dir = std::env::temp_dir().join(format!("tabby-backup-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for n in ["tabby-2026-10-01_090000", "tabby-2026-10-02_090000", "tabby-2026-09-30_090000"] {
            fs::create_dir_all(dir.join(n)).unwrap();
            fs::write(dir.join(n).join("notes.db"), b"").unwrap();
        }
        // Backup interrotto: senza database non conta.
        fs::create_dir_all(dir.join("tabby-2026-10-03_090000")).unwrap();
        assert_eq!(
            names(&dir),
            ["tabby-2026-10-02_090000", "tabby-2026-10-01_090000", "tabby-2026-09-30_090000"]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn identical_media_is_hard_linked() {
        let dir = std::env::temp_dir().join(format!("tabby-link-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let (src, prev, dest) = (dir.join("a.png"), dir.join("prev.png"), dir.join("dest.png"));
        fs::write(&src, b"img").unwrap();
        fs::write(&prev, b"img").unwrap();
        link_or_copy(&src, Some(&prev), &dest).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"img");
        // Senza backup precedente si copia.
        let dest2 = dir.join("dest2.png");
        link_or_copy(&src, None, &dest2).unwrap();
        assert_eq!(fs::read(&dest2).unwrap(), b"img");
        fs::remove_dir_all(&dir).unwrap();
    }
}
