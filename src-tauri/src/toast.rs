//! Notifiche toast di Windows condivise da promemoria e cattura rapida.

use tauri::{AppHandle, Manager};
use tauri_winrt_notification::{Sound, Toast};

use crate::settings::SettingsState;

/// In sviluppo l'app non è registrata in Windows, quindi si usa l'ID di PowerShell
/// (come fa il plugin notification di Tauri); l'installer registra il nostro identificativo.
pub fn app_id(app: &AppHandle) -> String {
    if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    }
}

/// Suono delle notifiche secondo le impostazioni.
pub fn sound(app: &AppHandle) -> Option<Sound> {
    app.state::<SettingsState>().get().sound.then_some(Sound::Reminder)
}

/// Notifica semplice; `on_click` parte al clic sulla notifica.
pub fn simple(
    app: &AppHandle,
    title: &str,
    text: &str,
    on_click: impl Fn(&AppHandle) + Send + 'static,
) {
    let handle = app.clone();
    let result = Toast::new(&app_id(app))
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
