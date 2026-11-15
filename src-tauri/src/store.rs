//! Tabby installato dal Microsoft Store (pacchetto MSIX) invece che con l'installer.
//! Cambia poco: gli aggiornamenti li fa lo Store, l'avvio con Windows si attiva dalle
//! impostazioni di Windows (attività di avvio del pacchetto) e le notifiche usano
//! l'identità del pacchetto.

use std::sync::OnceLock;

/// AUMID del pacchetto ("Famiglia!App") se Tabby gira come pacchetto MSIX.
pub fn package_app_id() -> Option<String> {
    static ID: OnceLock<Option<String>> = OnceLock::new();
    ID.get_or_init(read_app_id).clone()
}

pub fn is_packaged() -> bool {
    package_app_id().is_some()
}

#[cfg(windows)]
fn read_app_id() -> Option<String> {
    use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentApplicationUserModelId;
    let mut len: u32 = 0;
    // Prima chiamata: chiede solo la lunghezza; fuori da un pacchetto risponde con un errore.
    let first = unsafe { GetCurrentApplicationUserModelId(&mut len, std::ptr::null_mut()) };
    const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
    if first != ERROR_INSUFFICIENT_BUFFER || len == 0 {
        return None;
    }
    let mut buffer = vec![0u16; len as usize];
    let result = unsafe { GetCurrentApplicationUserModelId(&mut len, buffer.as_mut_ptr()) };
    if result != 0 {
        return None;
    }
    let text = String::from_utf16_lossy(&buffer[..(len as usize).saturating_sub(1)]);
    (!text.is_empty()).then_some(text)
}

#[cfg(not(windows))]
fn read_app_id() -> Option<String> {
    None
}

/// Per l'interfaccia: niente controllo degli aggiornamenti e avvio con Windows dalle impostazioni.
#[tauri::command]
pub fn is_store_build() -> bool {
    is_packaged()
}

/// Apre "Impostazioni di Windows → App → Avvio", dove si accende l'avvio di Tabby.
#[tauri::command]
pub fn open_startup_settings(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url("ms-settings:startupapps", None::<&str>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn outside_a_package_there_is_no_package_id() {
        // I test girano come normale programma: niente identità di pacchetto.
        assert!(super::package_app_id().is_none());
    }
}
