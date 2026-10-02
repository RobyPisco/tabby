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
    capture, new_note_in_deck, set_autostart_enabled, show_settings, toggle_all_notes, toggle_deck,
    AutostartMenuItem,
};

/// Icona nell'area di notifica: clic sinistro apre "Tutte le note", clic destro il menu.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let autostart_enabled = app.autolaunch().is_enabled().unwrap_or(false);

    // Il testo dopo il tab viene allineato a destra, come le scorciatoie nei menu di Windows.
    let all_notes = MenuItem::with_id(app, "all", "Tutte le note\tCtrl+Alt+L", true, None::<&str>)?;
    let new_note = MenuItem::with_id(app, "new", "Nuova nota\tCtrl+Alt+N", true, None::<&str>)?;
    let new_reminder = MenuItem::with_id(
        app,
        "new-reminder",
        "Nuova nota con promemoria\tCtrl+Alt+P",
        true,
        None::<&str>,
    )?;
    let capture_item = MenuItem::with_id(
        app,
        "capture",
        "Salva gli appunti come nota\tCtrl+Alt+V",
        true,
        None::<&str>,
    )?;
    let deck = MenuItem::with_id(app, "deck", "Mostra/nascondi deck\tCtrl+Alt+H", true, None::<&str>)?;
    let quiet_off = MenuItem::with_id(app, "dnd-off", "Riattiva le notifiche", false, None::<&str>)?;
    let quiet_menu = Submenu::with_items(
        app,
        "Non disturbare",
        true,
        &[
            &MenuItem::with_id(app, "dnd-30", "Per 30 minuti", true, None::<&str>)?,
            &MenuItem::with_id(app, "dnd-60", "Per 1 ora	Ctrl+Alt+D", true, None::<&str>)?,
            &MenuItem::with_id(app, "dnd-120", "Per 2 ore", true, None::<&str>)?,
            &MenuItem::with_id(app, "dnd-morning", "Fino a domattina", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &quiet_off,
        ],
    )?;
    app.manage(QuietMenu {
        submenu: quiet_menu.clone(),
        off: quiet_off,
    });
    let settings = MenuItem::with_id(app, "settings", "Impostazioni…", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Avvia con Windows",
        true,
        autostart_enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Esci", true, None::<&str>)?;
    app.manage(AutostartMenuItem(autostart.clone()));
    let menu = Menu::with_items(
        app,
        &[
            &all_notes,
            &new_note,
            &new_reminder,
            &capture_item,
            &deck,
            &PredefinedMenuItem::separator(app)?,
            &quiet_menu,
            &settings,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Tabby")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "all" => toggle_all_notes(app),
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
