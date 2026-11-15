//! Parte "database" della sincronizzazione tra PC (vedi `sync.rs` e docs/SYNC-E-STORE.md).
//!
//! Ogni nota e cartella ha un identificativo globale (`uid`, uguale su tutti i PC) e un
//! `changed_at` in millisecondi, aggiornato da trigger a ogni modifica che va condivisa.
//! Le eliminazioni definitive lasciano una "lapide" in `sync_tombstones`.
//! Tra due versioni della stessa nota vince quella con `changed_at` più recente; il testo
//! che perde, se era stato modificato qui e non ancora condiviso, finisce nella cronologia.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::Db;

/// Millisecondi Unix calcolati da SQLite (per i trigger).
const NOW_MS: &str = "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)";

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn add_column(conn: &Connection, table: &str, column: &str, definition: &str) -> rusqlite::Result<()> {
    let exists: bool = conn.query_row(
        &format!("SELECT EXISTS (SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1)"),
        [column],
        |r| r.get(0),
    )?;
    if !exists {
        conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"), [])?;
    }
    Ok(())
}

/// Colonne, tabelle e trigger della sincronizzazione. Si può chiamare più volte.
pub fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    add_column(conn, "notes", "uid", "TEXT")?;
    add_column(conn, "notes", "changed_at", "INTEGER NOT NULL DEFAULT 0")?;
    add_column(conn, "folders", "uid", "TEXT")?;
    add_column(conn, "folders", "changed_at", "INTEGER NOT NULL DEFAULT 0")?;
    // Database di prima della sincronizzazione: le note di benvenuto mai toccate prendono
    // l'uid fisso, così non si duplicano tra i PC.
    for (uid, title, body, _) in crate::db::SEED {
        conn.execute(
            "UPDATE notes SET uid = ?1, changed_at = 1 WHERE uid IS NULL AND title = ?2 AND body = ?3
                 AND NOT EXISTS (SELECT 1 FROM notes WHERE uid = ?1)
                 AND id = (SELECT MIN(id) FROM notes WHERE uid IS NULL AND title = ?2 AND body = ?3)",
            params![uid, title, body],
        )?;
    }
    conn.execute_batch(&format!(
        "UPDATE notes SET uid = lower(hex(randomblob(16))) WHERE uid IS NULL;
         UPDATE notes SET changed_at = updated_at * 1000 WHERE changed_at = 0;
         UPDATE folders SET uid = lower(hex(randomblob(16))) WHERE uid IS NULL;
         UPDATE folders SET changed_at = {NOW_MS} WHERE changed_at = 0;
         CREATE UNIQUE INDEX IF NOT EXISTS notes_uid ON notes(uid);
         CREATE UNIQUE INDEX IF NOT EXISTS folders_uid ON folders(uid);

         CREATE TABLE IF NOT EXISTS sync_tombstones (
             uid  TEXT PRIMARY KEY,
             kind TEXT NOT NULL,
             at   INTEGER NOT NULL
         );
         -- Una riga qui durante l'applicazione delle modifiche di un altro PC:
         -- i trigger dei promemoria non devono segnarle come modifiche di questo PC.
         CREATE TABLE IF NOT EXISTS sync_applying (x INTEGER);

         CREATE TRIGGER IF NOT EXISTS notes_sync_ai AFTER INSERT ON notes
         WHEN NEW.uid IS NULL OR NEW.changed_at = 0 BEGIN
             UPDATE notes SET uid = COALESCE(NEW.uid, lower(hex(randomblob(16)))),
                              changed_at = CASE WHEN NEW.changed_at > 0 THEN NEW.changed_at ELSE {NOW_MS} END
             WHERE id = NEW.id;
         END;
         CREATE TRIGGER IF NOT EXISTS notes_sync_au
         AFTER UPDATE OF title, body, color, archived, deleted_at, pinned, folder_id, locked ON notes
         WHEN NEW.changed_at = OLD.changed_at AND (
             NEW.title IS NOT OLD.title OR NEW.body IS NOT OLD.body OR NEW.color IS NOT OLD.color
             OR NEW.archived IS NOT OLD.archived OR NEW.deleted_at IS NOT OLD.deleted_at
             OR NEW.pinned IS NOT OLD.pinned OR NEW.folder_id IS NOT OLD.folder_id
             OR NEW.locked IS NOT OLD.locked) BEGIN
             UPDATE notes SET changed_at = MAX({NOW_MS}, OLD.changed_at + 1) WHERE id = NEW.id;
         END;
         CREATE TRIGGER IF NOT EXISTS notes_sync_ad AFTER DELETE ON notes
         WHEN OLD.uid IS NOT NULL BEGIN
             INSERT OR REPLACE INTO sync_tombstones (uid, kind, at) VALUES (OLD.uid, 'note', {NOW_MS});
         END;

         CREATE TRIGGER IF NOT EXISTS reminders_sync_ai AFTER INSERT ON reminders
         WHEN NOT EXISTS (SELECT 1 FROM sync_applying) BEGIN
             UPDATE notes SET changed_at = MAX({NOW_MS}, changed_at + 1) WHERE id = NEW.note_id;
         END;
         CREATE TRIGGER IF NOT EXISTS reminders_sync_au AFTER UPDATE OF due_at, repeat, snoozed_until ON reminders
         WHEN NOT EXISTS (SELECT 1 FROM sync_applying) AND (NEW.due_at IS NOT OLD.due_at
             OR NEW.repeat IS NOT OLD.repeat OR NEW.snoozed_until IS NOT OLD.snoozed_until) BEGIN
             UPDATE notes SET changed_at = MAX({NOW_MS}, changed_at + 1) WHERE id = NEW.note_id;
         END;
         CREATE TRIGGER IF NOT EXISTS reminders_sync_ad AFTER DELETE ON reminders
         WHEN NOT EXISTS (SELECT 1 FROM sync_applying) BEGIN
             UPDATE notes SET changed_at = MAX({NOW_MS}, changed_at + 1) WHERE id = OLD.note_id;
         END;

         CREATE TRIGGER IF NOT EXISTS folders_sync_ai AFTER INSERT ON folders
         WHEN NEW.uid IS NULL OR NEW.changed_at = 0 BEGIN
             UPDATE folders SET uid = COALESCE(NEW.uid, lower(hex(randomblob(16)))),
                                changed_at = CASE WHEN NEW.changed_at > 0 THEN NEW.changed_at ELSE {NOW_MS} END
             WHERE id = NEW.id;
         END;
         CREATE TRIGGER IF NOT EXISTS folders_sync_au AFTER UPDATE OF name ON folders
         WHEN NEW.changed_at = OLD.changed_at AND NEW.name IS NOT OLD.name BEGIN
             UPDATE folders SET changed_at = MAX({NOW_MS}, OLD.changed_at + 1) WHERE id = NEW.id;
         END;
         CREATE TRIGGER IF NOT EXISTS folders_sync_ad AFTER DELETE ON folders
         WHEN OLD.uid IS NOT NULL BEGIN
             INSERT OR REPLACE INTO sync_tombstones (uid, kind, at) VALUES (OLD.uid, 'folder', {NOW_MS});
         END;
         DELETE FROM sync_applying;"
    ))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncReminder {
    pub due_at: i64,
    pub repeat: String,
    pub snoozed_until: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncNote {
    pub uid: String,
    pub title: String,
    pub body: String,
    pub color: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub pinned: bool,
    /// `uid` della cartella.
    pub folder: Option<String>,
    pub locked: bool,
    pub changed_at: i64,
    pub reminder: Option<SyncReminder>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SyncFolder {
    pub uid: String,
    pub name: String,
    pub changed_at: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tombstone {
    pub uid: String,
    pub kind: String,
    pub at: i64,
}

/// Tutto ciò che un PC condivide con gli altri.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub notes: Vec<SyncNote>,
    pub folders: Vec<SyncFolder>,
    pub tombstones: Vec<Tombstone>,
}

/// Come è andata l'applicazione delle modifiche di un altro PC.
#[derive(Debug, Default, PartialEq)]
pub struct Applied {
    /// Note o cartelle cambiate qui.
    pub changed: usize,
    /// Note modificate su entrambi i PC: la versione di qui è finita nella cronologia.
    pub conflicts: usize,
}

impl Db {
    /// Firma che cambia a ogni modifica da condividere (per sapere quando riscrivere il file).
    pub fn sync_signature(&self) -> rusqlite::Result<String> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT (SELECT COALESCE(MAX(changed_at), 0) || ':' || COUNT(*) FROM notes) || '/' ||
                    (SELECT COALESCE(MAX(changed_at), 0) || ':' || COUNT(*) FROM folders) || '/' ||
                    (SELECT COALESCE(MAX(at), 0) || ':' || COUNT(*) FROM sync_tombstones)",
            [],
            |r| r.get(0),
        )
    }

    pub fn sync_export(&self) -> rusqlite::Result<Snapshot> {
        let conn = self.0.lock().unwrap();
        let folders: Vec<SyncFolder> = conn
            .prepare("SELECT uid, name, changed_at FROM folders ORDER BY uid")?
            .query_map([], |r| Ok(SyncFolder { uid: r.get(0)?, name: r.get(1)?, changed_at: r.get(2)? }))?
            .collect::<rusqlite::Result<_>>()?;
        let notes: Vec<SyncNote> = conn
            .prepare(
                "SELECT n.uid, n.title, n.body, n.color, n.archived, n.created_at, n.updated_at, n.deleted_at,
                        n.pinned, f.uid, n.locked, n.changed_at, r.due_at, r.repeat, r.snoozed_until
                 FROM notes n
                 LEFT JOIN folders f ON f.id = n.folder_id
                 LEFT JOIN reminders r ON r.note_id = n.id
                 ORDER BY n.uid",
            )?
            .query_map([], |r| {
                let due: Option<i64> = r.get(12)?;
                Ok(SyncNote {
                    uid: r.get(0)?,
                    title: r.get(1)?,
                    body: r.get(2)?,
                    color: r.get(3)?,
                    archived: r.get::<_, i64>(4)? != 0,
                    created_at: r.get(5)?,
                    updated_at: r.get(6)?,
                    deleted_at: r.get(7)?,
                    pinned: r.get::<_, i64>(8)? != 0,
                    folder: r.get(9)?,
                    locked: r.get::<_, i64>(10)? != 0,
                    changed_at: r.get(11)?,
                    reminder: match due {
                        Some(due_at) => Some(SyncReminder { due_at, repeat: r.get(13)?, snoozed_until: r.get(14)? }),
                        None => None,
                    },
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let tombstones: Vec<Tombstone> = conn
            .prepare("SELECT uid, kind, at FROM sync_tombstones ORDER BY uid")?
            .query_map([], |r| Ok(Tombstone { uid: r.get(0)?, kind: r.get(1)?, at: r.get(2)? }))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Snapshot { notes, folders, tombstones })
    }

    /// Applica le note di un altro PC. `shared_until`: fin quando le modifiche di questo PC
    /// sono già state condivise (quelle successive, se perdono, vanno nella cronologia).
    pub fn sync_apply(&self, remote: &Snapshot, shared_until: i64) -> rusqlite::Result<Applied> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO sync_applying (x) VALUES (1)", [])?;
        let mut applied = Applied::default();
        let tomb_at = |tx: &Connection, uid: &str| -> rusqlite::Result<Option<i64>> {
            tx.query_row("SELECT at FROM sync_tombstones WHERE uid = ?1", [uid], |r| r.get(0)).optional()
        };

        // 1. Cartelle.
        for f in &remote.folders {
            let local: Option<(i64, i64)> = tx
                .query_row("SELECT id, changed_at FROM folders WHERE uid = ?1", [&f.uid], |r| Ok((r.get(0)?, r.get(1)?)))
                .optional()?;
            match local {
                Some((id, changed_at)) => {
                    if f.changed_at > changed_at {
                        let taken: bool = tx.query_row(
                            "SELECT EXISTS (SELECT 1 FROM folders WHERE name = ?1 AND id <> ?2)",
                            params![f.name, id],
                            |r| r.get(0),
                        )?;
                        if !taken {
                            tx.execute(
                                "UPDATE folders SET name = ?2, changed_at = ?3 WHERE id = ?1",
                                params![id, f.name, f.changed_at],
                            )?;
                            applied.changed += 1;
                        }
                    }
                }
                None => {
                    if tomb_at(&tx, &f.uid)?.is_some_and(|at| at >= f.changed_at) {
                        continue;
                    }
                    // Stessa cartella creata a mano su due PC: vince l'uid più piccolo, ovunque.
                    let same_name: Option<(i64, String)> = tx
                        .query_row("SELECT id, uid FROM folders WHERE name = ?1", [&f.name], |r| Ok((r.get(0)?, r.get(1)?)))
                        .optional()?;
                    match same_name {
                        Some((id, uid)) => {
                            if f.uid < uid {
                                tx.execute("UPDATE folders SET uid = ?2 WHERE id = ?1", params![id, f.uid])?;
                                applied.changed += 1;
                            }
                        }
                        None => {
                            tx.execute("DELETE FROM sync_tombstones WHERE uid = ?1", [&f.uid])?;
                            tx.execute(
                                "INSERT INTO folders (name, uid, changed_at) VALUES (?1, ?2, ?3)",
                                params![f.name, f.uid, f.changed_at],
                            )?;
                            applied.changed += 1;
                        }
                    }
                }
            }
        }
        // Cartella di una nota remota → cartella di qui (per uid, altrimenti per nome).
        let folder_id = |tx: &Connection, uid: &Option<String>| -> rusqlite::Result<Option<i64>> {
            let Some(uid) = uid else { return Ok(None) };
            if let Some(id) = tx.query_row("SELECT id FROM folders WHERE uid = ?1", [uid], |r| r.get(0)).optional()? {
                return Ok(Some(id));
            }
            match remote.folders.iter().find(|f| &f.uid == uid) {
                Some(f) => tx.query_row("SELECT id FROM folders WHERE name = ?1", [&f.name], |r| r.get(0)).optional(),
                None => Ok(None),
            }
        };

        // 2. Note.
        for n in &remote.notes {
            let local: Option<(i64, i64, String, String)> = tx
                .query_row(
                    "SELECT id, changed_at, title, body FROM notes WHERE uid = ?1",
                    [&n.uid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            let folder = folder_id(&tx, &n.folder)?;
            let id = match local {
                Some((id, changed_at, title, body)) => {
                    if n.changed_at <= changed_at {
                        continue;
                    }
                    if title != n.title || body != n.body {
                        if changed_at > shared_until && !(title.is_empty() && body.is_empty()) {
                            // Modificata anche qui e non ancora condivisa: la versione di qui va nella cronologia.
                            tx.execute(
                                "INSERT INTO note_versions (note_id, title, body, saved_at) VALUES (?1, ?2, ?3, ?4)",
                                params![id, title, body, crate::db::now()],
                            )?;
                            applied.conflicts += 1;
                        } else {
                            crate::organize::snapshot_before_update(&tx, id, &n.title, &n.body)?;
                        }
                    }
                    tx.execute(
                        "UPDATE notes SET title = ?2, body = ?3, color = ?4, archived = ?5, created_at = ?6,
                             updated_at = ?7, deleted_at = ?8, pinned = ?9, folder_id = ?10, locked = ?11,
                             changed_at = ?12
                         WHERE id = ?1",
                        params![
                            id, n.title, n.body, n.color, n.archived as i64, n.created_at, n.updated_at,
                            n.deleted_at, n.pinned as i64, folder, n.locked as i64, n.changed_at
                        ],
                    )?;
                    id
                }
                None => {
                    if tomb_at(&tx, &n.uid)?.is_some_and(|at| at >= n.changed_at) {
                        continue;
                    }
                    tx.execute("DELETE FROM sync_tombstones WHERE uid = ?1", [&n.uid])?;
                    tx.execute(
                        "INSERT INTO notes (uid, title, body, color, archived, created_at, updated_at, deleted_at,
                                            pinned, folder_id, locked, changed_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                        params![
                            n.uid, n.title, n.body, n.color, n.archived as i64, n.created_at, n.updated_at,
                            n.deleted_at, n.pinned as i64, folder, n.locked as i64, n.changed_at
                        ],
                    )?;
                    tx.last_insert_rowid()
                }
            };
            match &n.reminder {
                Some(r) => {
                    // L'eventuale notifica già mostrata qui resta valida se la scadenza non cambia.
                    tx.execute(
                        "INSERT INTO reminders (note_id, due_at, repeat, snoozed_until) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT (note_id) DO UPDATE SET
                             fired_at = CASE WHEN due_at IS excluded.due_at AND snoozed_until IS excluded.snoozed_until
                                             THEN fired_at ELSE NULL END,
                             due_at = excluded.due_at, repeat = excluded.repeat, snoozed_until = excluded.snoozed_until",
                        params![id, r.due_at, r.repeat, r.snoozed_until],
                    )?;
                }
                None => {
                    tx.execute("DELETE FROM reminders WHERE note_id = ?1", [id])?;
                }
            }
            applied.changed += 1;
        }

        // 3. Eliminazioni definitive.
        for t in &remote.tombstones {
            let table = if t.kind == "folder" { "folders" } else { "notes" };
            let local: Option<(i64, i64)> = tx
                .query_row(&format!("SELECT id, changed_at FROM {table} WHERE uid = ?1"), [&t.uid], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .optional()?;
            if let Some((id, changed_at)) = local {
                if changed_at > t.at {
                    continue; // modificata qui dopo l'eliminazione: resta
                }
                if t.kind == "folder" {
                    tx.execute("UPDATE notes SET folder_id = NULL WHERE folder_id = ?1", [id])?;
                }
                tx.execute(&format!("DELETE FROM {table} WHERE id = ?1"), [id])?;
                // Il trigger ha scritto la lapide con l'ora di adesso: va rimessa quella originale.
                tx.execute("UPDATE sync_tombstones SET at = ?2 WHERE uid = ?1", params![t.uid, t.at])?;
                applied.changed += 1;
                continue;
            }
            // La lapide tiene il momento originale, non quello in cui arriva qui.
            tx.execute(
                "INSERT INTO sync_tombstones (uid, kind, at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (uid) DO UPDATE SET at = MAX(at, excluded.at)",
                params![t.uid, t.kind, t.at],
            )?;
        }

        tx.execute("DELETE FROM sync_applying", [])?;
        tx.commit()?;
        Ok(applied)
    }

    /// Dopo il ripristino di un backup le note ripristinate devono vincere su quelle degli altri PC.
    pub fn sync_touch_all(&self) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE notes SET changed_at = MAX(changed_at + 1, ?1)",
            [now_ms()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Filter;
    use std::path::Path;

    fn db() -> Db {
        Db::open(Path::new(":memory:")).unwrap()
    }

    /// Porta le modifiche di `from` su `to`, come farebbe la cartella condivisa.
    fn send(from: &Db, to: &Db) -> Applied {
        to.sync_apply(&from.sync_export().unwrap(), i64::MAX).unwrap()
    }

    fn titles(db: &Db) -> Vec<String> {
        let mut t: Vec<String> = db.search("", Filter::All, None, None).unwrap().into_iter().map(|n| n.title).collect();
        t.sort();
        t
    }

    fn by_title(db: &Db, title: &str) -> crate::db::Note {
        db.search("", Filter::All, None, None).unwrap().into_iter().find(|n| n.title == title).unwrap()
    }

    fn sleep() {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    #[test]
    fn welcome_notes_are_not_duplicated() {
        let (a, b) = (db(), db());
        send(&a, &b);
        send(&b, &a);
        assert_eq!(titles(&a), vec!["Benvenuto", "Spesa"]);
        assert_eq!(titles(&b), vec!["Benvenuto", "Spesa"]);
    }

    #[test]
    fn notes_of_two_pcs_are_merged() {
        let (a, b) = (db(), db());
        a.create_full("Da A", "uno", "#fff").unwrap();
        b.create_full("Da B", "due", "#fff").unwrap();
        send(&a, &b);
        send(&b, &a);
        assert_eq!(titles(&a), titles(&b));
        assert!(titles(&a).contains(&"Da A".to_string()) && titles(&a).contains(&"Da B".to_string()));
    }

    #[test]
    fn the_newest_change_wins_everywhere() {
        let (a, b) = (db(), db());
        let n = a.create_full("Lista", "latte", "#fff").unwrap();
        send(&a, &b);
        sleep();
        let nb = by_title(&b, "Lista");
        b.update(nb.id, "Lista", "latte, uova").unwrap();
        b.set_color(nb.id, "#b3e5cf").unwrap();
        b.set_pinned(nb.id, true).unwrap();
        send(&b, &a);
        let na = a.get(n.id).unwrap();
        assert_eq!(na.body, "latte, uova");
        assert_eq!(na.color, "#b3e5cf");
        assert!(na.pinned);
        // Rimandare indietro non cambia niente: niente rimbalzi infiniti.
        assert_eq!(send(&a, &b), Applied::default());
    }

    #[test]
    fn a_conflict_keeps_the_losing_text_in_history() {
        let (a, b) = (db(), db());
        let n = a.create_full("Idee", "base", "#fff").unwrap();
        send(&a, &b);
        let shared_until = now_ms();
        sleep();
        a.update(n.id, "Idee", "scritto su A").unwrap();
        sleep();
        let nb = by_title(&b, "Idee");
        b.update(nb.id, "Idee", "scritto su B").unwrap();
        let applied = a.sync_apply(&b.sync_export().unwrap(), shared_until).unwrap();
        assert_eq!(applied.conflicts, 1);
        assert_eq!(a.get(n.id).unwrap().body, "scritto su B");
        let versions = a.list_versions(n.id).unwrap();
        assert!(versions.iter().any(|v| v.body == "scritto su A"));
    }

    #[test]
    fn deleting_forever_reaches_the_other_pc() {
        let (a, b) = (db(), db());
        let n = a.create_full("Vecchia", "", "#fff").unwrap();
        send(&a, &b);
        sleep();
        a.delete(&[n.id]).unwrap();
        send(&a, &b);
        assert!(!titles(&b).contains(&"Vecchia".to_string()));
        // E non torna indietro dal PC che l'aveva ancora.
        send(&b, &a);
        assert!(!titles(&a).contains(&"Vecchia".to_string()));
    }

    #[test]
    fn trash_archive_and_lock_travel() {
        let (a, b) = (db(), db());
        let n = a.create_full("Segreta", "x", "#fff").unwrap();
        let m = a.create_full("Finita", "", "#fff").unwrap();
        send(&a, &b);
        sleep();
        a.set_locked(n.id, true).unwrap();
        a.set_archived(&[m.id], true).unwrap();
        send(&a, &b);
        assert!(by_title(&b, "Segreta").locked);
        assert!(by_title(&b, "Finita").archived);
        sleep();
        a.set_trashed(&[m.id], true).unwrap();
        send(&a, &b);
        let trashed = b.search("", Filter::Trash, None, None).unwrap();
        assert!(trashed.iter().any(|n| n.title == "Finita" && n.deleted_at.is_some()));
    }

    #[test]
    fn reminders_travel_and_done_closes_them() {
        let (a, b) = (db(), db());
        let n = a.create_full("Dentista", "", "#fff").unwrap();
        a.set_reminder(n.id, 2_000_000_000, "none").unwrap();
        send(&a, &b);
        let nb = by_title(&b, "Dentista");
        assert_eq!(b.reminder(nb.id).unwrap(), Some((2_000_000_000, "none".to_string())));
        sleep();
        b.clear_reminder(nb.id).unwrap();
        send(&b, &a);
        assert_eq!(a.reminder(n.id).unwrap(), None);
    }

    #[test]
    fn folders_travel_and_same_names_are_merged() {
        let (a, b) = (db(), db());
        let fa = a.create_folder("Lavoro").unwrap();
        b.create_folder("Lavoro").unwrap();
        let n = a.create_full("Riunione", "", "#fff").unwrap();
        a.set_folder(&[n.id], Some(fa)).unwrap();
        send(&a, &b);
        send(&b, &a);
        send(&a, &b);
        assert_eq!(b.list_folders().unwrap().len(), 1);
        let folder_b = b.list_folders().unwrap()[0].id;
        assert_eq!(by_title(&b, "Riunione").folder_id, Some(folder_b));
        let uid = |db: &Db| -> String {
            db.0.lock().unwrap().query_row("SELECT uid FROM folders", [], |r| r.get(0)).unwrap()
        };
        assert_eq!(uid(&a), uid(&b));
        sleep();
        a.rename_folder(fa, "Ufficio").unwrap();
        send(&a, &b);
        assert_eq!(b.list_folders().unwrap()[0].name, "Ufficio");
        sleep();
        a.delete_folder(fa).unwrap();
        send(&a, &b);
        assert!(b.list_folders().unwrap().is_empty());
        assert_eq!(by_title(&b, "Riunione").folder_id, None);
    }

    #[test]
    fn a_note_edited_after_being_deleted_elsewhere_comes_back() {
        let (a, b) = (db(), db());
        let n = a.create_full("Ostinata", "", "#fff").unwrap();
        send(&a, &b);
        sleep();
        a.delete(&[n.id]).unwrap();
        sleep();
        let nb = by_title(&b, "Ostinata");
        b.update(nb.id, "Ostinata", "ancora qui").unwrap();
        send(&a, &b);
        assert!(titles(&b).contains(&"Ostinata".to_string()));
        send(&b, &a);
        assert!(titles(&a).contains(&"Ostinata".to_string()));
    }

    #[test]
    fn restoring_a_backup_wins_over_the_other_pcs() {
        let (a, b) = (db(), db());
        let n = a.create_full("Nota", "vecchio", "#fff").unwrap();
        send(&a, &b);
        sleep();
        let nb = by_title(&b, "Nota");
        b.update(nb.id, "Nota", "nuovo").unwrap();
        sleep();
        a.sync_touch_all().unwrap();
        send(&a, &b);
        assert_eq!(b.get(nb.id).unwrap().body, "vecchio");
        assert_eq!(a.get(n.id).unwrap().body, "vecchio");
    }
}
