//! Timer / pomodoro legato a una nota: un solo timer alla volta, con notifica alla fine
//! e i pulsanti per partire con la pausa o con un altro giro.

use serde::Serialize;
use std::{
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
/// Senza timer il thread si riaddormenta comunque ogni tanto.
const IDLE_WAIT: Duration = Duration::from_secs(3600);

#[derive(Clone, Serialize)]
pub struct Timer {
    pub note_id: i64,
    /// "focus" (concentrazione) o "break" (pausa).
    pub kind: String,
    pub minutes: u32,
    pub ends_at: i64,
}

pub struct TimerState {
    current: Mutex<Option<Timer>>,
    wake: Sender<()>,
}

impl TimerState {
    pub fn get(&self) -> Option<Timer> {
        self.current.lock().unwrap().clone()
    }
}

/// Registra lo stato del timer e avvia il thread che aspetta la fine.
pub fn setup(app: &AppHandle) {
    let (tx, rx) = channel();
    app.manage(TimerState {
        current: Mutex::new(None),
        wake: tx,
    });
    let handle = app.clone();
    thread::spawn(move || loop {
        let state = handle.state::<TimerState>();
        let wait = match state.get() {
            Some(timer) if timer.ends_at <= now() => {
                *state.current.lock().unwrap() = None;
                changed(&handle);
                finished_toast(&handle, &timer);
                continue;
            }
            Some(timer) => Duration::from_secs((timer.ends_at - now()).max(1) as u64),
            None => IDLE_WAIT,
        };
        if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(wait) {
            break;
        }
    });
}

fn changed(app: &AppHandle) {
    let _ = app.emit("timer-changed", app.state::<TimerState>().get());
}

/// Fa partire un timer sulla nota (quello eventuale su un'altra nota viene sostituito).
pub fn start(app: &AppHandle, note_id: i64, minutes: u32, kind: &str) -> Result<Timer, String> {
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
    let state = app.state::<TimerState>();
    *state.current.lock().unwrap() = Some(timer.clone());
    let _ = state.wake.send(());
    changed(app);
    Ok(timer)
}

/// Aggiunge minuti al timer in corso.
pub fn extend(app: &AppHandle, minutes: u32) -> Option<Timer> {
    let state = app.state::<TimerState>();
    let timer = {
        let mut current = state.current.lock().unwrap();
        let timer = current.as_mut()?;
        timer.ends_at += i64::from(minutes.min(MAX_MINUTES)) * 60;
        timer.minutes += minutes;
        timer.clone()
    };
    let _ = state.wake.send(());
    changed(app);
    Some(timer)
}

pub fn stop(app: &AppHandle) {
    let state = app.state::<TimerState>();
    *state.current.lock().unwrap() = None;
    let _ = state.wake.send(());
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
            let _ = match action.as_deref() {
                Some("break") => start(&handle, note_id, BREAK_MINUTES, "break").map(drop),
                Some("again") => start(&handle, note_id, minutes, "focus").map(drop),
                Some("focus") => start(&handle, note_id, FOCUS_MINUTES, "focus").map(drop),
                Some("dismiss") => Ok(()),
                _ => {
                    crate::reminders::open_note_in_deck(&handle, note_id);
                    Ok(())
                }
            };
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("notifica del timer non mostrata: {e:?}");
    }
}
