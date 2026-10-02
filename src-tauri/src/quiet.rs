//! "Non disturbare": per un po' (1 ora, fino a domattina…) o in una fascia oraria fissa
//! i promemoria non suonano. Restano in attesa e arrivano tutti insieme alla fine.

use chrono::{DateTime, Days, Local, NaiveTime, TimeZone};
use std::sync::Mutex;
use tauri::{
    menu::{MenuItem, Submenu},
    AppHandle, Emitter, Manager, Wry,
};

use crate::db::now;
use crate::reminders::Scheduler;
use crate::settings::{parse_time, Settings, SettingsState};

/// Voci del menu della tray da aggiornare quando cambia lo stato.
pub struct QuietMenu {
    pub submenu: Submenu<Wry>,
    pub off: MenuItem<Wry>,
}

/// Ultimo testo mostrato nella tray, per aggiornarla solo quando cambia.
static LAST_LABEL: Mutex<Option<String>> = Mutex::new(None);

/// `t` cade nella fascia da `from` a `to`? Anche a cavallo della mezzanotte (22:00 → 08:00).
fn in_window(t: NaiveTime, from: NaiveTime, to: NaiveTime) -> bool {
    if from <= to {
        from <= t && t < to
    } else {
        t >= from || t < to
    }
}

/// Prossimo momento (dopo `after`) in cui l'orologio segna `time`.
fn next_at(after: DateTime<Local>, time: NaiveTime) -> Option<i64> {
    let today = after.date_naive();
    [today, today.checked_add_days(Days::new(1))?]
        .into_iter()
        .filter_map(|d| Local.from_local_datetime(&d.and_time(time)).earliest())
        .find(|dt| *dt > after)
        .map(|dt| dt.timestamp())
}

/// Fine del silenzio in corso a `t`, oppure `None` se le notifiche sono attive.
pub fn quiet_until(settings: &Settings, t: i64) -> Option<i64> {
    let manual = settings.dnd_until.filter(|until| *until > t);
    let window = (|| {
        if !settings.quiet_enabled {
            return None;
        }
        let (from, to) = (parse_time(&settings.quiet_from)?, parse_time(&settings.quiet_to)?);
        let local = Local.timestamp_opt(t, 0).single()?;
        in_window(local.time(), from, to).then(|| next_at(local, to)).flatten()
    })();
    manual.max(window)
}

pub fn is_quiet(app: &AppHandle) -> bool {
    quiet_until(&app.state::<SettingsState>().get(), now()).is_some()
}

/// "Fino a domattina": la prossima volta che è l'ora di "Domani" dei promemoria.
pub fn until_morning(settings: &Settings) -> i64 {
    let time = parse_time(&settings.tomorrow_time).unwrap_or(NaiveTime::MIN);
    next_at(Local::now(), time).unwrap_or(now() + 12 * 60 * 60)
}

/// Accende "Non disturbare" fino a `until`, o lo spegne con `None`.
pub fn set_dnd(app: &AppHandle, until: Option<i64>) -> Result<Settings, String> {
    let state = app.state::<SettingsState>();
    let saved = state.set(Settings {
        dnd_until: until,
        ..state.get()
    })?;
    let _ = app.emit("settings-changed", saved.clone());
    // Spento: lo scheduler consegna subito i promemoria rimasti in attesa.
    app.state::<Scheduler>().wake();
    refresh_tray(app);
    Ok(saved)
}

/// Ctrl+Alt+D: un'ora di silenzio, o riattiva le notifiche se era già acceso a mano.
pub fn toggle(app: &AppHandle) {
    let settings = app.state::<SettingsState>().get();
    let active = settings.dnd_until.is_some_and(|t| t > now());
    let until = (!active).then(|| now() + 60 * 60);
    if let Err(e) = set_dnd(app, until) {
        eprintln!("non disturbare: {e}");
        return;
    }
    let text = match until {
        Some(t) => format!("Niente notifiche fino alle {}.", clock(t)),
        None => "Le notifiche sono di nuovo attive.".into(),
    };
    crate::toast::silent(app, "Non disturbare", &text);
}

fn clock(t: i64) -> String {
    Local
        .timestamp_opt(t, 0)
        .single()
        .map(|d| d.format("%H:%M").to_string())
        .unwrap_or_default()
}

/// Aggiorna suggerimento e menu della tray (chiamata anche dallo scheduler, per quando il silenzio finisce da solo).
pub fn refresh_tray(app: &AppHandle) {
    let until = quiet_until(&app.state::<SettingsState>().get(), now());
    let label = match until {
        Some(t) => format!("Non disturbare · fino alle {}", clock(t)),
        None => "Non disturbare".into(),
    };
    let manual = app.state::<SettingsState>().get().dnd_until.is_some_and(|t| t > now());
    let key = format!("{label}|{manual}");
    let mut last = LAST_LABEL.lock().unwrap();
    if last.as_deref() == Some(key.as_str()) {
        return;
    }
    // Lo scheduler parte prima della tray: finché non c'è, si riprova al giro dopo.
    let (Some(tray), Some(menu)) = (app.tray_by_id("main"), app.try_state::<QuietMenu>()) else {
        return;
    };
    *last = Some(key);
    let tooltip = if until.is_some() { format!("Tabby · {}", label.to_lowercase()) } else { "Tabby".into() };
    let _ = tray.set_tooltip(Some(tooltip));
    let _ = menu.submenu.set_text(&label);
    let _ = menu.off.set_enabled(manual);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hm(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    fn local(d: u32, h: u32, m: u32) -> i64 {
        Local.with_ymd_and_hms(2026, 10, d, h, m, 0).earliest().unwrap().timestamp()
    }

    #[test]
    fn window_across_midnight() {
        let (from, to) = (hm(22, 0), hm(8, 0));
        assert!(in_window(hm(23, 30), from, to));
        assert!(in_window(hm(2, 0), from, to));
        assert!(!in_window(hm(8, 0), from, to));
        assert!(!in_window(hm(12, 0), from, to));
    }

    #[test]
    fn window_in_the_day_and_empty() {
        assert!(in_window(hm(13, 0), hm(12, 30), hm(14, 0)));
        assert!(!in_window(hm(14, 0), hm(12, 30), hm(14, 0)));
        // Stessa ora d'inizio e fine: nessun silenzio.
        assert!(!in_window(hm(9, 0), hm(9, 0), hm(9, 0)));
    }

    #[test]
    fn quiet_until_end_of_window_or_manual() {
        let night = Settings {
            quiet_enabled: true,
            quiet_from: "22:00".into(),
            quiet_to: "08:00".into(),
            ..Settings::default()
        };
        assert_eq!(quiet_until(&night, local(5, 23, 0)), Some(local(6, 8, 0)));
        assert_eq!(quiet_until(&night, local(6, 7, 0)), Some(local(6, 8, 0)));
        assert_eq!(quiet_until(&night, local(6, 12, 0)), None);

        // "Non disturbare" a mano che va oltre la fascia: vince il più lungo.
        let both = Settings { dnd_until: Some(local(6, 9, 30)), ..night };
        assert_eq!(quiet_until(&both, local(6, 7, 0)), Some(local(6, 9, 30)));
        assert_eq!(quiet_until(&both, local(6, 9, 0)), Some(local(6, 9, 30)));
        assert_eq!(quiet_until(&both, local(6, 9, 30)), None);
    }
}
