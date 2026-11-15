//! Lingua dei testi del backend (notifiche, menu della tray, messaggi d'errore).
//! "auto" segue la lingua di Windows: italiano se è italiano, altrimenti inglese.
//! La stessa scelta, lato interfaccia, è in `src/lib/i18n.ts`.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// Lingua dell'interfaccia di Windows: vero se è italiano.
fn windows_is_italian() -> bool {
    #[cfg(windows)]
    {
        // LANG_ITALIAN = 0x10; i 10 bit bassi sono la lingua, gli altri la variante (Svizzera…).
        let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
        (id & 0x3ff) == 0x10
    }
    #[cfg(not(windows))]
    {
        std::env::var("LANG").is_ok_and(|l| l.starts_with("it"))
    }
}

/// Da chiamare quando si caricano o cambiano le impostazioni.
pub fn set_language(setting: &str) {
    let english = match setting {
        "it" => false,
        "en" => true,
        _ => !windows_is_italian(),
    };
    ENGLISH.store(english, Ordering::Relaxed);
}

pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Il testo nella lingua scelta.
pub fn tr<'a>(it: &'a str, en: &'a str) -> &'a str {
    if is_english() {
        en
    } else {
        it
    }
}
