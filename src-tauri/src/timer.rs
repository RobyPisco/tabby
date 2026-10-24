//! Timer / pomodoro legato a una nota: un solo timer alla volta, con notifica alla fine
//! e i pulsanti per partire con la pausa o con un altro giro.

use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, RecvTimeoutError, Sender},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_winrt_notification::{Scenario, Toast};

use crate::db::{now, Db};
use crate::toast::{app_id, sound, with_icon};

/// Durate del pomodoro classico, proposte dai pulsanti della notifica.
const FOCUS_MINUTES: u32 = 25;
const BREAK_MINUTES: u32 = 5;
const MAX_MINUTES: u32 = 8 * 60;
/// Ogni quanto lo scheduler ricontrolla i timer scaduti, anche senza essere svegliato
/// esplicitamente (stesso schema di MAX_SLEEP_SECS in reminders.rs).
const MAX_SLEEP_SECS: u64 = 30;

#[derive(Clone, Serialize)]
pub struct Timer {
    pub note_id: i64,
    /// "focus" (concentrazione) o "break" (pausa).
    pub kind: String,
    pub minutes: u32,
    pub ends_at: i64,
}

pub struct TimerState {
    timers: Mutex<HashMap<i64, Timer>>,
}

impl TimerState {
    pub fn new() -> Self {
        TimerState { timers: Mutex::new(HashMap::new()) }
    }

    /// Fa partire un timer sulla nota (un eventuale timer già attivo su quella nota viene sostituito;
    /// i timer sulle altre note restano intatti).
    pub fn start(&self, note_id: i64, minutes: u32, kind: &str) -> Result<Timer, String> {
        if !matches!(kind, "focus" | "break") {
            return Err(format!("tipo di timer sconosciuto: {kind}"));
        }
        let minutes = minutes.clamp(1, MAX_MINUTES);
        let timer = Timer {
            note_id,
            kind: kind.into(),
            minutes,
            ends_at: now() + i64::from(minutes) * 60,
        };
        self.timers.lock().unwrap().insert(note_id, timer.clone());
        Ok(timer)
    }

    pub fn get(&self, note_id: i64) -> Option<Timer> {
        self.timers.lock().unwrap().get(&note_id).cloned()
    }

    /// Aggiunge minuti al timer della nota indicata, se esiste.
    pub fn extend(&self, note_id: i64, minutes: u32) -> Option<Timer> {
        let mut timers = self.timers.lock().unwrap();
        let timer = timers.get_mut(&note_id)?;
        timer.ends_at += i64::from(minutes.min(MAX_MINUTES)) * 60;
        timer.minutes += minutes;
        Some(timer.clone())
    }

    /// Rimuove il timer della nota indicata, se esiste.
    pub fn stop(&self, note_id: i64) {
        self.timers.lock().unwrap().remove(&note_id);
    }

    /// Tutti i timer attivi, uno per nota.
    pub fn all(&self) -> Vec<Timer> {
        self.timers.lock().unwrap().values().cloned().collect()
    }

    /// La scadenza più vicina tra i timer attivi, se ce n'è almeno uno.
    pub fn next_due(&self) -> Option<i64> {
        self.timers.lock().unwrap().values().map(|t| t.ends_at).min()
    }

    /// Rimuove e restituisce i timer scaduti (`ends_at <= now`); gli altri restano attivi.
    pub fn take_due(&self, now: i64) -> Vec<Timer> {
        let mut timers = self.timers.lock().unwrap();
        let due_ids: Vec<i64> = timers
            .values()
            .filter(|t| t.ends_at <= now)
            .map(|t| t.note_id)
            .collect();
        due_ids.into_iter().filter_map(|id| timers.remove(&id)).collect()
    }
}

/// Sveglia lo scheduler quando un timer parte, si allunga o si ferma (cambi che aspettare
/// il prossimo giro di poll renderebbe percepibili in ritardo).
pub struct TimerScheduler(Sender<()>);

impl TimerScheduler {
    pub fn wake(&self) {
        let _ = self.0.send(());
    }
}

/// Registra lo stato dei timer e avvia il thread che controlla le scadenze: dorme fino alla
/// prossima scadenza reale (come lo scheduler dei promemoria in `reminders.rs`), con
/// `MAX_SLEEP_SECS` solo come limite massimo quando non c'è nulla in attesa.
pub fn setup(app: &AppHandle) -> TimerScheduler {
    app.manage(TimerState::new());
    let (tx, rx) = channel();
    let handle = app.clone();
    thread::spawn(move || loop {
        let state = handle.state::<TimerState>();
        let due = state.take_due(now());
        if !due.is_empty() {
            changed(&handle);
            for timer in &due {
                finished_toast(&handle, timer);
            }
        }
        let wait = match state.next_due() {
            Some(due_at) => (due_at - now()).clamp(1, MAX_SLEEP_SECS as i64) as u64,
            None => MAX_SLEEP_SECS,
        };
        if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(Duration::from_secs(wait)) {
            break;
        }
    });
    TimerScheduler(tx)
}

/// Notifica il frontend con l'elenco aggiornato di tutti i timer attivi.
fn changed(app: &AppHandle) {
    let _ = app.emit("timer-changed", app.state::<TimerState>().all());
}

/// Fa partire un timer sulla nota, sveglia lo scheduler e notifica il frontend.
pub fn start(app: &AppHandle, note_id: i64, minutes: u32, kind: &str) -> Result<Timer, String> {
    let timer = app.state::<TimerState>().start(note_id, minutes, kind)?;
    app.state::<TimerScheduler>().wake();
    changed(app);
    Ok(timer)
}

/// Allunga il timer della nota indicata, sveglia lo scheduler e notifica il frontend.
pub fn extend(app: &AppHandle, note_id: i64, minutes: u32) -> Option<Timer> {
    let timer = app.state::<TimerState>().extend(note_id, minutes)?;
    app.state::<TimerScheduler>().wake();
    changed(app);
    Some(timer)
}

/// Ferma il timer della nota indicata, sveglia lo scheduler e notifica il frontend.
pub fn stop(app: &AppHandle, note_id: i64) {
    app.state::<TimerState>().stop(note_id);
    app.state::<TimerScheduler>().wake();
    changed(app);
}

/// La notifica di fine suona anche in "Non disturbare": il timer l'hai fatto partire tu.
fn finished_toast(app: &AppHandle, timer: &Timer) {
    let title = app
        .state::<Db>()
        .get(timer.note_id)
        .map(|n| n.title.trim().to_string())
        .unwrap_or_default();
    let title = if title.is_empty() { "Senza titolo".to_string() } else { title };
    let focus = timer.kind == "focus";
    let (heading, text) = if focus {
        ("🍅 Tempo!", format!("{title} · {} min di concentrazione", timer.minutes))
    } else {
        ("☕ Pausa finita", format!("Si riparte con «{title}»?"))
    };
    let handle = app.clone();
    let (note_id, minutes) = (timer.note_id, timer.minutes);
    let mut toast = with_icon(Toast::new(&app_id(app)))
        .title(heading)
        .text1(&text)
        .scenario(Scenario::Reminder)
        .sound(sound(app));
    toast = if focus {
        toast
            .add_button(&format!("☕ Pausa {BREAK_MINUTES} min"), "break")
            .add_button(&format!("🔁 Altri {minutes} min"), "again")
    } else {
        toast.add_button(&format!("🍅 {FOCUS_MINUTES} min"), "focus")
    };
    let result = toast
        .add_button("Chiudi", "dismiss")
        .on_activated(move |action| {
            match action.as_deref() {
                Some("break") => drop(start(&handle, note_id, BREAK_MINUTES, "break")),
                Some("again") => drop(start(&handle, note_id, minutes, "focus")),
                Some("focus") => drop(start(&handle, note_id, FOCUS_MINUTES, "focus")),
                Some("dismiss") => {}
                _ => crate::reminders::open_note_in_deck(&handle, note_id),
            }
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("notifica del timer non mostrata: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::TimerState;

    #[test]
    fn starting_a_timer_does_not_touch_other_notes() {
        let state = TimerState::new();
        state.start(1, 25, "focus").unwrap();
        state.start(2, 5, "break").unwrap();
        assert!(state.get(1).is_some());
        assert_eq!(state.get(2).unwrap().minutes, 5);
    }

    #[test]
    fn starting_again_on_the_same_note_replaces_it() {
        let state = TimerState::new();
        state.start(1, 25, "focus").unwrap();
        let replaced = state.start(1, 5, "break").unwrap();
        assert_eq!(replaced.kind, "break");
        assert_eq!(replaced.minutes, 5);
        assert_eq!(state.get(1).unwrap().minutes, 5);
    }

    #[test]
    fn extend_lengthens_only_the_given_notes_timer() {
        let state = TimerState::new();
        state.start(1, 10, "focus").unwrap();
        state.start(2, 10, "focus").unwrap();
        let extended = state.extend(1, 5).unwrap();
        assert_eq!(extended.minutes, 15);
        assert_eq!(state.get(1).unwrap().minutes, 15);
        assert_eq!(state.get(2).unwrap().minutes, 10);
    }

    #[test]
    fn extend_on_a_note_without_a_timer_does_nothing() {
        let state = TimerState::new();
        assert!(state.extend(1, 5).is_none());
    }

    #[test]
    fn stop_removes_only_the_given_notes_timer() {
        let state = TimerState::new();
        state.start(1, 10, "focus").unwrap();
        state.start(2, 10, "focus").unwrap();
        state.stop(1);
        assert!(state.get(1).is_none());
        assert!(state.get(2).is_some());
    }

    #[test]
    fn take_due_removes_and_returns_only_expired_timers() {
        let state = TimerState::new();
        state.start(1, 1, "focus").unwrap(); // scade tra 1 minuto
        state.start(2, 100, "focus").unwrap(); // scade tra 100 minuti
        let soon = crate::db::now() + 2 * 60; // tra 2 minuti: solo la nota 1 è scaduta

        let due = state.take_due(soon);

        assert_eq!(due.len(), 1);
        assert_eq!(due[0].note_id, 1);
        assert!(state.get(1).is_none());
        assert!(state.get(2).is_some());
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let state = TimerState::new();
        assert!(state.start(1, 10, "nap").is_err());
    }

    #[test]
    fn minutes_out_of_range_are_clamped() {
        let state = TimerState::new();
        let timer = state.start(1, 99999, "focus").unwrap();
        assert_eq!(timer.minutes, 8 * 60);
    }

    #[test]
    fn all_lists_every_active_timer() {
        let state = TimerState::new();
        state.start(1, 10, "focus").unwrap();
        state.start(2, 5, "break").unwrap();
        let mut ids: Vec<i64> = state.all().into_iter().map(|t| t.note_id).collect();
        ids.sort();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn next_due_is_the_earliest_ends_at_among_active_timers() {
        let state = TimerState::new();
        state.start(1, 100, "focus").unwrap(); // scade più tardi
        state.start(2, 10, "focus").unwrap(); // scade prima
        assert_eq!(state.next_due(), state.get(2).map(|t| t.ends_at));
    }

    #[test]
    fn next_due_is_none_without_active_timers() {
        let state = TimerState::new();
        assert!(state.next_due().is_none());
    }
}
