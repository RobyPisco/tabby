//! Cattura rapida: una scorciatoia globale salva come nota il testo negli appunti,
//! annotando da quale finestra/app arriva.

use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use windows_sys::Win32::{
    Foundation::CloseHandle,
    System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
    UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId},
};

use crate::{db::Db, notify_changed_from, settings::SettingsState, toast};

const MAX_TITLE_CHARS: usize = 60;

/// Titolo e programma della finestra in primo piano (quella da cui si è copiato).
pub fn foreground_window() -> Option<(String, String)> {
    // SAFETY: chiamate Win32 di sola lettura su buffer locali di dimensione dichiarata.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut title = [0u16; 512];
        let len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let title = String::from_utf16_lossy(&title[..len.max(0) as usize]);

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut exe = String::new();
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !process.is_null() {
            let mut path = [0u16; 1024];
            let mut size = path.len() as u32;
            if QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut size) != 0 {
                let path = String::from_utf16_lossy(&path[..size as usize]);
                exe = path.rsplit('\\').next().unwrap_or_default().to_string();
            }
            CloseHandle(process);
        }
        Some((title, exe))
    }
}

/// Titolo della nota: la prima riga non vuota, accorciata.
fn title_from(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    if line.chars().count() > MAX_TITLE_CHARS {
        let short: String = line.chars().take(MAX_TITLE_CHARS - 1).collect();
        format!("{}…", short.trim_end())
    } else {
        line.to_string()
    }
}

/// Testo della nota con la provenienza in fondo. Se la prima riga è diventata
/// il titolo per intero, non la si ripete nel testo.
fn body_from(text: &str, title: &str, origin: Option<(String, String)>) -> String {
    let text = text.trim();
    let text = match text.split_once('\n') {
        Some((first, rest)) if first.trim() == title => rest.trim_start_matches(['\r', '\n']),
        None if text == title => "",
        _ => text,
    };
    let text = text.trim_end();
    match origin {
        Some((title, exe)) if !title.is_empty() || !exe.is_empty() => {
            let source = match (title.is_empty(), exe.is_empty()) {
                (false, false) => format!("{title} ({exe})"),
                (false, true) => title,
                _ => exe,
            };
            if text.is_empty() {
                format!("_Da: {source}_")
            } else {
                format!("{text}\n\n_Da: {source}_")
            }
        }
        _ => text.to_string(),
    }
}

pub fn capture_clipboard(app: &AppHandle) {
    // La finestra in primo piano va letta subito, prima che qualcosa prenda il focus.
    let origin = foreground_window();
    let text = app.clipboard().read_text().unwrap_or_default();
    if text.trim().is_empty() {
        toast::simple(app, "Niente da salvare", "Negli appunti non c'è testo.", |_| {});
        return;
    }
    let db = app.state::<Db>();
    let existing = db.list_active().map(|n| n.len()).unwrap_or(0);
    let color = app.state::<SettingsState>().get().color_for_new_note(existing);
    let title = title_from(&text);
    match db.create_full(&title, &body_from(&text, &title, origin), &color) {
        Ok(note) => {
            notify_changed_from(app, "capture");
            let id = note.id;
            toast::simple(app, "Salvata dagli appunti", &title, move |app| {
                crate::reminders::open_note_in_deck(app, id);
            });
        }
        Err(e) => toast::simple(app, "Nota non salvata", &e.to_string(), |_| {}),
    }
}

#[cfg(test)]
mod tests {
    use super::{body_from, title_from};

    #[test]
    fn title_is_first_line_shortened() {
        assert_eq!(title_from("\n  Ciao mondo \nresto"), "Ciao mondo");
        let long = "a".repeat(100);
        assert_eq!(title_from(&long).chars().count(), 60);
    }

    #[test]
    fn body_mentions_origin() {
        let origin = Some(("Pagina".into(), "chrome.exe".into()));
        let body = body_from("Titolo\r\nseconda riga\n", "Titolo", origin);
        assert_eq!(body, "seconda riga\n\n_Da: Pagina (chrome.exe)_");
        assert_eq!(body_from("una riga", "una riga", None), "");
        // Titolo accorciato: il testo resta intero.
        assert_eq!(body_from("riga lunga", "riga…", None), "riga lunga");
    }
}
