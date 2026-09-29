//! Organizzazione delle note: tag (#parola nel testo), cartelle, note fissate,
//! link tra note ([[titolo]]) e cronologia delle versioni.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::db::{now, row_to_note, Db, Note, COLUMNS, NOTES_JOIN};

/// Una nuova versione si salva solo se l'ultima ha almeno 10 minuti:
/// così una sessione di scrittura diventa una versione sola.
const VERSION_INTERVAL_SECS: i64 = 10 * 60;
const MAX_VERSIONS_PER_NOTE: i64 = 50;

#[derive(Serialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    /// Note non nel cestino dentro la cartella.
    pub count: i64,
}

/// Note non nel cestino in totale e fuori da ogni cartella.
#[derive(Serialize)]
pub struct FolderCounts {
    pub all: i64,
    pub unfiled: i64,
}

#[derive(Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
}

#[derive(Serialize)]
pub struct NoteRef {
    pub id: i64,
    pub title: String,
}

#[derive(Serialize)]
pub struct Version {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub saved_at: i64,
}

pub fn create_tables(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS folders (
             id   INTEGER PRIMARY KEY AUTOINCREMENT,
             name TEXT NOT NULL UNIQUE COLLATE NOCASE
         );
         CREATE TABLE IF NOT EXISTS note_versions (
             id       INTEGER PRIMARY KEY AUTOINCREMENT,
             note_id  INTEGER NOT NULL,
             title    TEXT    NOT NULL,
             body     TEXT    NOT NULL,
             saved_at INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS note_versions_by_note ON note_versions (note_id, saved_at);
         CREATE TRIGGER IF NOT EXISTS notes_ad_versions AFTER DELETE ON notes BEGIN
             DELETE FROM note_versions WHERE note_id = old.id;
         END;",
    )
}

/// Tag di una nota: parole precedute da "#" a inizio riga o dopo uno spazio
/// (così "# Titolo" e gli indirizzi con "#" non contano), in minuscolo e senza doppioni.
pub fn extract_tags(body: &str) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let is_tag_char = |c: char| c.is_alphanumeric() || c == '_' || c == '-';
    let mut tags = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '#' && (i == 0 || chars[i - 1].is_whitespace()) {
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && is_tag_char(chars[end]) {
                end += 1;
            }
            let word: String = chars[start..end].iter().collect::<String>().to_lowercase();
            if word.chars().any(char::is_alphabetic) && !tags.contains(&word) {
                tags.push(word);
            }
            i = end.max(i + 1);
        } else {
            i += 1;
        }
    }
    tags
}

/// Prima di sovrascrivere una nota ne conserva il contenuto attuale come versione,
/// se è cambiato e se l'ultima versione è abbastanza vecchia.
pub fn snapshot_before_update(conn: &Connection, id: i64, title: &str, body: &str) -> rusqlite::Result<()> {
    let Some((old_title, old_body)) = conn
        .query_row("SELECT title, body FROM notes WHERE id = ?1", [id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .optional()?
    else {
        return Ok(());
    };
    if (old_title == title && old_body == body) || (old_title.is_empty() && old_body.is_empty()) {
        return Ok(());
    }
    let last: Option<i64> = conn.query_row(
        "SELECT MAX(saved_at) FROM note_versions WHERE note_id = ?1",
        [id],
        |r| r.get(0),
    )?;
    let t = now();
    if last.is_some_and(|last| t - last < VERSION_INTERVAL_SECS) {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO note_versions (note_id, title, body, saved_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, old_title, old_body, t],
    )?;
    conn.execute(
        "DELETE FROM note_versions WHERE note_id = ?1 AND id NOT IN (
             SELECT id FROM note_versions WHERE note_id = ?1 ORDER BY saved_at DESC, id DESC LIMIT ?2
         )",
        params![id, MAX_VERSIONS_PER_NOTE],
    )?;
    Ok(())
}

/// Sostituisce `from` con `to` ignorando maiuscole/minuscole (solo lettere ASCII, come `lower()` di SQLite).
fn replace_ignore_ascii_case(text: &str, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text.as_bytes()[i..];
        if rest.len() >= from.len() && rest[..from.len()].eq_ignore_ascii_case(from.as_bytes()) {
            out.push_str(to);
            i += from.len();
        } else {
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Quando una nota cambia titolo, i link `[[vecchio]]` nelle altre note diventano `[[nuovo]]`.
pub fn rename_links(conn: &Connection, id: i64, old: &str, new: &str) -> rusqlite::Result<usize> {
    if old.is_empty() || new.is_empty() || old == new || new.contains(['[', ']']) {
        return Ok(0);
    }
    let from = format!("[[{old}]]");
    let to = format!("[[{new}]]");
    let linking: Vec<(i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, body FROM notes WHERE id <> ?1 AND instr(lower(body), lower(?2)) > 0",
        )?;
        let rows = stmt.query_map(params![id, from], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for (other, body) in &linking {
        conn.execute(
            "UPDATE notes SET body = ?2 WHERE id = ?1",
            params![other, replace_ignore_ascii_case(body, &from, &to)],
        )?;
    }
    Ok(linking.len())
}

impl Db {
    pub fn set_pinned(&self, id: i64, pinned: bool) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE notes SET pinned = ?2 WHERE id = ?1", params![id, pinned as i64])?;
        Ok(())
    }

    pub fn list_folders(&self) -> rusqlite::Result<Vec<Folder>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT f.id, f.name, COUNT(n.id) FROM folders f
             LEFT JOIN notes n ON n.folder_id = f.id AND n.deleted_at IS NULL
             GROUP BY f.id ORDER BY f.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Folder {
                id: r.get(0)?,
                name: r.get(1)?,
                count: r.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn folder_counts(&self) -> rusqlite::Result<FolderCounts> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(folder_id IS NULL), 0) FROM notes WHERE deleted_at IS NULL",
            [],
            |r| {
                Ok(FolderCounts {
                    all: r.get(0)?,
                    unfiled: r.get(1)?,
                })
            },
        )
    }

    pub fn create_folder(&self, name: &str) -> rusqlite::Result<i64> {
        let conn = self.0.lock().unwrap();
        conn.execute("INSERT INTO folders (name) VALUES (?1)", [name])?;
        Ok(conn.last_insert_rowid())
    }

    pub fn rename_folder(&self, id: i64, name: &str) -> rusqlite::Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute("UPDATE folders SET name = ?2 WHERE id = ?1", params![id, name])?;
        Ok(())
    }

    /// Le note della cartella eliminata restano, senza cartella.
    pub fn delete_folder(&self, id: i64) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("UPDATE notes SET folder_id = NULL WHERE folder_id = ?1", [id])?;
        tx.execute("DELETE FROM folders WHERE id = ?1", [id])?;
        tx.commit()
    }

    pub fn set_folder(&self, ids: &[i64], folder_id: Option<i64>) -> rusqlite::Result<()> {
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        for id in ids {
            tx.execute("UPDATE notes SET folder_id = ?2 WHERE id = ?1", params![id, folder_id])?;
        }
        tx.commit()
    }

    /// Tutti i tag usati nelle note non nel cestino, dal più usato.
    pub fn list_tags(&self) -> rusqlite::Result<Vec<TagCount>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare("SELECT body FROM notes WHERE deleted_at IS NULL")?;
        let bodies = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut counts: Vec<TagCount> = Vec::new();
        for body in bodies {
            for tag in extract_tags(&body?) {
                match counts.iter_mut().find(|c| c.tag == tag) {
                    Some(c) => c.count += 1,
                    None => counts.push(TagCount { tag, count: 1 }),
                }
            }
        }
        counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.tag.cmp(&b.tag)));
        Ok(counts)
    }

    /// Titoli per l'autocompletamento dei link [[...]].
    pub fn list_titles(&self) -> rusqlite::Result<Vec<NoteRef>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title FROM notes WHERE deleted_at IS NULL AND title <> ''
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| Ok(NoteRef { id: r.get(0)?, title: r.get(1)? }))?;
        rows.collect()
    }

    /// La nota a cui punta un link [[titolo]] (senza distinguere maiuscole; la più recente se più d'una).
    pub fn find_by_title(&self, title: &str) -> rusqlite::Result<Option<Note>> {
        let conn = self.0.lock().unwrap();
        conn.query_row(
            &format!(
                "SELECT {COLUMNS} FROM {NOTES_JOIN}
                 WHERE deleted_at IS NULL AND lower(trim(title)) = lower(trim(?1))
                 ORDER BY updated_at DESC LIMIT 1"
            ),
            [title],
            row_to_note,
        )
        .optional()
    }

    /// Note che contengono un link [[titolo]] verso questa.
    pub fn backlinks(&self, id: i64) -> rusqlite::Result<Vec<NoteRef>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT n.id, n.title FROM notes n, notes target
             WHERE target.id = ?1 AND target.title <> '' AND n.id <> target.id
               AND n.deleted_at IS NULL
               AND instr(lower(n.body), lower('[[' || trim(target.title) || ']]')) > 0
             ORDER BY n.updated_at DESC",
        )?;
        let rows = stmt.query_map([id], |r| Ok(NoteRef { id: r.get(0)?, title: r.get(1)? }))?;
        rows.collect()
    }

    pub fn list_versions(&self, note_id: i64) -> rusqlite::Result<Vec<Version>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, body, saved_at FROM note_versions
             WHERE note_id = ?1 ORDER BY saved_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([note_id], |r| {
            Ok(Version {
                id: r.get(0)?,
                title: r.get(1)?,
                body: r.get(2)?,
                saved_at: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Riporta la nota a una versione; il contenuto attuale diventa a sua volta una versione.
    pub fn restore_version(&self, version_id: i64) -> rusqlite::Result<i64> {
        let conn = self.0.lock().unwrap();
        let (note_id, title, body): (i64, String, String) = conn.query_row(
            "SELECT note_id, title, body FROM note_versions WHERE id = ?1",
            [version_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        // Forza il salvataggio della versione attuale anche se l'ultima è recente.
        let (old_title, old_body): (String, String) =
            conn.query_row("SELECT title, body FROM notes WHERE id = ?1", [note_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        let t = now();
        conn.execute(
            "INSERT INTO note_versions (note_id, title, body, saved_at) VALUES (?1, ?2, ?3, ?4)",
            params![note_id, old_title, old_body, t],
        )?;
        conn.execute(
            "UPDATE notes SET title = ?2, body = ?3, updated_at = ?4 WHERE id = ?1",
            params![note_id, title, body, t],
        )?;
        Ok(note_id)
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_tags, replace_ignore_ascii_case};

    #[test]
    fn replaces_links_ignoring_case() {
        assert_eq!(
            replace_ignore_ascii_case("vedi [[spesa]] e [[Spesa]] città", "[[Spesa]]", "[[Lista]]"),
            "vedi [[Lista]] e [[Lista]] città"
        );
    }

    #[test]
    fn tags_from_text() {
        let body = "# Titolo\nComprare #pane e #Latte, poi #pane ancora.\nvedi https://x.it/#frammento #2026 #anno-2026";
        assert_eq!(extract_tags(body), vec!["pane", "latte", "anno-2026"]);
    }

    #[test]
    fn accented_tags() {
        assert_eq!(extract_tags("#città #caffè"), vec!["città", "caffè"]);
    }
}
