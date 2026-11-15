//! Copie leggibili delle note nella cartella condivisa (`Note da leggere/`), un file `.md`
//! per nota: dal telefono si aprono con l'app di Google Drive, OneDrive o Dropbox,
//! senza installare niente. Sono di sola lettura: le modifiche fatte lì non tornano in Tabby.
//!
//! Le note protette da PIN, archiviate o nel cestino non vengono copiate. Con la cartella
//! cifrata le copie non esistono (sarebbero in chiaro).

use crate::syncdb::Snapshot;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

pub const DIR_NAME: &str = "Note da leggere";
pub const DIR_NAME_EN: &str = "Notes to read";

/// Il nome viene da `tabby-sync.json`: deve essere un nome semplice, non un percorso.
pub fn is_valid_dir_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 60 && file_name(name) == name
}
const README: &str = "LEGGIMI.txt";
const README_TEXT: &str = "Copie di sola lettura delle note di Tabby, aggiornate da sole.\r\n\
Aprile dal telefono con l'app di Google Drive, OneDrive o Dropbox.\r\n\
Le modifiche fatte qui NON tornano in Tabby: verrebbero sovrascritte.\r\n\r\n\
Read-only copies of your Tabby notes, kept up to date automatically.\r\n\
Open them on your phone with the Google Drive, OneDrive or Dropbox app.\r\n\
Changes made here do NOT go back to Tabby: they would be overwritten.\r\n";

/// Nome di file valido su Windows (e sui telefoni) a partire dal titolo della nota.
pub fn file_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { ' ' } else { c })
        .collect();
    let mut name: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    name = name.trim_matches(['.', ' ']).chars().take(80).collect::<String>().trim_end().to_string();
    if name.is_empty() {
        name = crate::i18n::tr("Senza titolo", "Untitled").into();
    }
    let upper = name.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&upper.as_str())
        || ((upper.starts_with("COM") || upper.starts_with("LPT")) && upper.len() == 4 && upper.as_bytes()[3].is_ascii_digit());
    if reserved {
        name = format!("_{name}");
    }
    name
}

/// File da avere in `Note da leggere/`: percorso relativo → contenuto.
pub fn desired_files(snapshot: &Snapshot) -> BTreeMap<PathBuf, String> {
    let folders: HashMap<&str, &str> = snapshot.folders.iter().map(|f| (f.uid.as_str(), f.name.as_str())).collect();
    let mut notes: Vec<_> = snapshot
        .notes
        .iter()
        .filter(|n| n.deleted_at.is_none() && !n.archived && !n.locked)
        .collect();
    notes.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.uid.cmp(&b.uid)));
    let mut files = BTreeMap::new();
    let mut used: HashSet<String> = HashSet::new();
    for note in notes {
        let dir = note
            .folder
            .as_deref()
            .and_then(|uid| folders.get(uid))
            .map(|name| PathBuf::from(file_name(name)))
            .unwrap_or_default();
        let base = file_name(&note.title);
        let mut n = 1;
        let path = loop {
            let candidate = dir.join(if n == 1 { format!("{base}.md") } else { format!("{base} ({n}).md") });
            if used.insert(candidate.to_string_lossy().to_lowercase()) {
                break candidate;
            }
            n += 1;
        };
        let title = if note.title.trim().is_empty() { crate::i18n::tr("Senza titolo", "Untitled") } else { note.title.trim() };
        files.insert(path, format!("# {title}\n\n{}\n", note.body.trim_end()));
    }
    files
}

/// Allinea `Note da leggere/` ai file voluti: scrive quelli cambiati, toglie quelli in più.
pub fn write(dir: &Path, dir_name: &str, snapshot: &Snapshot) -> std::io::Result<()> {
    let root = dir.join(dir_name);
    fs::create_dir_all(&root)?;
    let readme = root.join(README);
    if !readme.is_file() {
        fs::write(&readme, README_TEXT)?;
    }
    let desired = desired_files(snapshot);
    for (rel, content) in &desired {
        let path = root.join(rel);
        if fs::read_to_string(&path).ok().as_deref() == Some(content.as_str()) {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
    }
    remove_extra(&root, &root, &desired)?;
    Ok(())
}

/// Toglie i `.md` che non corrispondono più a nessuna nota e le sottocartelle rimaste vuote.
fn remove_extra(root: &Path, dir: &Path, desired: &BTreeMap<PathBuf, String>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            remove_extra(root, &path, desired)?;
            if fs::read_dir(&path)?.next().is_none() {
                let _ = fs::remove_dir(&path);
            }
        } else if path.extension().is_some_and(|e| e == "md") {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if !desired.contains_key(&rel) {
                let _ = fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

/// Elimina tutte le copie (cartella cifrata o copie spente).
pub fn remove(dir: &Path, dir_name: &str) {
    let root = dir.join(dir_name);
    if root.is_dir() {
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syncdb::{SyncFolder, SyncNote};

    fn note(uid: &str, title: &str, folder: Option<&str>) -> SyncNote {
        SyncNote {
            uid: uid.into(),
            title: title.into(),
            body: "testo".into(),
            color: "#fff".into(),
            archived: false,
            created_at: 1,
            updated_at: 1,
            deleted_at: None,
            pinned: false,
            folder: folder.map(String::from),
            locked: false,
            changed_at: 1,
            reminder: None,
        }
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("Spesa: latte/uova?"), "Spesa latte uova");
        assert_eq!(file_name("   "), "Senza titolo");
        assert_eq!(file_name("con"), "_con");
        assert_eq!(file_name("Fine."), "Fine");
    }

    #[test]
    fn folders_duplicates_and_hidden_notes() {
        let mut secret = note("c", "Segreta", None);
        secret.locked = true;
        let mut old = note("d", "Vecchia", None);
        old.archived = true;
        let snapshot = Snapshot {
            notes: vec![note("a", "Spesa", Some("f")), note("b", "Spesa", Some("f")), secret, old],
            folders: vec![SyncFolder { uid: "f".into(), name: "Casa".into(), changed_at: 1 }],
            tombstones: vec![],
        };
        let files = desired_files(&snapshot);
        let names: Vec<String> = files.keys().map(|p| p.to_string_lossy().replace('\\', "/")).collect();
        assert_eq!(names, vec!["Casa/Spesa (2).md", "Casa/Spesa.md"]);
        assert_eq!(files.values().next().unwrap(), "# Spesa\n\ntesto\n");
    }

    #[test]
    fn writing_removes_notes_that_are_gone() {
        let dir = std::env::temp_dir().join(format!("tabby-phone-{}", crate::syncdb::now_ms()));
        let mut snapshot = Snapshot { notes: vec![note("a", "Uno", None), note("b", "Due", None)], ..Snapshot::default() };
        write(&dir, DIR_NAME, &snapshot).unwrap();
        assert!(dir.join(DIR_NAME).join("Due.md").is_file());
        snapshot.notes.pop();
        write(&dir, DIR_NAME, &snapshot).unwrap();
        assert!(!dir.join(DIR_NAME).join("Due.md").exists());
        assert!(dir.join(DIR_NAME).join("Uno.md").is_file());
        remove(&dir, DIR_NAME);
        assert!(!dir.join(DIR_NAME).exists());
        assert!(is_valid_dir_name(DIR_NAME_EN));
        assert!(!is_valid_dir_name("../fuori"));
        let _ = fs::remove_dir_all(&dir);
    }
}
