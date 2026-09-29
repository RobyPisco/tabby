//! Export delle note in Markdown o testo (un file per nota o un file unico) e import di file .md/.txt.

use serde::Deserialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, State, WebviewWindow};

use crate::{db::Db, db::Note, media, notify_changed, settings::SettingsState};

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Md,
    Txt,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Md => "md",
            Format::Txt => "txt",
        }
    }
}

/// Nota in Markdown, con il titolo come intestazione.
pub fn to_markdown(note: &Note) -> String {
    let title = note.title.trim();
    if title.is_empty() {
        format!("{}\n", note.body.trim_end())
    } else {
        format!("# {title}\n\n{}\n", note.body.trim_end())
    }
}

/// Nota in testo semplice: caselle come ☐/☑, senza simboli Markdown.
pub fn to_text(note: &Note) -> String {
    let mut out = String::new();
    if !note.title.trim().is_empty() {
        out.push_str(note.title.trim());
        out.push_str("\n\n");
    }
    for line in note.body.lines() {
        out.push_str(&plain_line(line));
        out.push('\n');
    }
    out
}

fn plain_line(line: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    let rest = if let Some(r) = rest.strip_prefix("- [ ] ").or_else(|| rest.strip_prefix("* [ ] ")) {
        format!("☐ {r}")
    } else if let Some(r) = ["- [x] ", "- [X] ", "* [x] ", "* [X] "].iter().find_map(|p| rest.strip_prefix(p)) {
        format!("☑ {r}")
    } else {
        // Solo le intestazioni ("## Testo") perdono i #, i tag (#parola) restano.
        let heading = rest.trim_start_matches('#');
        if heading.len() < rest.len() && heading.starts_with(' ') {
            heading.trim_start().to_string()
        } else {
            rest.to_string()
        }
    };
    let mut text = format!("{indent}{rest}");
    for marker in ["**", "__", "~~", "`"] {
        text = text.replace(marker, "");
    }
    replace_links(&text)
}

/// `![](media/x.png)` → `[immagine: x.png]`, `[testo](url)` → `testo (url)`, `[[Titolo]]` → `Titolo`.
fn replace_links(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        let image = start > 0 && rest[..start].ends_with('!');
        out.push_str(&rest[..if image { start - 1 } else { start }]);
        let after = &rest[start..];
        if let Some(inner) = after.strip_prefix("[[") {
            if let Some(end) = inner.find("]]") {
                out.push_str(&inner[..end]);
                rest = &inner[end + 2..];
                continue;
            }
        }
        if let Some(close) = after.find("](") {
            if let Some(paren) = after[close + 2..].find(')') {
                let label = &after[1..close];
                let url = &after[close + 2..close + 2 + paren];
                if image {
                    let name = url.rsplit('/').next().unwrap_or(url);
                    out.push_str(&format!("[immagine: {name}]"));
                } else if label.is_empty() {
                    out.push_str(url);
                } else {
                    out.push_str(&format!("{label} ({url})"));
                }
                rest = &after[close + 3 + paren..];
                continue;
            }
        }
        if image {
            out.push('!');
        }
        out.push('[');
        rest = &after[1..];
    }
    out.push_str(rest);
    out
}

fn render(note: &Note, format: Format) -> String {
    match format {
        Format::Md => to_markdown(note),
        Format::Txt => to_text(note),
    }
}

/// Nome di file valido su Windows ricavato dal titolo, senza doppioni.
pub fn file_name(title: &str, extension: &str, used: &mut HashSet<String>) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let mut stem: String = cleaned.trim().trim_end_matches('.').chars().take(80).collect();
    if stem.is_empty() {
        stem = "Senza titolo".into();
    }
    let mut name = format!("{stem}.{extension}");
    let mut n = 2;
    while !used.insert(name.to_lowercase()) {
        name = format!("{stem} ({n}).{extension}");
        n += 1;
    }
    name
}

/// Nomi dei file della cartella media citati nel testo (`](media/nome)`).
pub fn referenced_media(body: &str) -> Vec<String> {
    body.match_indices("](media/")
        .filter_map(|(i, pat)| {
            let rest = &body[i + pat.len()..];
            rest.find(')').map(|end| rest[..end].to_string())
        })
        .filter(|name| !name.is_empty() && !name.contains(['/', '\\']))
        .collect()
}

fn load(db: &Db, ids: &[i64]) -> Result<Vec<Note>, String> {
    ids.iter()
        .map(|id| db.get(*id).map_err(|e| e.to_string()))
        .collect()
}

/// Copia accanto all'export le immagini e gli allegati citati dalle note.
fn copy_media(app: &AppHandle, notes: &[Note], dir: &Path) -> Result<(), String> {
    let names: HashSet<String> = notes.iter().flat_map(|n| referenced_media(&n.body)).collect();
    if names.is_empty() {
        return Ok(());
    }
    let source = media::media_dir(app)?;
    let target = dir.join("media");
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    for name in names {
        let from = source.join(&name);
        if from.exists() {
            std::fs::copy(&from, target.join(&name)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Un file per nota nella cartella scelta; restituisce quanti file ha scritto.
#[tauri::command]
pub fn export_notes(
    app: AppHandle,
    db: State<Db>,
    ids: Vec<i64>,
    dir: PathBuf,
    format: Format,
) -> Result<usize, String> {
    let notes = load(&db, &ids)?;
    let mut used = HashSet::new();
    for note in &notes {
        let name = file_name(&note.title, format.extension(), &mut used);
        std::fs::write(dir.join(name), render(note, format)).map_err(|e| e.to_string())?;
    }
    if format == Format::Md {
        copy_media(&app, &notes, &dir)?;
    }
    Ok(notes.len())
}

/// Tutte le note scelte in un unico file, separate da una riga.
#[tauri::command]
pub fn export_combined(
    app: AppHandle,
    db: State<Db>,
    ids: Vec<i64>,
    path: PathBuf,
    format: Format,
) -> Result<(), String> {
    let notes = load(&db, &ids)?;
    let separator = match format {
        Format::Md => "\n---\n\n",
        Format::Txt => "\n────────\n\n",
    };
    let text = notes.iter().map(|n| render(n, format)).collect::<Vec<_>>().join(separator);
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    if format == Format::Md {
        if let Some(dir) = path.parent() {
            copy_media(&app, &notes, dir)?;
        }
    }
    Ok(())
}

/// Titolo e testo di un file importato: "# Titolo" in cima diventa il titolo,
/// altrimenti si usa il nome del file.
pub fn parse_import(file_stem: &str, content: &str) -> (String, String) {
    let content = content.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    if let Some(first) = content.lines().next() {
        if let Some(title) = first.strip_prefix("# ") {
            let body = content[first.len()..].trim_start_matches('\n').trim_end().to_string();
            return (title.trim().to_string(), body);
        }
    }
    (file_stem.to_string(), content.trim_end().to_string())
}

#[tauri::command]
pub fn import_files(window: WebviewWindow, db: State<Db>, paths: Vec<PathBuf>) -> Result<usize, String> {
    let settings = window.app_handle().state::<SettingsState>().get();
    let mut existing = db.list_active().map(|n| n.len()).unwrap_or(0);
    let mut count = 0;
    for path in paths {
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Importata");
        let (title, body) = parse_import(stem, &String::from_utf8_lossy(&bytes));
        db.create_full(&title, &body, &settings.color_for_new_note(existing))
            .map_err(|e| e.to_string())?;
        existing += 1;
        count += 1;
    }
    notify_changed(&window);
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(title: &str, body: &str) -> Note {
        Note {
            id: 1,
            title: title.into(),
            body: body.into(),
            color: "#fff".into(),
            archived: false,
            created_at: 0,
            updated_at: 0,
            deleted_at: None,
            remind_at: None,
            repeat: None,
            pinned: false,
            folder_id: None,
        }
    }

    #[test]
    fn plain_text_export() {
        let n = note(
            "Spesa",
            "## Oggi\n- [ ] **pane**\n  - [x] latte\nVedi [[Menu]] e [sito](https://x.it)\n![](media/abc.png)",
        );
        assert_eq!(
            to_text(&n),
            "Spesa\n\nOggi\n☐ pane\n  ☑ latte\nVedi Menu e sito (https://x.it)\n[immagine: abc.png]\n"
        );
    }

    #[test]
    fn markdown_export_has_heading() {
        assert_eq!(to_markdown(&note("T", "corpo\n")), "# T\n\ncorpo\n");
        assert_eq!(to_markdown(&note("", "corpo")), "corpo\n");
    }

    #[test]
    fn file_names_are_safe_and_unique() {
        let mut used = HashSet::new();
        assert_eq!(file_name("a/b: c?", "md", &mut used), "a_b_ c_.md");
        assert_eq!(file_name("", "md", &mut used), "Senza titolo.md");
        assert_eq!(file_name("", "md", &mut used), "Senza titolo (2).md");
    }

    #[test]
    fn finds_media_references() {
        let body = "![](media/a.png) testo [📎 doc.pdf](media/b-doc.pdf) ![](https://x.it/c.png)";
        assert_eq!(referenced_media(body), vec!["a.png", "b-doc.pdf"]);
    }

    #[test]
    fn import_uses_heading_as_title() {
        assert_eq!(parse_import("file", "# Titolo\r\n\r\ncorpo\r\n"), ("Titolo".into(), "corpo".into()));
        assert_eq!(parse_import("file", "solo testo"), ("file".into(), "solo testo".into()));
    }
}
