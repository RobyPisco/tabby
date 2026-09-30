mod capture;
mod db;
mod dock;
mod export;
mod media;
mod organize;
mod reminders;
mod settings;
mod toast;
mod tray;

use db::{Db, Filter, Note};
use organize::{Folder, FolderCounts, NoteRef, TagCount, Version};
use reminders::Scheduler;
use serde::Serialize;
use settings::{Settings, SettingsState};
use tauri::{menu::CheckMenuItem, AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent, Wry};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Evento `notes-changed`: le altre finestre ricaricano le note. `source` è l'etichetta
/// della finestra che ha fatto la modifica, che la ignora per non sovrascrivere ciò che sta scrivendo.
#[derive(Clone, Serialize)]
struct NotesChanged {
    source: String,
}

fn notify_changed(window: &WebviewWindow) {
    notify_changed_from(window.app_handle(), window.label());
}

/// Come `notify_changed`, per le modifiche che non partono da una finestra (es. lo scheduler).
pub(crate) fn notify_changed_from(app: &AppHandle, source: &str) {
    let _ = app.emit(
        "notes-changed",
        NotesChanged {
            source: source.to_string(),
        },
    );
}

/// A riposo la finestra è click-through; quando il deck è aperto diventa interattiva.
#[tauri::command]
fn set_interactive(window: WebviewWindow, interactive: bool) -> Result<(), String> {
    window
        .set_ignore_cursor_events(!interactive)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_notes(db: State<Db>) -> Result<Vec<Note>, String> {
    db.list_active().map_err(|e| e.to_string())
}

#[tauri::command]
fn search_notes(
    db: State<Db>,
    query: String,
    filter: Filter,
    folder: Option<i64>,
    tag: Option<String>,
) -> Result<Vec<Note>, String> {
    db.search(&query, filter, folder, tag.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_pinned(window: WebviewWindow, db: State<Db>, id: i64, pinned: bool) -> Result<(), String> {
    db.set_pinned(id, pinned).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn list_folders(db: State<Db>) -> Result<Vec<Folder>, String> {
    db.list_folders().map_err(|e| e.to_string())
}

#[tauri::command]
fn folder_counts(db: State<Db>) -> Result<FolderCounts, String> {
    db.folder_counts().map_err(|e| e.to_string())
}

fn folder_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty() {
        Err("il nome della cartella è vuoto".into())
    } else {
        Ok(name)
    }
}

#[tauri::command]
fn create_folder(window: WebviewWindow, db: State<Db>, name: String) -> Result<i64, String> {
    let id = db
        .create_folder(folder_name(&name)?)
        .map_err(|_| format!("esiste già una cartella \"{}\"", name.trim()))?;
    notify_changed(&window);
    Ok(id)
}

#[tauri::command]
fn rename_folder(window: WebviewWindow, db: State<Db>, id: i64, name: String) -> Result<(), String> {
    db.rename_folder(id, folder_name(&name)?)
        .map_err(|_| format!("esiste già una cartella \"{}\"", name.trim()))?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn delete_folder(window: WebviewWindow, db: State<Db>, id: i64) -> Result<(), String> {
    db.delete_folder(id).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn set_folder(
    window: WebviewWindow,
    db: State<Db>,
    ids: Vec<i64>,
    folder_id: Option<i64>,
) -> Result<(), String> {
    db.set_folder(&ids, folder_id).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn list_tags(db: State<Db>) -> Result<Vec<TagCount>, String> {
    db.list_tags().map_err(|e| e.to_string())
}

#[tauri::command]
fn list_titles(db: State<Db>) -> Result<Vec<NoteRef>, String> {
    db.list_titles().map_err(|e| e.to_string())
}

#[tauri::command]
fn find_note_by_title(db: State<Db>, title: String) -> Result<Option<Note>, String> {
    db.find_by_title(&title).map_err(|e| e.to_string())
}

#[tauri::command]
fn backlinks(db: State<Db>, id: i64) -> Result<Vec<NoteRef>, String> {
    db.backlinks(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_versions(db: State<Db>, note_id: i64) -> Result<Vec<Version>, String> {
    db.list_versions(note_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn restore_version(window: WebviewWindow, db: State<Db>, version_id: i64) -> Result<(), String> {
    db.restore_version(version_id).map_err(|e| e.to_string())?;
    // Anche la finestra che ripristina deve ricaricare: qui il testo cambia "da fuori".
    notify_changed_from(window.app_handle(), "restore");
    Ok(())
}

/// Apre "Tutte le note" su una nota precisa (es. link a una nota archiviata dal deck).
#[tauri::command]
fn show_note_in_all(app: AppHandle, id: i64) {
    show_all_notes_window(&app);
    let _ = app.emit_to("all", "select-note", id);
}

#[tauri::command]
fn create_note(window: WebviewWindow, db: State<Db>, color: String) -> Result<Note, String> {
    let note = db.create(&color).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(note)
}

#[tauri::command]
fn update_note(
    window: WebviewWindow,
    db: State<Db>,
    id: i64,
    title: String,
    body: String,
) -> Result<(), String> {
    db.update(id, &title, &body).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn set_archived(
    window: WebviewWindow,
    db: State<Db>,
    ids: Vec<i64>,
    archived: bool,
) -> Result<(), String> {
    db.set_archived(&ids, archived).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn delete_notes(window: WebviewWindow, db: State<Db>, ids: Vec<i64>) -> Result<(), String> {
    db.delete(&ids).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn set_color(window: WebviewWindow, db: State<Db>, id: i64, color: String) -> Result<(), String> {
    db.set_color(id, &color).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn set_trashed(
    window: WebviewWindow,
    db: State<Db>,
    ids: Vec<i64>,
    trashed: bool,
) -> Result<(), String> {
    db.set_trashed(&ids, trashed).map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn empty_trash(window: WebviewWindow, db: State<Db>) -> Result<(), String> {
    db.empty_trash().map_err(|e| e.to_string())?;
    notify_changed(&window);
    Ok(())
}

const REPEATS: [&str; 4] = ["none", "daily", "weekly", "monthly"];

#[tauri::command]
fn set_reminder(
    window: WebviewWindow,
    db: State<Db>,
    scheduler: State<Scheduler>,
    note_id: i64,
    due_at: i64,
    repeat: String,
) -> Result<(), String> {
    if !REPEATS.contains(&repeat.as_str()) {
        return Err(format!("ricorrenza sconosciuta: {repeat}"));
    }
    db.set_reminder(note_id, due_at, &repeat)
        .map_err(|e| e.to_string())?;
    scheduler.wake();
    notify_changed(&window);
    Ok(())
}

#[tauri::command]
fn clear_reminder(
    window: WebviewWindow,
    db: State<Db>,
    scheduler: State<Scheduler>,
    note_id: i64,
) -> Result<(), String> {
    db.clear_reminder(note_id).map_err(|e| e.to_string())?;
    scheduler.wake();
    notify_changed(&window);
    Ok(())
}

/// Posticipa / Fatto dall'interfaccia: stesse azioni dei pulsanti della notifica.
#[tauri::command]
fn reminder_action(app: AppHandle, note_id: i64, action: String) {
    reminders::apply_action(&app, note_id, &action);
}

#[tauri::command]
fn get_settings(state: State<SettingsState>) -> Settings {
    state.get()
}

/// Salva le impostazioni, riposiziona il deck e avvisa le finestre.
#[tauri::command]
fn set_settings(app: AppHandle, state: State<SettingsState>, settings: Settings) -> Result<Settings, String> {
    let saved = state.set(settings)?;
    if let Some(main) = app.get_webview_window("main") {
        dock::dock(&main, &saved).map_err(|e| e.to_string())?;
    }
    let _ = app.emit("settings-changed", saved.clone());
    Ok(saved)
}

#[tauri::command]
fn preview_sound(app: AppHandle, name: String) {
    toast::preview(&app, &name);
}

/// Voce "Avvia con Windows" del menu della tray, da tenere allineata con le impostazioni.
pub(crate) struct AutostartMenuItem(pub CheckMenuItem<Wry>);

#[tauri::command]
fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    set_autostart_enabled(&app, enabled)
}

pub(crate) fn set_autostart_enabled(app: &AppHandle, enabled: bool) -> Result<bool, String> {
    let launcher = app.autolaunch();
    let result = if enabled { launcher.enable() } else { launcher.disable() };
    let actual = launcher.is_enabled().unwrap_or(false);
    if let Some(item) = app.try_state::<AutostartMenuItem>() {
        let _ = item.0.set_checked(actual);
    }
    result.map_err(|e| e.to_string())?;
    Ok(actual)
}

/// Cartella dei dati; `TABBY_DATA_DIR` la sposta (utile per prove e screenshot con note di esempio).
pub(crate) fn data_dir(app: &AppHandle) -> Result<std::path::PathBuf, tauri::Error> {
    match std::env::var_os("TABBY_DATA_DIR") {
        Some(dir) => Ok(dir.into()),
        None => app.path().app_data_dir(),
    }
}

/// Apre in Esplora risorse la cartella con database, immagini e impostazioni.
#[tauri::command]
fn open_data_folder(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let dir = data_dir(&app).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Apre "Tutte le note" con il pannello delle impostazioni.
pub(crate) fn show_settings(app: &AppHandle) {
    show_all_notes_window(app);
    let _ = app.emit_to("all", "open-settings", ());
}

/// Scorciatoie globali.
const SHORTCUT_ALL_NOTES: Code = Code::KeyL; // Ctrl+Alt+L: finestra "Tutte le note"
const SHORTCUT_NEW_NOTE: Code = Code::KeyN; // Ctrl+Alt+N: nuova nota nel deck
const SHORTCUT_TOGGLE_DECK: Code = Code::KeyH; // Ctrl+Alt+H: mostra/nascondi il deck
const SHORTCUT_NEW_REMINDER: Code = Code::KeyP; // Ctrl+Alt+P: nuova nota con promemoria
const SHORTCUT_CAPTURE: Code = Code::KeyV; // Ctrl+Alt+V: salva gli appunti come nota
const SHORTCUTS: [Code; 5] = [
    SHORTCUT_ALL_NOTES,
    SHORTCUT_NEW_NOTE,
    SHORTCUT_TOGGLE_DECK,
    SHORTCUT_NEW_REMINDER,
    SHORTCUT_CAPTURE,
];

fn ctrl_alt(code: Code) -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), code)
}

/// Mostra la finestra "Tutte le note" in primo piano, o la nasconde se è già davanti.
pub(crate) fn toggle_all_notes(app: &AppHandle) {
    let Some(window) = app.get_webview_window("all") else {
        return;
    };
    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);
    if visible && focused && !window.is_minimized().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show_all_notes_window(app);
    }
}

pub(crate) fn show_all_notes_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("all") else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit_to("all", "all-notes-shown", ());
}

/// Nasconde il deck (anche la pillola) o lo fa ricomparire.
pub(crate) fn toggle_deck(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(true) {
        let _ = window.hide();
    } else {
        let _ = window.show();
    }
}

/// Apre il deck su una nota nuova, pronta per scrivere; con `with_reminder`
/// si apre anche la scelta della data del promemoria.
pub(crate) fn new_note_in_deck(app: &AppHandle, with_reminder: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit_to("main", "new-note", with_reminder);
}

#[tauri::command]
fn show_all_notes(app: AppHandle) {
    toggle_all_notes(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Deve essere il primo plugin: un secondo avvio mostra "Tutte le note" ed esce.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_all_notes_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, pressed, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    match pressed.key {
                        SHORTCUT_ALL_NOTES => toggle_all_notes(app),
                        SHORTCUT_NEW_NOTE => new_note_in_deck(app, false),
                        SHORTCUT_NEW_REMINDER => new_note_in_deck(app, true),
                        SHORTCUT_TOGGLE_DECK => toggle_deck(app),
                        SHORTCUT_CAPTURE => capture::capture_clipboard(app),
                        _ => {}
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            set_interactive,
            list_notes,
            search_notes,
            create_note,
            update_note,
            set_archived,
            set_color,
            set_trashed,
            empty_trash,
            set_reminder,
            clear_reminder,
            reminder_action,
            set_pinned,
            list_folders,
            folder_counts,
            create_folder,
            rename_folder,
            delete_folder,
            set_folder,
            list_tags,
            list_titles,
            find_note_by_title,
            backlinks,
            list_versions,
            restore_version,
            show_note_in_all,
            media::save_image,
            media::save_attachment,
            media::open_media,
            media::save_media_as,
            media::media_path,
            export::export_notes,
            export::export_combined,
            export::import_files,
            dock::list_monitors,
            get_settings,
            set_settings,
            preview_sound,
            get_autostart,
            set_autostart,
            open_data_folder,
            delete_notes,
            show_all_notes
        ])
        .on_window_event(|window, event| {
            // Chiudere "Tutte le note" la nasconde soltanto: riaprirla è istantaneo.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "all" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let data_dir = data_dir(app.handle())?;
            std::fs::create_dir_all(&data_dir)?;
            app.manage(Db::open(&data_dir.join("notes.db"))?);
            app.manage(SettingsState::load(data_dir.join("settings.json")));
            // Pulizia dei file media non più usati, senza rallentare l'avvio.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                if let Err(e) = media::remove_unused(&handle) {
                    eprintln!("pulizia media non riuscita: {e}");
                }
            });
            // Lo scheduler notifica subito anche i promemoria scaduti ad app chiusa.
            let scheduler = reminders::start(app.handle());
            app.manage(scheduler);

            let window = app
                .get_webview_window("main")
                .expect("finestra main mancante");
            dock::dock(&window, &app.state::<SettingsState>().get())?;
            window.set_ignore_cursor_events(true)?;
            dock::spawn_cursor_poller(window);

            // Registrate una per una: se un'altra app ne occupa già una, si salta solo quella.
            for code in SHORTCUTS {
                if let Err(e) = app.global_shortcut().register(ctrl_alt(code)) {
                    eprintln!("scorciatoia Ctrl+Alt+{code:?} non disponibile: {e}");
                }
            }

            tray::create(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
