//! Impostazioni dell'app, salvate in `settings.json` nella cartella dei dati.

use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Mutex};

/// Colori delle note (pastello); le stesse tinte di `COLORS` in `src/lib/notes.ts`.
pub const COLORS: [&str; 12] = [
    "#b5d3f7", "#b3e5cf", "#d4c8f2", "#f7dc82", "#f7c4b5", "#f2b8d4",
    "#a8e4e0", "#d4ec9e", "#f9cc8f", "#f5a9a3", "#e6d5bd", "#d5d9e0",
];

/// Suoni disponibili per le notifiche (chiavi salvate nelle impostazioni).
pub const SOUNDS: [&str; 9] = [
    "reminder", "default", "im", "mail", "sms", "alarm", "alarm2", "call", "call2",
];

/// Intervalli di importazione di Google Calendar: settimane ("2w") o mesi ("3m").
pub const GCAL_RANGES: [&str; 6] = ["1w", "2w", "3w", "1m", "2m", "3m"];

/// Giorni prima dell'inizio in cui un evento di Google compare nel deck; 0 = quando inizia,
/// 365 = sempre (l'importazione arriva al massimo a 3 mesi).
pub const GCAL_DECK_DAYS: [u32; 7] = [0, 1, 3, 7, 14, 30, 365];

const REPEAT_MINUTES_MAX: u32 = 120;
const NOTE_WIDTH_RANGE: (u32, u32) = (280, 520);
const EDITOR_SIZE_RANGE: (u32, u32) = (13, 24);
const DECK_TRANSPARENCY_MAX: u32 = 100;
const BACKUP_KEEP_RANGE: (u32, u32) = (1, 60);

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
    /// Quanto avanti importare gli eventi: una delle chiavi di `GCAL_RANGES`.
    pub gcal_sync_range: String,
    /// ID dei calendari Google selezionati dall'utente.
    pub gcal_calendar_ids: Vec<String>,
    /// Usa il proxy di Windows (PAC, credenziali dell'utente) per le chiamate a Google.
    pub gcal_use_proxy: bool,
    /// Quanti giorni prima dell'inizio un evento compare nel deck: uno dei `GCAL_DECK_DAYS`.
    pub gcal_deck_days: u32,
    /// Backup automatico giornaliero di note e immagini.
    pub backup_enabled: bool,
    /// Cartella dei backup; `None` = `backups` nella cartella dei dati.
    pub backup_dir: Option<String>,
    /// Quanti backup tenere: i più vecchi vengono cancellati.
    pub backup_keep: u32,
    /// "Non disturbare" fino a questo momento (secondi Unix); `None` = spento.
    pub dnd_until: Option<i64>,
    /// Fascia oraria fissa senza notifiche, da `quiet_from` a `quiet_to` ("HH:MM", anche a cavallo della mezzanotte).
    pub quiet_enabled: bool,
    pub quiet_from: String,
    pub quiet_to: String,
    /// Ora a cui scatta "Domani" quando si posticipa un promemoria ("HH:MM").
    pub tomorrow_time: String,
    /// Posizione verticale della pillola sul bordo dx/sx: frazione 0.0 (in alto) - 1.0 (in basso)
    /// dell'altezza disponibile; 0.5 = centro.
    pub deck_offset: f64,
    /// Hash del PIN globale che protegge le note protette; `None` = nessun PIN impostato.
    pub pin_hash: Option<String>,
    /// Salt usato per `pin_hash`.
    pub pin_salt: Option<String>,
    /// Ultima versione per cui è già stata mostrata la notifica di aggiornamento disponibile;
    /// `None` = nessuna ancora notificata. Evita di notificare la stessa versione più volte.
    pub last_update_notified: Option<String>,
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
            gcal_sync_range: "2w".into(),
            gcal_calendar_ids: vec![],
            gcal_use_proxy: false,
            gcal_deck_days: crate::db::DECK_GCAL_DAYS,
            backup_enabled: true,
            backup_dir: None,
            backup_keep: 7,
            dnd_until: None,
            quiet_enabled: false,
            quiet_from: "22:00".into(),
            quiet_to: "08:00".into(),
            tomorrow_time: "09:00".into(),
            deck_offset: 0.5,
            pin_hash: None,
            pin_salt: None,
            last_update_notified: None,
        }
    }
}

/// "HH:MM" con due cifre per l'ora (come `<input type="time">`), altrimenti il valore predefinito.
fn time_or(value: String, fallback: &str) -> String {
    parse_time(&value).map_or_else(|| fallback.into(), |t| t.format("%H:%M").to_string())
}

/// Ore e minuti da "HH:MM".
pub fn parse_time(text: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(text.trim(), "%H:%M").ok()
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
            gcal_sync_range: one_of(self.gcal_sync_range, &GCAL_RANGES, &default.gcal_sync_range),
            gcal_calendar_ids: self.gcal_calendar_ids,
            gcal_use_proxy: self.gcal_use_proxy,
            gcal_deck_days: if GCAL_DECK_DAYS.contains(&self.gcal_deck_days) {
                self.gcal_deck_days
            } else {
                default.gcal_deck_days
            },
            backup_enabled: self.backup_enabled,
            backup_dir: self.backup_dir.filter(|d| !d.trim().is_empty()),
            backup_keep: self.backup_keep.clamp(BACKUP_KEEP_RANGE.0, BACKUP_KEEP_RANGE.1),
            dnd_until: self.dnd_until.filter(|t| *t > crate::db::now()),
            quiet_enabled: self.quiet_enabled,
            quiet_from: time_or(self.quiet_from, &default.quiet_from),
            quiet_to: time_or(self.quiet_to, &default.quiet_to),
            tomorrow_time: time_or(self.tomorrow_time, &default.tomorrow_time),
            deck_offset: self.deck_offset.clamp(0.0, 1.0),
            pin_hash: self.pin_hash,
            pin_salt: self.pin_salt,
            last_update_notified: self.last_update_notified,
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

    pub fn has_pin(&self) -> bool {
        self.pin_hash.is_some() && self.pin_salt.is_some()
    }

    /// Imposta il PIN la prima volta; usa `change_pin` per sostituirne uno già impostato.
    pub fn set_pin(&mut self, pin: &str) -> Result<(), String> {
        if self.has_pin() {
            return Err("un PIN è già impostato".into());
        }
        if !crate::pin::is_valid(pin) {
            return Err("il PIN deve avere 4-6 cifre".into());
        }
        let (hash, salt) = crate::pin::hash(pin);
        self.pin_hash = Some(hash);
        self.pin_salt = Some(salt);
        Ok(())
    }

    pub fn verify_pin(&self, pin: &str) -> bool {
        match (&self.pin_hash, &self.pin_salt) {
            (Some(hash), Some(salt)) => crate::pin::verify(pin, salt, hash),
            _ => false,
        }
    }

    pub fn change_pin(&mut self, old_pin: &str, new_pin: &str) -> Result<(), String> {
        if !self.verify_pin(old_pin) {
            return Err("PIN attuale sbagliato".into());
        }
        if !crate::pin::is_valid(new_pin) {
            return Err("il PIN deve avere 4-6 cifre".into());
        }
        let (hash, salt) = crate::pin::hash(new_pin);
        self.pin_hash = Some(hash);
        self.pin_salt = Some(salt);
        Ok(())
    }

    /// Reset del PIN dimenticato: nessuna verifica richiesta.
    pub fn clear_pin(&mut self) {
        self.pin_hash = None;
        self.pin_salt = None;
    }

    /// Vero se è già stata mostrata la notifica di aggiornamento per questa versione.
    pub fn update_already_notified(&self, version: &str) -> bool {
        self.last_update_notified.as_deref() == Some(version)
    }

    pub fn mark_update_notified(&mut self, version: &str) {
        self.last_update_notified = Some(version.to_string());
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

    pub fn set_pin(&self, pin: &str) -> Result<Settings, String> {
        let mut current = self.get();
        current.set_pin(pin)?;
        self.set(current)
    }

    pub fn change_pin(&self, old_pin: &str, new_pin: &str) -> Result<Settings, String> {
        let mut current = self.get();
        current.change_pin(old_pin, new_pin)?;
        self.set(current)
    }

    pub fn clear_pin(&self) -> Result<Settings, String> {
        let mut current = self.get();
        current.clear_pin();
        self.set(current)
    }

    pub fn update_already_notified(&self, version: &str) -> bool {
        self.get().update_already_notified(version)
    }

    pub fn mark_update_notified(&self, version: &str) -> Result<Settings, String> {
        let mut current = self.get();
        current.mark_update_notified(version);
        self.set(current)
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
    // Fino alla 1.3 l'intervallo era un numero di settimane (1 o 2).
    if let Some(weeks) = obj.get("gcal_sync_weeks").and_then(|w| w.as_u64()) {
        if !obj.contains_key("gcal_sync_range") {
            obj.insert("gcal_sync_range".into(), format!("{weeks}w").into());
        }
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
            deck_offset: -1.0,
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.side, "right");
        assert_eq!(s.note_width, 520);
        assert_eq!(s.editor_font, "hand");
        assert_eq!(s.new_note_color, "cycle");
        assert_eq!(s.deck_transparency, 100);
        assert_eq!(s.deck_offset, 0.0);
    }

    #[test]
    fn deck_offset_valid_value_is_kept() {
        let s = Settings { deck_offset: 0.3, ..Settings::default() }.sanitized();
        assert_eq!(s.deck_offset, 0.3);
    }

    #[test]
    fn deck_offset_above_range_falls_back_to_max() {
        let s = Settings { deck_offset: 2.0, ..Settings::default() }.sanitized();
        assert_eq!(s.deck_offset, 1.0);
    }

    #[test]
    fn invalid_times_fall_back() {
        let s = Settings {
            quiet_from: "25:00".into(),
            quiet_to: "7:30".into(),
            tomorrow_time: "domani".into(),
            backup_keep: 0,
            dnd_until: Some(1),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(s.quiet_from, "22:00");
        assert_eq!(s.quiet_to, "07:30");
        assert_eq!(s.tomorrow_time, "09:00");
        assert_eq!(s.backup_keep, 1);
        assert_eq!(s.dnd_until, None);
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

    #[test]
    fn setting_a_pin_makes_it_verifiable() {
        let mut s = Settings::default();
        s.set_pin("1234").unwrap();
        assert!(s.has_pin());
        assert!(s.verify_pin("1234"));
    }

    #[test]
    fn cannot_set_a_pin_when_one_already_exists() {
        let mut s = Settings::default();
        s.set_pin("1234").unwrap();
        assert!(s.set_pin("5678").is_err());
        assert!(s.verify_pin("1234"));
    }

    #[test]
    fn set_pin_rejects_an_invalid_pin() {
        let mut s = Settings::default();
        assert!(s.set_pin("abcd").is_err());
        assert!(!s.has_pin());
    }

    #[test]
    fn changing_the_pin_replaces_it() {
        let mut s = Settings::default();
        s.set_pin("1234").unwrap();
        s.change_pin("1234", "5678").unwrap();
        assert!(s.verify_pin("5678"));
        assert!(!s.verify_pin("1234"));
    }

    #[test]
    fn changing_the_pin_with_the_wrong_current_pin_fails() {
        let mut s = Settings::default();
        s.set_pin("1234").unwrap();
        assert!(s.change_pin("0000", "5678").is_err());
        assert!(s.verify_pin("1234"));
    }

    #[test]
    fn clearing_the_pin_removes_it() {
        let mut s = Settings::default();
        s.set_pin("1234").unwrap();
        s.clear_pin();
        assert!(!s.has_pin());
        assert!(!s.verify_pin("1234"));
    }

    #[test]
    fn a_version_not_yet_notified_is_not_already_notified() {
        let s = Settings::default();
        assert!(!s.update_already_notified("1.4.4"));
    }

    #[test]
    fn marking_a_version_notified_makes_it_already_notified() {
        let mut s = Settings::default();
        s.mark_update_notified("1.4.4");
        assert!(s.update_already_notified("1.4.4"));
        assert!(!s.update_already_notified("1.4.5"));
    }

    #[test]
    fn old_week_count_becomes_range() {
        assert_eq!(parse(r#"{ "gcal_sync_weeks": 1 }"#).unwrap().gcal_sync_range, "1w");
        assert_eq!(parse("{}").unwrap().gcal_sync_range, "2w");
        let s = Settings { gcal_sync_range: "6m".into(), ..Settings::default() }.sanitized();
        assert_eq!(s.gcal_sync_range, "2w");
    }
}
