//! La finestra del deck: ancorata al bordo dello schermo, riposizionata se cambiano
//! monitor, risoluzione o scala, e con il cursore letto da Rust (a riposo è click-through).

use serde::Serialize;
use std::{thread, time::Duration};
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, Monitor, WebviewWindow};
use windows_sys::Win32::{
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowLongW, GetWindowRect, SetWindowPos, GWL_STYLE, HWND_BOTTOM,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, WS_CAPTION,
    },
};

use crate::settings::{Settings, SettingsState};

/// Ogni quanti giri del poller (~16 ms l'uno) si ricontrolla la geometria dello schermo.
const MONITOR_CHECK_EVERY: u32 = 120;
/// Ogni quanti giri si controlla se c'è un'app a tutto schermo (~0,5 s).
const FULLSCREEN_CHECK_EVERY: u32 = 30;

#[derive(Clone, Serialize)]
struct CursorPayload {
    /// Coordinate logiche del cursore relative all'angolo alto-sinistro della finestra.
    x: f64,
    y: f64,
    /// Altezza logica della finestra (serve al frontend per centrare il deck).
    height: f64,
}

#[derive(Serialize)]
pub struct MonitorInfo {
    /// Identificativo di Windows (es. `\\.\DISPLAY2`), salvato nelle impostazioni.
    name: String,
    /// Descrizione per l'interfaccia, es. "Schermo 2 · 1920×1080".
    label: String,
}

/// Il monitor scelto nelle impostazioni, se collegato; altrimenti il principale.
fn target_monitor(window: &WebviewWindow, settings: &Settings) -> Option<Monitor> {
    if let Some(wanted) = &settings.monitor {
        let found = window
            .available_monitors()
            .ok()
            .and_then(|list| list.into_iter().find(|m| m.name() == Some(wanted)));
        if found.is_some() {
            return found;
        }
    }
    window.primary_monitor().ok().flatten()
}

/// La finestra in primo piano è a tutto schermo (gioco, video, presentazione)?
/// Vale se copre tutto il monitor, oppure se copre l'area di lavoro ed è senza barra del
/// titolo (i giochi "in finestra senza bordi" lasciano visibile la barra delle applicazioni).
/// Desktop e barra delle applicazioni non contano: coprono lo schermo ma sono la shell.
fn foreground_is_fullscreen() -> bool {
    // SAFETY: chiamate Win32 di sola lettura su strutture locali inizializzate.
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        let class = String::from_utf16_lossy(&class[..len]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return false;
        }
        let mut rect = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return false;
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return false;
        }
        let covers = |m: windows_sys::Win32::Foundation::RECT| {
            rect.left <= m.left && rect.top <= m.top && rect.right >= m.right && rect.bottom >= m.bottom
        };
        let borderless = GetWindowLongW(hwnd, GWL_STYLE) as u32 & WS_CAPTION != WS_CAPTION;
        covers(info.rcMonitor) || (borderless && covers(info.rcWork))
    }
}

/// Ancora la finestra al bordo scelto, a tutta altezza della work area del monitor.
pub fn dock(window: &WebviewWindow, settings: &Settings) -> tauri::Result<()> {
    let Some(monitor) = target_monitor(window, settings) else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let pos = area.position.to_logical::<f64>(scale);
    let size = area.size.to_logical::<f64>(scale);
    let width = settings.dock_width();
    let x = if settings.side == "left" {
        pos.x
    } else {
        pos.x + size.width - width
    };
    window.set_size(LogicalSize::new(width, size.height))?;
    window.set_position(LogicalPosition::new(x, pos.y))?;
    Ok(())
}

/// Ciò che, se cambia, richiede di riposizionare il deck.
fn geometry(window: &WebviewWindow, settings: &Settings) -> Option<(i32, i32, u32, u32, u64)> {
    let monitor = target_monitor(window, settings)?;
    let area = monitor.work_area();
    Some((
        area.position.x,
        area.position.y,
        area.size.width,
        area.size.height,
        monitor.scale_factor().to_bits(),
    ))
}

/// Legge il cursore a ~60 Hz e lo inoltra al frontend: con la finestra click-through
/// la webview non riceve eventi mouse, quindi la prossimità va rilevata da Rust.
/// Ogni ~0,5 s controlla se un'app a tutto schermo è in primo piano: in quel caso
/// toglie al deck lo "sempre in primo piano". Ogni ~2 s controlla anche se lo schermo è cambiato (monitor staccato, risoluzione,
/// scala, barra delle applicazioni spostata) e in quel caso riposiziona il deck.
pub fn spawn_cursor_poller(window: WebviewWindow) {
    thread::spawn(move || {
        let mut last_geometry = None;
        let mut tick = 0u32;
        let mut on_top = true;
        loop {
            thread::sleep(Duration::from_millis(16));
            tick = tick.wrapping_add(1);
            // Con un'app a tutto schermo il deck smette di stare sopra a tutto.
            if tick % FULLSCREEN_CHECK_EVERY == 0 {
                let want_top = !foreground_is_fullscreen();
                if want_top != on_top && window.set_always_on_top(want_top).is_ok() {
                    on_top = want_top;
                    if !want_top {
                        // Senza "sempre in primo piano" resterebbe comunque sopra alle finestre normali.
                        if let Ok(hwnd) = window.hwnd() {
                            // SAFETY: l'handle appartiene alla nostra finestra, ancora viva.
                            unsafe {
                                SetWindowPos(hwnd.0 as _, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                            }
                        }
                    }
                }
            }
            if tick % MONITOR_CHECK_EVERY == 0 {
                let settings = window.state::<SettingsState>().get();
                let current = geometry(&window, &settings);
                if last_geometry.is_some() && current != last_geometry {
                    let _ = dock(&window, &settings);
                }
                last_geometry = current;
            }
            let (Ok(cursor), Ok(origin), Ok(size)) = (
                window.cursor_position(),
                window.outer_position(),
                window.outer_size(),
            ) else {
                continue;
            };
            let scale = window.scale_factor().unwrap_or(1.0);
            let payload = CursorPayload {
                x: (cursor.x - f64::from(origin.x)) / scale,
                y: (cursor.y - f64::from(origin.y)) / scale,
                height: f64::from(size.height) / scale,
            };
            let _ = window.emit_to("main", "cursor", payload);
        }
    });
}

#[tauri::command]
pub fn list_monitors(window: WebviewWindow) -> Vec<MonitorInfo> {
    let primary = window.primary_monitor().ok().flatten().and_then(|m| m.name().cloned());
    window
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let name = m.name()?.clone();
            let size = m.size();
            let main = if primary.as_ref() == Some(&name) { " (principale)" } else { "" };
            Some(MonitorInfo {
                label: format!("Schermo {} · {}×{}{main}", i + 1, size.width, size.height),
                name,
            })
        })
        .collect()
}
