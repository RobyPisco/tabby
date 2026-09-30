//! Notifiche toast di Windows condivise da promemoria e cattura rapida.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use tauri_winrt_notification::{IconCrop, LoopableSound, Sound, Toast};

use crate::settings::SettingsState;

const ICON: &[u8] = include_bytes!("../icons/128x128@2x.png");

/// In sviluppo l'app non è registrata in Windows, quindi si usa l'ID di PowerShell
/// (come fa il plugin notification di Tauri); l'installer registra il nostro identificativo.
pub fn app_id(app: &AppHandle) -> String {
    if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    }
}

/// Suono corrispondente a una chiave di `settings::SOUNDS`.
fn sound_by_name(name: &str) -> Sound {
    match name {
        "default" => Sound::Default,
        "im" => Sound::IM,
        "mail" => Sound::Mail,
        "sms" => Sound::SMS,
        "alarm" => Sound::Single(LoopableSound::Alarm),
        "alarm2" => Sound::Single(LoopableSound::Alarm2),
        "call" => Sound::Single(LoopableSound::Call),
        "call2" => Sound::Single(LoopableSound::Call2),
        _ => Sound::Reminder,
    }
}

/// Suono delle notifiche secondo le impostazioni.
pub fn sound(app: &AppHandle) -> Option<Sound> {
    let settings = app.state::<SettingsState>().get();
    settings.sound.then(|| sound_by_name(&settings.sound_name))
}

/// Il toast vuole un file: l'icona dell'app viene scritta una volta nella cartella temporanea.
fn icon_path() -> Option<PathBuf> {
    let path = std::env::temp_dir().join("tabby-toast-icon.png");
    let up_to_date = std::fs::metadata(&path).is_ok_and(|m| m.len() as usize == ICON.len());
    if !up_to_date {
        std::fs::write(&path, ICON).ok()?;
    }
    Some(path)
}

/// Icona tonda dell'app accanto al testo, come nelle notifiche moderne di Windows.
pub fn with_icon(toast: Toast) -> Toast {
    match icon_path() {
        Some(path) => toast.icon(&path, IconCrop::Circular, "Tabby"),
        None => toast,
    }
}

/// Prova di un suono dalle impostazioni, anche se le notifiche sono mute.
pub fn preview(app: &AppHandle, name: &str) {
    let result = with_icon(Toast::new(&app_id(app)))
        .title("Ecco il suono")
        .text1("Così ti avviserà Tabby.")
        .sound(Some(sound_by_name(name)))
        .show();
    if let Err(e) = result {
        eprintln!("anteprima suono non mostrata: {e:?}");
    }
}

/// Notifica semplice; `on_click` parte al clic sulla notifica.
pub fn simple(
    app: &AppHandle,
    title: &str,
    text: &str,
    on_click: impl Fn(&AppHandle) + Send + 'static,
) {
    let handle = app.clone();
    let result = with_icon(Toast::new(&app_id(app)))
        .title(title)
        .text1(text)
        .sound(sound(app))
        .on_activated(move |_| {
            on_click(&handle);
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("notifica non mostrata: {e:?}");
    }
}
