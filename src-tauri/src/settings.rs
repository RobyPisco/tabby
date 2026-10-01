//! Impostazioni dell'app, salvate in `settings.json` nella cartella dei dati.

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};

/// Colori delle note (pastello); le stesse tinte di `COLORS` in `src/lib/notes.ts`.
pub const COLORS: [&str; 6] = ["#b5d3f7", "#b3e5cf", "#d4c8f2", "#f7dc82", "#f7c4b5", "#f2b8d4"];

/// Suoni disponibili per le notifiche (chiavi salvate nelle impostazioni).
pub const SOUNDS: [&str; 9] = [
    "reminder", "default", "im", "mail", "sms", "alarm", "alarm2", "call", "call2",
];

const REPEAT_MINUTES_MAX: u32 = 120;
const NOTE_WIDTH_RANGE: (u32, u32) = (280, 520);
const EDITOR_SIZE_RANGE: (u32, u32) = (13, 24);
const DECK_TRANSPARENCY_MAX: u32 = 100;

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Bordo dello schermo del deck: "right" o "left".
    pub side: String,
    /// Larghezza della nota aperta nel deck, in pixel logici.
    pub note_width: u32,
    /// Nome del monitor su cui stare; `None` = monitor principale.
    pub monitor: Option<String>,
    /// Carattere del testo delle note: "hand", "sans" o "mono".
    pub editor_font: String,
    pub editor_size: u32,
    /// Colore delle note nuove: "cycle" (a rotazione) o uno dei `COLORS`.
    pub new_note_color: String,
    /// Suono delle notifiche.
    pub sound: bool,
    /// Quale suono: una delle chiavi di `SOUNDS`.
    pub sound_name: String,
    /// Ogni quanti minuti ripetere la notifica finché il promemoria non è gestito; 0 = mai.
    pub repeat_minutes: u32,
    /// Tema di "Tutte le note": "auto", "light" o "dark".
    pub theme: String,
    /// Trasparenza del vetro delle schede nel deck, da 0 (opaco) a 100.
    pub deck_transparency: u32,
    /// Integrazione Google Calendar attiva.
    pub gcal_enabled: bool,
    /// Quante settimane di eventi importare (1 o 2).
    pub gcal_sync_weeks: u32,
    /// ID dei calendari Google selezionati dall'utente.
    pub gcal_calendar_ids: Vec<String>,
    /// Usa il proxy di Windows (PAC, credenziali dell'utente) per le chiamate a Google.
    pub gcal_use_proxy: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            side: "right".into(),
            note_width: 340,
            monitor: None,
            editor_font: "hand".into(),
            editor_size: 18,
            new_note_color: "cycle".into(),
            sound: true,
            sound_name: "reminder".into(),
            repeat_minutes: 0,
            theme: "auto".into(),
            deck_transparency: 45,
            gcal_enabled: false,
            gcal_sync_weeks: 2,
            gcal_calendar_ids: vec![],
            gcal_use_proxy: false,
        }
    }
}

fn one_of(value: String, allowed: &[&str], fallback: &str) -> String {
    if allowed.contains(&value.as_str()) {
        value
    } else {
        fallback.into()
    }
}

impl Settings {
    /// Riporta nei limiti i valori arrivati dal file o dall'interfaccia.
    pub fn sanitized(self) -> Self {
        let default = Settings::default();
        Settings {
            side: one_of(self.side, &["right", "left"], &default.side),
            note_width: self.note_width.clamp(NOTE_WIDTH_RANGE.0, NOTE_WIDTH_RANGE.1),
            monitor: self.monitor.filter(|m| !m.is_empty()),
            editor_font: one_of(self.editor_font, &["hand", "sans", "mono"], &default.editor_font),
            editor_size: self.editor_size.clamp(EDITOR_SIZE_RANGE.0, EDITOR_SIZE_RANGE.1),
            new_note_color: if self.new_note_color == "cycle" || COLORS.contains(&self.new_note_color.as_str()) {
                self.new_note_color
            } else {
                default.new_note_color
            },
            sound: self.sound,
            sound_name: one_of(self.sound_name, &SOUNDS, &default.sound_name),
            repeat_minutes: self.repeat_minutes.min(REPEAT_MINUTES_MAX),
            theme: one_of(self.theme, &["auto", "light", "dark"], &default.theme),
            deck_transparency: self.deck_transparency.min(DECK_TRANSPARENCY_MAX),
            gcal_enabled: self.gcal_enabled,
            gcal_sync_weeks: self.gcal_sync_weeks.clamp(1, 2),
            gcal_calendar_ids: self.gcal_calendar_ids,
            gcal_use_proxy: self.gcal_use_proxy,
        }
    }

    /// Larghezza della finestra del deck: nota + schede + margini.
    /// Deve restare uguale a `dockWidth` in `src/lib/settings.svelte.ts`.
    pub fn dock_width(&self) -> f64 {
        f64::from(self.note_width) + 200.0
    }

    /// Colore per una nota nuova, dato il numero di note già nel deck.
    pub fn color_for_new_note(&self, existing: usize) -> String {
        if self.new_note_color == "cycle" {
            COLORS[existing % COLORS.len()].to_string()
        } else {
            self.new_note_color.clone()
        }
    }
}

pub struct SettingsState {
    path: PathBuf,
    value: Mutex<Settings>,
}

impl SettingsState {
    /// Un file mancante o rovinato non blocca l'avvio: si riparte dai valori predefiniti.
    pub fn load(path: PathBuf) -> Self {
        let value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| parse(&text))
            .unwrap_or_default()
            .sanitized();
        crate::http::set_use_proxy(value.gcal_use_proxy);
        SettingsState {
            path,
            value: Mutex::new(value),
        }
    }

    pub fn get(&self) -> Settings {
        self.value.lock().unwrap().clone()
    }

    pub fn set(&self, settings: Settings) -> Result<Settings, String> {
        let settings = settings.sanitized();
        let text = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, text).map_err(|e| e.to_string())?;
        *self.value.lock().unwrap() = settings.clone();
        crate::http::set_use_proxy(settings.gcal_use_proxy);
        Ok(settings)
    }
}

/// Legge `settings.json`. Chi aveva già usato Google Calendar prima che esistesse l'opzione
/// del proxy (quando il proxy di Windows era sempre attivo) lo ritrova acceso.
fn parse(text: &str) -> Option<Settings> {
    let mut json: serde_json::Value = serde_json::from_str(text).ok()?;
    let obj = json.as_object_mut()?;
    if obj.contains_key("gcal_enabled") && !obj.contains_key("gcal_use_proxy") {
        obj.insert("gcal_use_proxy".into(), true.into());
    }
    serde_json::from_value(json).ok()
}

#[cfg(test)]
mod tests {
    use super::{parse, Settings};

    #[test]
    fn invalid_values_fall_back() {
        let s = Settings {
            side: "top".into(),
            note_width: 9999,
            editor_font: "comic".into(),
            new_note_color: "#000000".into(),
            deck_transparency: 500,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.side, "right");
        assert_eq!(s.note_width, 520);
        assert_eq!(s.editor_font, "hand");
        assert_eq!(s.new_note_color, "cycle");
        assert_eq!(s.deck_transparency, 100);
    }

    #[test]
    fn partial_file_uses_defaults() {
        let s: Settings = serde_json::from_str(r#"{ "side": "left" }"#).unwrap();
        assert_eq!(s.side, "left");
        assert_eq!(s.note_width, 340);
        assert!(s.sound);
    }

    #[test]
    fn proxy_default_depends_on_previous_calendar_use() {
        // Installazione nuova o aggiornamento da una versione senza Calendar: spento.
        assert!(!parse(r#"{ "side": "left" }"#).unwrap().gcal_use_proxy);
        // Aveva già usato Calendar quando il proxy era sempre attivo: resta acceso.
        assert!(parse(r#"{ "gcal_enabled": true }"#).unwrap().gcal_use_proxy);
        // Scelta esplicita dell'utente: si rispetta.
        assert!(!parse(r#"{ "gcal_enabled": true, "gcal_use_proxy": false }"#).unwrap().gcal_use_proxy);
    }
}
