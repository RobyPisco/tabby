//! Promemoria: scheduler in background, notifiche toast di Windows e azioni Posticipa/Fatto.

use chrono::{Days, Local, Months, NaiveDateTime, NaiveTime, TimeZone};
use std::{
    collections::HashMap,
    sync::{mpsc::{channel, RecvTimeoutError, Sender},
        LazyLock, Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_winrt_notification::{Scenario, Toast};

use crate::db::{now, Db, DueReminder};
use crate::notify_changed_from;
use crate::settings::SettingsState;
use crate::toast::{app_id, sound, with_icon};

/// Anche senza scadenze vicine lo scheduler ricontrolla ogni tanto:
/// copre il risveglio dalla sospensione e i cambi di ora del sistema.
const MAX_SLEEP_SECS: i64 = 30;
/// Oltre questo numero di promemoria scaduti insieme (es. all'avvio) si mostra un riepilogo.
const MAX_SEPARATE_TOASTS: usize = 3;
/// Ora a cui scatta "Domani" quando si posticipa.
const TOMORROW_HOUR: u32 = 9;

/// Oltre questo tempo dalla scadenza un promemoria ignorato non viene più ripetuto.
const REPEAT_GIVE_UP_SECS: i64 = 24 * 60 * 60;

/// Ultimo avviso di ogni promemoria non gestito; `i64::MAX` = ripetizione fermata.
static LAST_ALERT: LazyLock<Mutex<HashMap<i64, i64>>> = LazyLock::new(Default::default);

/// Stato gestito da Tauri: sveglia lo scheduler quando i promemoria cambiano.
pub struct Scheduler(Sender<()>);

impl Scheduler {
    pub fn wake(&self) {
        let _ = self.0.send(());
    }
}

pub fn start(app: &AppHandle) -> Scheduler {
    let (tx, rx) = channel();
    let app = app.clone();
    thread::spawn(move || loop {
        fire_due(&app);
        repeat_unhandled(&app);
        let db = app.state::<Db>();
        let wait = match db.next_pending_due() {
            Ok(Some(due)) => (due - now()).clamp(1, MAX_SLEEP_SECS),
            _ => MAX_SLEEP_SECS,
        };
        if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(Duration::from_secs(wait as u64)) {
            break;
        }
    });
    Scheduler(tx)
}

/// Notifica i promemoria scaduti e li segna come notificati.
fn fire_due(app: &AppHandle) {
    let db = app.state::<Db>();
    let t = now();
    let Ok(due) = db.due_reminders(t) else {
        return;
    };
    if due.is_empty() {
        return;
    }
    if due.len() > MAX_SEPARATE_TOASTS {
        show_summary_toast(app, due.len());
    } else {
        for reminder in &due {
            show_toast(app, reminder);
        }
    }
    for reminder in &due {
        let _ = db.mark_fired(reminder.note_id, t);
    }
    notify_changed_from(app, "scheduler");
}

/// Ripete notifica e suono dei promemoria lasciati senza risposta, ogni `repeat_minutes`.
fn repeat_unhandled(app: &AppHandle) {
    let minutes = app.state::<SettingsState>().get().repeat_minutes;
    let mut last = LAST_ALERT.lock().unwrap();
    if minutes == 0 {
        last.clear();
        return;
    }
    let t = now();
    let Ok(pending) = app.state::<Db>().unhandled_reminders(t - REPEAT_GIVE_UP_SECS) else {
        return;
    };
    last.retain(|id, _| pending.iter().any(|r| r.note_id == *id));
    for reminder in &pending {
        // La prima volta si parte a contare da adesso: l'avviso originale è già uscito.
        let at = *last.entry(reminder.note_id).or_insert(t);
        if at != i64::MAX && t - at >= i64::from(minutes) * 60 {
            last.insert(reminder.note_id, t);
            show_toast(app, reminder);
        }
    }
}

fn show_toast(app: &AppHandle, reminder: &DueReminder) {
    let title = if reminder.title.trim().is_empty() {
        "Promemoria"
    } else {
        reminder.title.trim()
    };
    let preview: String = reminder
        .body
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .take(120)
        .collect();
    let when = Local
        .timestamp_opt(reminder.due_at, 0)
        .single()
        .map(|d| format!("⏰ {}", friendly_when(d)))
        .unwrap_or_default();

    let handle = app.clone();
    let note_id = reminder.note_id;
    let result = with_icon(Toast::new(&app_id(app)))
        .title(title)
        .text1(&preview)
        .text2(&when)
        .scenario(Scenario::Reminder)
        .sound(sound(app))
        .add_button("⏱ 10 min", "snooze10")
        .add_button("🕐 1 ora", "snooze60")
        .add_button("🌅 Domani", "tomorrow")
        .add_button("✓ Fatto", "done")
        .on_activated(move |action| {
            // Clic sul corpo della notifica: nessuna azione, si apre la nota.
            apply_action(&handle, note_id, action.as_deref().unwrap_or("open"));
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("notifica non mostrata: {e:?}");
    }
}

/// "oggi alle 13:08", "ieri alle 09:00", "30/09 alle 13:08".
fn friendly_when(due: chrono::DateTime<Local>) -> String {
    let today = Local::now().date_naive();
    let time = due.format("%H:%M");
    match (due.date_naive() - today).num_days() {
        0 => format!("oggi alle {time}"),
        -1 => format!("ieri alle {time}"),
        1 => format!("domani alle {time}"),
        _ => format!("{} alle {time}", due.format("%d/%m")),
    }
}

fn show_summary_toast(app: &AppHandle, count: usize) {
    let handle = app.clone();
    let result = with_icon(Toast::new(&app_id(app)))
        .title(&format!("{count} promemoria scaduti"))
        .text1("Aprili da \"Tutte le note\" o dal deck.")
        .sound(sound(app))
        .on_activated(move |_| {
            crate::show_all_notes_window(&handle);
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("notifica non mostrata: {e:?}");
    }
}

/// Azioni dalla notifica o dall'interfaccia: "snooze10", "snooze60", "tomorrow", "done", "open".
pub fn apply_action(app: &AppHandle, note_id: i64, action: &str) {
    let db = app.state::<Db>();
    let t = now();
    let result = match action {
        "snooze10" => db.snooze_reminder(note_id, t + 10 * 60),
        "snooze60" => db.snooze_reminder(note_id, t + 60 * 60),
        "tomorrow" => db.snooze_reminder(note_id, tomorrow_at(TOMORROW_HOUR).unwrap_or(t + 86_400)),
        "done" => complete(&db, note_id, t),
        "open" => {
            LAST_ALERT.lock().unwrap().insert(note_id, i64::MAX);
            open_note_in_deck(app, note_id);
            return;
        }
        _ => return,
    };
    if let Err(e) = result {
        eprintln!("azione promemoria {action} fallita: {e}");
    }
    app.state::<Scheduler>().wake();
    notify_changed_from(app, "scheduler");
}

/// "Fatto": un promemoria singolo sparisce, uno ricorrente passa alla prossima scadenza futura.
fn complete(db: &Db, note_id: i64, t: i64) -> rusqlite::Result<()> {
    match db.reminder(note_id)? {
        Some((due, repeat)) if repeat != "none" => match next_occurrence(due, &repeat, t) {
            Some(next) => db.set_reminder(note_id, next, &repeat),
            None => db.clear_reminder(note_id),
        },
        _ => db.clear_reminder(note_id),
    }
}

/// Prima ripetizione della serie successiva a `after`, calcolata in ora locale
/// (così "ogni giorno alle 9" resta alle 9 anche col cambio dell'ora legale).
pub fn next_occurrence(due: i64, repeat: &str, after: i64) -> Option<i64> {
    if !matches!(repeat, "daily" | "weekly" | "monthly") {
        return None;
    }
    let base = Local.timestamp_opt(due, 0).single()?.naive_local();
    (1..=100_000).find_map(|k| {
        let local = nth(base, repeat, k)?;
        let ts = Local.from_local_datetime(&local).earliest()?.timestamp();
        (ts > after).then_some(ts)
    })
}

/// La k-esima ripetizione, contata sempre dalla data di partenza della serie.
fn nth(base: NaiveDateTime, repeat: &str, k: u32) -> Option<NaiveDateTime> {
    match repeat {
        "daily" => base.checked_add_days(Days::new(k.into())),
        "weekly" => base.checked_add_days(Days::new(7 * u64::from(k))),
        // Il 31 diventa l'ultimo giorno dei mesi più corti, poi torna al 31.
        "monthly" => base.checked_add_months(Months::new(k)),
        _ => None,
    }
}

fn tomorrow_at(hour: u32) -> Option<i64> {
    let date = Local::now().date_naive().succ_opt()?;
    let time = NaiveTime::from_hms_opt(hour, 0, 0)?;
    Some(Local.from_local_datetime(&date.and_time(time)).earliest()?.timestamp())
}

pub fn open_note_in_deck(app: &AppHandle, note_id: i64) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit_to("main", "open-note", note_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(y: i32, m: u32, d: u32, h: u32) -> i64 {
        Local.with_ymd_and_hms(y, m, d, h, 0, 0).earliest().unwrap().timestamp()
    }

    #[test]
    fn daily_keeps_local_hour_across_dst() {
        // In Italia l'ora legale finisce il 25 ottobre 2026.
        let due = local(2026, 10, 24, 9);
        let next = next_occurrence(due, "daily", due).unwrap();
        assert_eq!(next, local(2026, 10, 25, 9));
    }

    #[test]
    fn skips_missed_occurrences() {
        let due = local(2026, 9, 1, 9);
        let after = local(2026, 9, 10, 12);
        assert_eq!(next_occurrence(due, "weekly", after), Some(local(2026, 9, 15, 9)));
    }

    #[test]
    fn monthly_clamps_to_month_end() {
        let due = local(2026, 1, 31, 9);
        assert_eq!(next_occurrence(due, "monthly", due), Some(local(2026, 2, 28, 9)));
        let march = next_occurrence(due, "monthly", local(2026, 2, 28, 9));
        assert_eq!(march, Some(local(2026, 3, 31, 9)));
    }

    #[test]
    fn once_has_no_next() {
        let due = local(2026, 9, 1, 9);
        assert_eq!(next_occurrence(due, "none", due), None);
    }
}
