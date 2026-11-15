use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};
use tauri::{Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::db::now;
use crate::quiet::{self, QuietMenu};
use crate::settings::SettingsState;
use crate::{
    capture, new_note_in_deck, set_autostart_enabled, show_settings, toggle_all_notes, toggle_deck, toggle_search,
    AutostartMenuItem,
};

/// Icona nell'area di notifica: clic sinistro apre "Tutte le note", clic destro il menu.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);

    // Il testo dopo il tab viene allineato a destra, come le scorciatoie nei menu di Windows.
    let all_notes = MenuItem::with_id(app, "all", crate::i18n::tr("Tutte le note\tCtrl+Alt+L", "All notes\tCtrl+Alt+L"), true, None::<&str>)?;
    let search = MenuItem::with_id(app, "search", crate::i18n::tr("Cerca ovunque\tCtrl+Alt+F", "Search everywhere\tCtrl+Alt+F"), true, None::<&str>)?;
    let new_note = MenuItem::with_id(app, "new", crate::i18n::tr("Nuova nota\tCtrl+Alt+N", "New note\tCtrl+Alt+N"), true, None::<&str>)?;
    let new_reminder = MenuItem::with_id(
        app,
        "new-reminder",
        crate::i18n::tr("Nuova nota con promemoria\tCtrl+Alt+P", "New note with reminder\tCtrl+Alt+P"),
        true,
        None::<&str>,
    )?;
    let capture_item = MenuItem::with_id(
        app,
        "capture",
        crate::i18n::tr("Salva gli appunti come nota\tCtrl+Alt+V", "Save clipboard as a note\tCtrl+Alt+V"),
        true,
        None::<&str>,
    )?;
    let deck = MenuItem::with_id(app, "deck", crate::i18n::tr("Mostra/nascondi deck\tCtrl+Alt+H", "Show/hide deck\tCtrl+Alt+H"), true, None::<&str>)?;
    let quiet_off = MenuItem::with_id(app, "dnd-off", crate::i18n::tr("Riattiva le notifiche", "Turn notifications back on"), false, None::<&str>)?;
    let quiet_menu = Submenu::with_items(
        app,
        crate::i18n::tr("Non disturbare", "Do not disturb"),
        true,
        &[
            &MenuItem::with_id(app, "dnd-30", crate::i18n::tr("Per 30 minuti", "For 30 minutes"), true, None::<&str>)?,
            &MenuItem::with_id(app, "dnd-60", crate::i18n::tr("Per 1 ora\tCtrl+Alt+D", "For 1 hour\tCtrl+Alt+D"), true, None::<&str>)?,
            &MenuItem::with_id(app, "dnd-120", crate::i18n::tr("Per 2 ore", "For 2 hours"), true, None::<&str>)?,
            &MenuItem::with_id(app, "dnd-morning", crate::i18n::tr("Fino a domattina", "Until tomorrow morning"), true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &quiet_off,
        ],
    )?;
    app.manage(QuietMenu {
        submenu: quiet_menu.clone(),
        off: quiet_off,
    });
    let settings = MenuItem::with_id(app, "settings", crate::i18n::tr("Impostazioni…", "Settings…"), true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        crate::i18n::tr("Avvia con Windows", "Start with Windows"),
        true,
        autostart_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", crate::i18n::tr("Esci", "Quit"), true, None::<&str>)?;
    // Dallo Store l'avvio con Windows si accende nelle impostazioni di Windows (attività di avvio).
    let startup_settings = MenuItem::with_id(
        app,
        "startup-settings",
        crate::i18n::tr("Avvio con Windows…", "Start with Windows…"),
        true,
        None::<&str>,
    )?;
    app.manage(AutostartMenuItem(autostart.clone()));
    let separator1 = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let start_item: &dyn tauri::menu::IsMenuItem<tauri::Wry> =
        if crate::store::is_packaged() { &startup_settings } else { &autostart };
    let menu = Menu::with_items(
        app,
        &[
            &all_notes,
            &search,
            &new_note,
            &new_reminder,
            &capture_item,
            &deck,
            &separator1,
            &quiet_menu,
            &settings,
            start_item,
            &separator2,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Tabby")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "all" => toggle_all_notes(app),
            "search" => toggle_search(app),
            "new" => new_note_in_deck(app, false),
            "new-reminder" => new_note_in_deck(app, true),
            "capture" => capture::capture_clipboard(app),
            "deck" => toggle_deck(app),
            "settings" => show_settings(app),
            "dnd-30" | "dnd-60" | "dnd-120" | "dnd-morning" | "dnd-off" => {
                let id = event.id().as_ref();
                let until = match id {
                    "dnd-off" => None,
                    "dnd-morning" => Some(quiet::until_morning(&app.state::<SettingsState>().get())),
                    _ => id[4..].parse::<i64>().ok().map(|m| now() + m * 60),
                };
                if let Err(e) = quiet::set_dnd(app, until) {
                    eprintln!("non disturbare: {e}");
                }
            }
            "autostart" => {
                // Il segno di spunta riflette lo stato reale, anche se l'operazione fallisce.
                let enable = !app.autolaunch().is_enabled().unwrap_or(false);
                let _ = set_autostart_enabled(app, enable);
                let _ = app.emit_to("all", "autostart-changed", ());
            }
            "startup-settings" => {
                let _ = crate::store::open_startup_settings(app.clone());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_all_notes(tray.app_handle());
            }
        });
    // Versione pensata per le dimensioni piccole dell'area di notifica.
    if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")) {
        builder = builder.icon(icon);
    } else if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    quiet::refresh_tray(app);
    Ok(())
}
