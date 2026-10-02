use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize)]
pub struct Note {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub color: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    /// Momento in cui la nota è finita nel cestino; `None` se non è nel cestino.
    pub deleted_at: Option<i64>,
    /// Prossima scadenza del promemoria (già tenendo conto del posticipo), se c'è.
    pub remind_at: Option<i64>,
    /// Ricorrenza del promemoria: "none", "daily", "weekly" o "monthly".
    pub repeat: Option<String>,
    /// Fissata in alto nel deck e nell'elenco.
    pub pinned: bool,
    pub folder_id: Option<i64>,
    /// ID univoco dell'evento Google Calendar; `None` per le note normali.
    pub gcal_event_id: Option<String>,
}

/// Promemoria scaduto da notificare.
pub struct DueReminder {
    pub note_id: i64,
    pub title: String,
    pub body: String,
    pub due_at: i64,
}

pub struct Db(pub Mutex<Connection>);

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn row_to_note(row: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        title: row.get(1)?,
        body: row.get(2)?,
        color: row.get(3)?,
        archived: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        deleted_at: row.get(7)?,
        remind_at: row.get(8)?,
        repeat: row.get(9)?,
        pinned: row.get::<_, i64>(10)? != 0,
        folder_id: row.get(11)?,
        gcal_event_id: row.get(12)?,
    })
}

pub(crate) const COLUMNS: &str = "id, title, body, color, archived, created_at, updated_at, deleted_at, \
                       COALESCE(snoozed_until, due_at), repeat, pinned, folder_id, gcal_event_id";
/// Ogni nota con il suo eventuale promemoria (al massimo uno per nota).
pub(crate) const NOTES_JOIN: &str = "notes LEFT JOIN reminders ON reminders.note_id = notes.id";
/// Scadenza effettiva: il posticipo, se c'è, prende il posto della data originale.
const EFFECTIVE_DUE: &str = "COALESCE(r.snoozed_until, r.due_at)";
/// Promemoria ancora da notificare per la scadenza attuale, su note attive.
const PENDING: &str = "n.deleted_at IS NULL AND n.archived = 0 \
                       AND (r.fired_at IS NULL OR r.fired_at < COALESCE(r.snoozed_until, r.due_at))";

/// Quanti giorni prima dell'inizio un evento di Google Calendar compare nel deck.
const DECK_GCAL_DAYS: i64 = 7;

/// Le note restano nel cestino 30 giorni, poi vengono eliminate all'avvio.
const TRASH_RETENTION_SECS: i64 = 30 * 24 * 60 * 60;

/// Quali note mostrare nella finestra "Tutte le note".
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Filter {
    All,
    Active,
    Archived,
    Trash,
}

impl Filter {
    fn sql(self) -> &'static str {
        match self {
            Filter::All => "deleted_at IS NULL",
            Filter::Active => "deleted_at IS NULL AND archived = 0",
            Filter::Archived => "deleted_at IS NULL AND archived = 1",
            Filter::Trash => "deleted_at IS NOT NULL",
        }
    }
}

fn add_column_if_missing(conn: &Connection, column: &str, definition: &str) -> rusqlite::Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM pragma_table_info('notes') WHERE name = ?1)",
        [column],
        |r| r.get(0),
    )?;
    if !exists {
        conn.execute(&format!("ALTER TABLE notes ADD COLUMN {column} {definition}"), [])?;
    }
    Ok(())
}

/// Aggiunge le colonne e le tabelle introdotte dopo la prima versione del database.
fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    add_column_if_missing(conn, "deleted_at", "INTEGER")?;
    add_column_if_missing(conn, "pinned", "INTEGER NOT NULL DEFAULT 0")?;
    add_column_if_missing(conn, "folder_id", "INTEGER")?;
    add_column_if_missing(conn, "gcal_event_id", "TEXT")?;
    crate::organize::create_tables(conn)?;
    // Indice univoco per gcal_event_id: evita duplicati alla sync.
    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS notes_gcal_event_id
             ON notes(gcal_event_id) WHERE gcal_event_id IS NOT NULL;",
    )?;
    // `due_at` è la scadenza della serie; `snoozed_until` un posticipo che non sposta la serie;
    // `fired_at` quando è partita l'ultima notifica, per non ripeterla.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS reminders (
             note_id       INTEGER PRIMARY KEY,
             due_at        INTEGER NOT NULL,
             repeat        TEXT    NOT NULL DEFAULT 'none',
             snoozed_until INTEGER,
             fired_at      INTEGER
         );
         CREATE TRIGGER IF NOT EXISTS notes_ad_reminder AFTER DELETE ON notes BEGIN
             DELETE FROM reminders WHERE note_id = old.id;
         END;",
    )?;
    Ok(())
}

/// Indice full-text (FTS5) su titolo e corpo, tenuto allineato da trigger.
/// `remove_diacritics` fa trovare "caffe" anche cercando "caffè".
fn create_search_index(conn: &Connection) -> rusqlite::Result<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE name = 'notes_fts')",
        [],
        |r| r.get(0),
    )?;
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
             title, body,
             content = 'notes', content_rowid = 'id',
             tokenize = 'unicode61 remove_diacritics 2'
         );
         CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
             INSERT INTO notes_fts (rowid, title, body) VALUES (new.id, new.title, new.body);
         END;
         CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
             INSERT INTO notes_fts (notes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
         END;
         CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE OF title, body ON notes BEGIN
             INSERT INTO notes_fts (notes_fts, rowid, title, body) VALUES ('delete', old.id, old.title, old.body);
             INSERT INTO notes_fts (rowid, title, body) VALUES (new.id, new.title, new.body);
         END;",
    )?;
    if !exists {
        // Database creato prima dell'indice: indicizza le note già presenti.
        conn.execute("INSERT INTO notes_fts (notes_fts) VALUES ('rebuild')", [])?;
    }
    Ok(())
}

/// Trasforma il testo digitato in una query FTS5: ogni parola diventa un prefisso
/// tra virgolette (`"spe"*`), così la ricerca funziona mentre si scrive e i caratteri
/// speciali di FTS5 non causano errori di sintassi.
fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .map(|w| format!("\"{}\"*", w.replace('"', "\"\"")))
        .collect();
    (!terms.is_empty()).then(|| terms.join(" "))
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS notes (
                 id         INTEGER PRIMARY KEY AUTOINCREMENT,
                 title      TEXT    NOT NULL DEFAULT '',
                 body       TEXT    NOT NULL DEFAULT '',
                 color      TEXT    NOT NULL,
                 archived   INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );",
        )?;
        migrate(&conn)?;
        create_search_index(&conn)?;
        conn.execute(
            "DELETE FROM notes WHERE deleted_at < ?1",
            [now() - TRASH_RETENTION_SECS],
        )?;
        let db = Db(Mutex::new(conn));
        db.seed_if_empty()?;
        Ok(db)
    }

    /// Copia coerente del database in un file nuovo (per i backup), anche mentre l'app scrive.
    pub fn copy_to(&self, path: &Path) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
        Ok(())
    }

    /// Sostituisce tutto il contenuto con quello di un backup, senza riaprire la connessione.
    pub fn restore_from(&self, path: &Path) -> rusqlite::Result<()> {
        let source = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut conn = self.0.lock().unwrap();
        rusqlite::backup::Backup::new(&source, &mut conn)?.run_to_completion(
            256,
            std::time::Duration::ZERO,
            None,
        )?;
        // Un backup di una versione precedente potrebbe non avere le colonne più recenti.
        migrate(&conn)?;
        create_search_index(&conn)
    }

    /// Alla prima esecuzione mette qualche nota di benvenuto, così il deck non è vuoto.
    fn seed_if_empty(&self) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))?;
        if count > 0 {
            return Ok(());
        }
        let t = now();
        let seed = [
            ("Benvenuto", "- avvicina il mouse al bordo destro\n- clicca una linguetta per aprirla\n- scrivi: si salva da sola", "#b5d3f7"),
            ("Spesa", "- mele\n- banane\n- caffè", "#b3e5cf"),
        ];
        for (title, body, color) in seed {
            conn.execute(
                "INSERT INTO notes (title, body, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![title, body, color, t],
            )?;
        }
        Ok(())
    }

    /// Note del deck. Gli eventi di Google Calendar compaiono solo nei `DECK_GCAL_DAYS` giorni
    /// prima dell'inizio: con mesi di calendario il deck sarebbe pieno di linguette.
    /// Gli altri restano in "Tutte le note".
    pub fn list_active(&self) -> rusqlite::Result<Vec<Note>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM {NOTES_JOIN} WHERE archived = 0 AND deleted_at IS NULL
                 AND (gcal_event_id IS NULL OR reminders.due_at IS NULL OR reminders.due_at <= ?1)
             ORDER BY pinned DESC, id ASC"
        ))?;
        let rows = stmt.query_map([now() + DECK_GCAL_DAYS * 24 * 60 * 60], row_to_note)?;
        rows.collect()
    }

    pub fn create(&self, color: &str) -> rusqlite::Result<Note> {
        let conn = self.0.lock().unwrap();
        let t = now();
        conn.execute(
            "INSERT INTO notes (color, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![color, t],
        )?;
        let id = conn.last_insert_rowid();
        conn.query_row(
            &format!("SELECT {COLUMNS} FROM {NOTES_JOIN} WHERE id = ?1"),
            [id],
            row_to_note,
        )
    }

    /// Salva titolo e testo; se il titolo cambia, aggiorna i link [[vecchio titolo]] nelle altre note.
    pub fn update(&self, id: i64, title: &str, body: &str) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        let old_title: Option<String> = conn
            .query_row("SELECT title FROM notes WHERE id = ?1", [id], |r| r.get(0))
            .optional()?;
        crate::organize::snapshot_before_update(&conn, id, title, body)?;
        conn.execute(
            "UPDATE notes SET title = ?2, body = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, title, body, now()],
        )?;
        if let Some(old) = old_title {
            crate::organize::rename_links(&conn, id, old.trim(), title.trim())?;
        }
        Ok(())
    }

    pub fn get(&self, id: i64) -> rusqlite::Result<Note> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            &format!("SELECT {COLUMNS} FROM {NOTES_JOIN} WHERE id = ?1"),
            [id],
            row_to_note,
        )
    }

    /// Nota nuova già con titolo e testo (cattura rapida, import).
    pub fn create_full(&self, title: &str, body: &str, color: &str) -> rusqlite::Result<Note> {
        let id = {
            let conn = self.0.lock().unwrap();
            let t = now();
            conn.execute(
                "INSERT INTO notes (title, body, color, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
                params![title, body, color, t],
            )?;
            conn.last_insert_rowid()
        };
        self.get(id)
    }

    /// Tutti i testi salvati (note, cestino, cronologia): servono a capire quali file media sono in uso.
    pub fn all_texts(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT body FROM notes UNION ALL SELECT body FROM note_versions")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    }

    /// Note per la finestra "Tutte le note": fissate prima, poi dalla più recente.
    /// `query` vuota = nessun filtro testuale; `folder` 0 = senza cartella; `tag` senza "#".
    pub fn search(
        &self,
        query: &str,
        filter: Filter,
        folder: Option<i64>,
        tag: Option<&str>,
    ) -> rusqlite::Result<Vec<Note>> {
        let conn = self.0.lock().unwrap();
        let where_filter = match folder {
            None => filter.sql().to_string(),
            Some(0) => format!("{} AND folder_id IS NULL", filter.sql()),
            Some(id) => format!("{} AND folder_id = {id}", filter.sql()),
        };
        let notes: Vec<Note> = match fts_query(query) {
            Some(fts) => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {COLUMNS} FROM {NOTES_JOIN}
                     WHERE {where_filter}
                       AND id IN (SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?1)
                     ORDER BY pinned DESC, updated_at DESC, id DESC"
                ))?;
                let rows = stmt.query_map([fts], row_to_note)?;
                rows.collect::<rusqlite::Result<_>>()?
            }
            None => {
                let mut stmt = conn.prepare(&format!(
                    "SELECT {COLUMNS} FROM {NOTES_JOIN} WHERE {where_filter}
                     ORDER BY pinned DESC, updated_at DESC, id DESC"
                ))?;
                let rows = stmt.query_map([], row_to_note)?;
                rows.collect::<rusqlite::Result<_>>()?
            }
        };
        // I tag stanno nel testo (#parola), quindi il filtro si fa qui.
        Ok(match tag {
            Some(tag) => notes
                .into_iter()
                .filter(|n| crate::organize::extract_tags(&n.body).iter().any(|t| t == tag))
                .collect(),
            None => notes,
        })
    }

    pub fn set_archived(&self, ids: &[i64], archived: bool) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let t = now();
        for id in ids {
            tx.execute(
                "UPDATE notes SET archived = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, archived as i64, t],
            )?;
        }
        tx.commit()
    }

    pub fn set_color(&self, id: i64, color: &str) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE notes SET color = ?2 WHERE id = ?1", params![id, color])?;
        Ok(())
    }

    /// Sposta nel cestino (`trashed = true`) o ripristina dal cestino.
    pub fn set_trashed(&self, ids: &[i64], trashed: bool) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let deleted_at = trashed.then(now);
        for id in ids {
            tx.execute(
                "UPDATE notes SET deleted_at = ?2 WHERE id = ?1",
                params![id, deleted_at],
            )?;
        }
        tx.commit()
    }

    pub fn empty_trash(&self) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM notes WHERE deleted_at IS NOT NULL", [])?;
        Ok(())
    }

    /// Imposta (o sostituisce) il promemoria di una nota, azzerando posticipo e notifica.
    pub fn set_reminder(&self, note_id: i64, due_at: i64, repeat: &str) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO reminders (note_id, due_at, repeat) VALUES (?1, ?2, ?3)
             ON CONFLICT (note_id) DO UPDATE SET
                 due_at = excluded.due_at, repeat = excluded.repeat,
                 snoozed_until = NULL, fired_at = NULL",
            params![note_id, due_at, repeat],
        )?;
        Ok(())
    }

    pub fn clear_reminder(&self, note_id: i64) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("DELETE FROM reminders WHERE note_id = ?1", [note_id])?;
        Ok(())
    }

    pub fn snooze_reminder(&self, note_id: i64, until: i64) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE reminders SET snoozed_until = ?2, fired_at = NULL WHERE note_id = ?1",
            params![note_id, until],
        )?;
        Ok(())
    }

    /// Scadenza della serie e ricorrenza del promemoria di una nota.
    pub fn reminder(&self, note_id: i64) -> rusqlite::Result<Option<(i64, String)>> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT due_at, repeat FROM reminders WHERE note_id = ?1",
            [note_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
    }

    pub fn due_reminders(&self, now: i64) -> rusqlite::Result<Vec<DueReminder>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT n.id, n.title, n.body, {EFFECTIVE_DUE} FROM notes n
             JOIN reminders r ON r.note_id = n.id
             WHERE {PENDING} AND {EFFECTIVE_DUE} <= ?1
             ORDER BY {EFFECTIVE_DUE}"
        ))?;
        let rows = stmt.query_map([now], |r| {
            Ok(DueReminder {
                note_id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                due_at: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Promemoria già notificati per la scadenza attuale e non ancora chiusi né posticipati.
    pub fn unhandled_reminders(&self, since: i64) -> rusqlite::Result<Vec<DueReminder>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(&format!(
            "SELECT n.id, n.title, n.body, {EFFECTIVE_DUE} FROM notes n
             JOIN reminders r ON r.note_id = n.id
             WHERE n.deleted_at IS NULL AND n.archived = 0
               AND r.fired_at >= {EFFECTIVE_DUE} AND {EFFECTIVE_DUE} >= ?1
             ORDER BY {EFFECTIVE_DUE}"
        ))?;
        let rows = stmt.query_map([since], |r| {
            Ok(DueReminder {
                note_id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                due_at: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn mark_fired(&self, note_id: i64, now: i64) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute(
            "UPDATE reminders SET fired_at = ?2 WHERE note_id = ?1",
            params![note_id, now],
        )?;
        Ok(())
    }

    /// La prossima scadenza non ancora notificata, per sapere quanto dormire.
    pub fn next_pending_due(&self) -> rusqlite::Result<Option<i64>> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            &format!(
                "SELECT MIN({EFFECTIVE_DUE}) FROM notes n
                 JOIN reminders r ON r.note_id = n.id WHERE {PENDING}"
            ),
            [],
            |r| r.get(0),
        )
    }

    /// Eliminazione definitiva.
    pub fn delete(&self, ids: &[i64]) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        }
        tx.commit()
    }

    // -- Google Calendar -------------------------------------------------------

    pub fn upsert_gcal_note(
        &self, gcal_event_id: &str, title: &str, body: &str,
        color: &str, remind_at: i64, folder_id: i64,
    ) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        let t = now();
        conn.execute(
            "INSERT INTO notes (title, body, color, folder_id, gcal_event_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) \
             ON CONFLICT (gcal_event_id) WHERE gcal_event_id IS NOT NULL DO UPDATE SET \
             title=excluded.title, body=excluded.body, updated_at=?6",
            params![title, body, color, folder_id, gcal_event_id, t],
        )?;
        let note_id: i64 = conn.query_row(
            "SELECT id FROM notes WHERE gcal_event_id = ?1",
            [gcal_event_id], |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO reminders (note_id, due_at, repeat) VALUES (?1, ?2, 'none') \
             ON CONFLICT (note_id) DO UPDATE SET              snoozed_until = CASE WHEN due_at <> ?2 THEN NULL ELSE snoozed_until END,              fired_at = CASE WHEN due_at <> ?2 THEN NULL ELSE fired_at END,              due_at = ?2",
            params![note_id, remind_at],
        )?;
        Ok(())
    }

    pub fn trash_stale_gcal_notes(
        &self, current_ids: &[String], calendar_ids: &[String],
    ) -> rusqlite::Result<()> {
        if calendar_ids.is_empty() { return Ok(()); }
        let conn = self.0.lock().unwrap();
        let t = now();
        let mut stmt = conn.prepare(
            "SELECT gcal_event_id FROM notes WHERE gcal_event_id IS NOT NULL AND deleted_at IS NULL",
        )?;
        let existing: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .filter(|eid: &String| calendar_ids.iter().any(|cid| eid.starts_with(&format!("{cid}_"))))
            .collect();
        for eid in existing.iter().filter(|eid| !current_ids.contains(eid)) {
            conn.execute(
                "UPDATE notes SET deleted_at = ?1 WHERE gcal_event_id = ?2",
                params![t, eid],
            )?;
        }
        Ok(())
    }

    pub fn gcal_folder_id(&self) -> rusqlite::Result<i64> {
        let conn = self.0.lock().unwrap();
        match conn.query_row("SELECT id FROM folders WHERE name = 'Calendario Google'", [], |r| r.get(0)) {
            Ok(id) => Ok(id),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                conn.execute("INSERT INTO folders (name) VALUES ('Calendario Google')", [])?;
                Ok(conn.last_insert_rowid())
            }
            Err(e) => Err(e),
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Db {
        Db::open(Path::new(":memory:")).expect("database in memoria")
    }

    fn ids(notes: &[Note]) -> Vec<i64> {
        let mut ids: Vec<i64> = notes.iter().map(|n| n.id).collect();
        ids.sort();
        ids
    }

    #[test]
    fn seeds_welcome_notes_once() {
        let db = db();
        assert_eq!(db.list_active().unwrap().len(), 2);
        db.seed_if_empty().unwrap();
        assert_eq!(db.list_active().unwrap().len(), 2);
    }

    #[test]
    fn search_ignores_accents_and_matches_prefixes() {
        let db = db();
        let note = db.create_full("Ricetta", "tiramisù con caffè", "#fff").unwrap();
        let found = db.search("caffe tira", Filter::All, None, None).unwrap();
        assert_eq!(ids(&found), vec![note.id]);
        // Caratteri speciali di FTS5 non devono dare errori.
        assert!(db.search("\"(*", Filter::All, None, None).is_ok());
    }

    #[test]
    fn filters_archive_and_trash() {
        let db = db();
        let a = db.create_full("A", "", "#fff").unwrap();
        let b = db.create_full("B", "", "#fff").unwrap();
        db.set_archived(&[a.id], true).unwrap();
        db.set_trashed(&[b.id], true).unwrap();
        let archived = db.search("", Filter::Archived, None, None).unwrap();
        assert_eq!(ids(&archived), vec![a.id]);
        let trash = db.search("", Filter::Trash, None, None).unwrap();
        assert_eq!(ids(&trash), vec![b.id]);
        assert!(!ids(&db.search("", Filter::All, None, None).unwrap()).contains(&b.id));
        assert!(!ids(&db.list_active().unwrap()).contains(&a.id));
        db.set_trashed(&[b.id], false).unwrap();
        assert!(ids(&db.list_active().unwrap()).contains(&b.id));
    }

    #[test]
    fn tags_and_folders_filter_the_list() {
        let db = db();
        let work = db.create_full("Lavoro", "riunione #ufficio", "#fff").unwrap();
        let home = db.create_full("Casa", "#spesa #ufficio-casa", "#fff").unwrap();
        let tagged = db.search("", Filter::All, None, Some("ufficio")).unwrap();
        assert_eq!(ids(&tagged), vec![work.id]);

        let folder = db.create_folder("Progetti").unwrap();
        db.set_folder(&[home.id], Some(folder)).unwrap();
        let counts = db.folder_counts().unwrap();
        assert_eq!((counts.all, counts.unfiled), (4, 3)); // 2 note di benvenuto + 2 create
        assert_eq!(ids(&db.search("", Filter::All, Some(folder), None).unwrap()), vec![home.id]);
        assert!(!ids(&db.search("", Filter::All, Some(0), None).unwrap()).contains(&home.id));
        db.delete_folder(folder).unwrap();
        assert_eq!(db.get(home.id).unwrap().folder_id, None);
    }

    #[test]
    fn reminders_fire_once_and_snooze() {
        let db = db();
        let note = db.create_full("Chiamare", "", "#fff").unwrap();
        let t = now();
        db.set_reminder(note.id, t - 60, "none").unwrap();
        assert_eq!(db.due_reminders(t).unwrap().len(), 1);
        db.mark_fired(note.id, t).unwrap();
        assert!(db.due_reminders(t).unwrap().is_empty());
        assert_eq!(db.next_pending_due().unwrap(), None);

        db.snooze_reminder(note.id, t + 600).unwrap();
        assert_eq!(db.get(note.id).unwrap().remind_at, Some(t + 600));
        assert_eq!(db.next_pending_due().unwrap(), Some(t + 600));
        assert!(db.due_reminders(t).unwrap().is_empty());
        assert_eq!(db.due_reminders(t + 601).unwrap().len(), 1);

        // Le note archiviate non suonano.
        db.set_archived(&[note.id], true).unwrap();
        assert!(db.due_reminders(t + 601).unwrap().is_empty());
    }

    #[test]
    fn versions_are_kept_at_most_every_ten_minutes() {
        let db = db();
        let note = db.create_full("Diario", "prima", "#fff").unwrap();
        db.update(note.id, "Diario", "seconda").unwrap();
        db.update(note.id, "Diario", "terza").unwrap();
        let versions = db.list_versions(note.id).unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].body, "prima");

        db.restore_version(versions[0].id).unwrap();
        assert_eq!(db.get(note.id).unwrap().body, "prima");
        // Il testo sostituito dal ripristino diventa a sua volta una versione.
        assert_eq!(db.list_versions(note.id).unwrap()[0].body, "terza");
    }

    #[test]
    fn renaming_a_note_updates_links_and_backlinks() {
        let db = db();
        let target = db.create_full("Spesa", "", "#fff").unwrap();
        let source = db.create_full("Menu", "vedi [[spesa]] per sabato", "#fff").unwrap();
        assert_eq!(db.backlinks(target.id).unwrap().len(), 1);
        db.update(target.id, "Lista della spesa", "").unwrap();
        assert_eq!(db.get(source.id).unwrap().body, "vedi [[Lista della spesa]] per sabato");
        assert_eq!(db.backlinks(target.id).unwrap()[0].id, source.id);
        assert_eq!(db.find_by_title("lista della SPESA").unwrap().map(|n| n.id), Some(target.id));
    }

    #[test]
    fn pinned_notes_come_first() {
        let db = db();
        let pinned = db.create_full("Z", "", "#fff").unwrap();
        db.set_pinned(pinned.id, true).unwrap();
        assert_eq!(db.list_active().unwrap()[0].id, pinned.id);
        assert_eq!(db.search("", Filter::All, None, None).unwrap()[0].id, pinned.id);
    }

    #[test]
    fn backup_copy_restores_notes_and_search() {
        let dir = std::env::temp_dir().join(format!("tabby-db-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("notes.db");

        let db = db();
        let note = db.create_full("Bollette", "pagare la luce", "#b5d3f7").unwrap();
        db.copy_to(&copy).unwrap();
        db.delete(&[note.id]).unwrap();
        db.create_full("Dopo il backup", "", "#b5d3f7").unwrap();

        db.restore_from(&copy).unwrap();
        let titles: Vec<String> = db.list_active().unwrap().into_iter().map(|n| n.title).collect();
        assert!(titles.contains(&"Bollette".to_string()));
        assert!(!titles.contains(&"Dopo il backup".to_string()));
        assert_eq!(db.search("luce", Filter::All, None, None).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn deck_shows_only_upcoming_calendar_events() {
        let db = db();
        let folder = db.gcal_folder_id().unwrap();
        let t = now();
        db.upsert_gcal_note("cal_vicino", "Dentista", "", "#b5d3f7", t + 2 * 86_400, folder).unwrap();
        db.upsert_gcal_note("cal_lontano", "Ferie", "", "#b5d3f7", t + 40 * 86_400, folder).unwrap();
        let deck: Vec<String> = db.list_active().unwrap().into_iter().map(|n| n.title).collect();
        assert!(deck.contains(&"Dentista".to_string()));
        assert!(!deck.contains(&"Ferie".to_string()));
        // In "Tutte le note" c'è anche l'evento lontano.
        assert_eq!(db.search("ferie", Filter::Active, None, None).unwrap().len(), 1);
    }
}
