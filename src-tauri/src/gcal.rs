//! Integrazione Google Calendar: OAuth2 PKCE, sync eventi -> note.
//!
//! Flusso:
//!   1. `connect()` apre il browser e ascolta su localhost il callback OAuth2.
//!   2. I token vengono salvati in gcal_token.json nella cartella dati.
//!   3. Lo `Syncer` gira su un thread dedicato e sincronizza ogni 15 min.
//!   4. Ogni evento diventa una nota nella cartella "Calendario Google" con
//!      promemoria all'orario di inizio; note di eventi rimossi vanno nel cestino.
//!
//! # Client ID
//! Crea un progetto su https://console.cloud.google.com/, abilita "Google Calendar API"
//! e crea credenziali "App desktop". Compila CLIENT_ID e CLIENT_SECRET qui sotto.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Local, TimeDelta, TimeZone, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{
        mpsc::{channel, RecvTimeoutError, Sender},
        Mutex,
    },
    thread,
    time::Duration,
};
use tauri::{AppHandle, Manager};

use crate::{db::Db, http, notify_changed_from, settings::SettingsState};

// -- Credenziali OAuth2 -------------------------------------------------------
// Sostituisci con le credenziali del tuo progetto Google Cloud.
// Per app desktop il Client Secret NON e' confidenziale (Google lo sa).
const CLIENT_ID:     &str = "808139008740-6jsdhjvdkog0iakjj1vjv8fkbkpafajp.apps.googleusercontent.com";
const CLIENT_SECRET: &str = "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";

const SCOPES: &str = "https://www.googleapis.com/auth/calendar.readonly";
const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const CALENDAR_API: &str = "https://www.googleapis.com/calendar/v3";

const SYNC_INTERVAL_SECS: u64 = 15 * 60;
const MAX_WAIT_SECS: u64 = 30;

// -- Strutture dati -----------------------------------------------------------

#[derive(Serialize, Deserialize, Clone)]
struct Token {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GcalCalendar {
    pub id: String,
    pub summary: String,
    #[serde(rename = "backgroundColor", default)]
    pub color: String,
}

#[derive(Serialize, Clone)]
pub struct GcalStatus {
    pub connected: bool,
    pub last_sync: Option<i64>,
    pub last_error: Option<String>,
}

// -- Stato gestito da Tauri ---------------------------------------------------

pub struct GcalState {
    token_path: PathBuf,
    token: Mutex<Option<Token>>,
    last_sync: Mutex<Option<i64>>,
    last_error: Mutex<Option<String>>,
}

impl GcalState {
    pub fn new(token_path: PathBuf) -> Self {
        let token = Self::load_token(&token_path);
        GcalState {
            token_path,
            token: Mutex::new(token),
            last_sync: Mutex::new(None),
            last_error: Mutex::new(None),
        }
    }

    fn load_token(path: &PathBuf) -> Option<Token> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save_token(&self, token: &Token) {
        if let Ok(text) = serde_json::to_string_pretty(token) {
            let _ = std::fs::write(&self.token_path, text);
        }
    }

    pub fn is_connected(&self) -> bool {
        self.token.lock().unwrap().is_some()
    }

    pub fn status(&self) -> GcalStatus {
        GcalStatus {
            connected: self.is_connected(),
            last_sync: *self.last_sync.lock().unwrap(),
            last_error: self.last_error.lock().unwrap().clone(),
        }
    }

    pub fn disconnect(&self) {
        *self.token.lock().unwrap() = None;
        let _ = std::fs::remove_file(&self.token_path);
    }

    fn access_token(&self) -> Result<String, String> {
        let mut guard = self.token.lock().unwrap();
        let token = guard.as_mut().ok_or("non connesso")?;
        let now = Utc::now().timestamp();
        if token.expires_at - now < 60 {
            let refreshed = do_refresh_token(&token.refresh_token)
                .map_err(|e| format!("refresh token fallito: {e}"))?;
            *token = refreshed;
            self.save_token(token);
        }
        Ok(token.access_token.clone())
    }
}

// -- OAuth2 PKCE --------------------------------------------------------------

pub fn connect(state: &GcalState) -> Result<(), String> {
    let mut raw = [0u8; 32];
    rand::rng().fill_bytes(&mut raw);
    let verifier = URL_SAFE_NO_PAD.encode(raw);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}/");

    let params = serde_urlencoded::to_string([
        ("client_id", CLIENT_ID),
        ("redirect_uri", &redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("access_type", "offline"),
        ("prompt", "consent"),
    ])
    .map_err(|e| e.to_string())?;
    let auth_url = format!("{AUTH_URL}?{params}");

    open::that(&auth_url).map_err(|e| format!("impossibile aprire il browser: {e}"))?;

    let code = wait_for_code(listener)?;
    let token = exchange_code(&code, &verifier, &redirect_uri)
        .map_err(|e| format!("scambio codice fallito: {e}"))?;

    state.save_token(&token);
    *state.token.lock().unwrap() = Some(token);
    Ok(())
}

fn wait_for_code(listener: TcpListener) -> Result<String, String> {
    listener.set_nonblocking(true).ok();
    let start = std::time::Instant::now();
    let stream = loop {
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed().as_secs() > 120 {
                    return Err("timeout: autorizzazione non completata in 2 minuti".into());
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).map_err(|e| e.to_string())?;
    let html = b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
        <html><body style='font-family:sans-serif;padding:40px'>\
        <h2>&#10003; Tabby collegato a Google Calendar</h2>\
        <p>Puoi chiudere questa scheda e tornare a Tabby.</p></body></html>";
    let _ = (&stream).write_all(html);
    let path = request_line.split_whitespace().nth(1).unwrap_or("");
    path.split('?')
        .nth(1)
        .and_then(|q| {
            q.split('&')
                .find(|p| p.starts_with("code="))
                .map(|p| p.trim_start_matches("code=").to_string())
        })
        .ok_or_else(|| {
            let err = path.split('?').nth(1).and_then(|q| {
                q.split('&')
                    .find(|p| p.starts_with("error="))
                    .map(|p| p.trim_start_matches("error=").to_string())
            });
            format!("autorizzazione negata: {}", err.unwrap_or_default())
        })
}

fn exchange_code(code: &str, verifier: &str, redirect_uri: &str) -> Result<Token, String> {
    let resp = post_form(
        TOKEN_URL,
        &[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier),
        ],
    )?;
    token_from(&resp, None)
}

fn do_refresh_token(refresh_tok: &str) -> Result<Token, String> {
    let resp = post_form(
        TOKEN_URL,
        &[
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("refresh_token", refresh_tok),
            ("grant_type", "refresh_token"),
        ],
    )?;
    token_from(&resp, Some(refresh_tok))
}

fn token_from(resp: &serde_json::Value, old_refresh: Option<&str>) -> Result<Token, String> {
    if let Some(err) = resp["error"].as_str() {
        let detail = resp["error_description"].as_str().unwrap_or("");
        return Err(format!("{err} {detail}").trim().to_string());
    }
    Ok(Token {
        access_token: resp["access_token"].as_str().ok_or("risposta senza access_token")?.to_string(),
        refresh_token: resp["refresh_token"]
            .as_str()
            .or(old_refresh)
            .ok_or("nessun refresh_token")?
            .to_string(),
        expires_at: Utc::now().timestamp() + resp["expires_in"].as_i64().unwrap_or(3600),
    })
}

// -- HTTP ---------------------------------------------------------------------

fn parse_json(resp: http::Response) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&resp.body)
        .map_err(|_| format!("risposta non valida da Google (HTTP {})", resp.status))
}

fn post_form(url: &str, fields: &[(&str, &str)]) -> Result<serde_json::Value, String> {
    let body = serde_urlencoded::to_string(fields).map_err(|e| e.to_string())?;
    let resp = http::request(
        "POST",
        url,
        &[("Content-Type", "application/x-www-form-urlencoded")],
        Some(body.as_bytes()),
    )?;
    parse_json(resp)
}

fn get_json(url: &str, token: &str, query: &[(&str, &str)]) -> Result<serde_json::Value, String> {
    let full = if query.is_empty() {
        url.to_string()
    } else {
        format!("{url}?{}", serde_urlencoded::to_string(query).map_err(|e| e.to_string())?)
    };
    let bearer = format!("Bearer {token}");
    parse_json(http::request("GET", &full, &[("Authorization", &bearer)], None)?)
}

// -- API Google Calendar ------------------------------------------------------

pub fn list_calendars(state: &GcalState) -> Result<Vec<GcalCalendar>, String> {
    let token = state.access_token()?;
    let resp = get_json(&format!("{CALENDAR_API}/users/me/calendarList"), &token, &[])?;
    if let Some(err) = resp["error"]["message"].as_str() {
        return Err(format!("Google ha risposto: {err}"));
    }
    let items = resp["items"].as_array().ok_or("risposta inattesa")?;
    Ok(items
        .iter()
        .map(|item| GcalCalendar {
            id: item["id"].as_str().unwrap_or("").to_string(),
            summary: item["summary"].as_str().unwrap_or("").to_string(),
            color: item["backgroundColor"]
                .as_str()
                .unwrap_or("#b5d3f7")
                .to_string(),
        })
        .filter(|c| !c.id.is_empty())
        .collect())
}

// -- Sync --------------------------------------------------------------------

pub fn sync(app: &AppHandle) -> Result<usize, String> {
    let state = app.state::<GcalState>();
    let db = app.state::<Db>();
    let settings = app.state::<SettingsState>().get();

    if !settings.gcal_enabled || settings.gcal_calendar_ids.is_empty() {
        return Ok(0);
    }

    let folder_id = db.gcal_folder_id().map_err(|e| e.to_string())?;
    let token = state.access_token()?;

    let now_dt: DateTime<Utc> = Utc::now();
    let weeks = settings.gcal_sync_weeks.clamp(1, 2) as i64;
    let time_min = now_dt.to_rfc3339();
    let time_max = (now_dt + TimeDelta::weeks(weeks)).to_rfc3339();

    let mut total = 0usize;
    let mut all_event_ids: Vec<String> = Vec::new();

    for cal_id in &settings.gcal_calendar_ids {
        let url = format!("{CALENDAR_API}/calendars/{}/events", urlencoded(cal_id));
        let resp = get_json(
            &url,
            &token,
            &[
                ("singleEvents", "true"),
                ("orderBy", "startTime"),
                ("timeMin", &time_min),
                ("timeMax", &time_max),
                ("maxResults", "250"),
            ],
        )
        .map_err(|e| format!("errore Calendar per {cal_id}: {e}"))?;

        if let Some(err) = resp["error"]["message"].as_str() {
            return Err(format!("Google ha risposto per {cal_id}: {err}"));
        }
        let items = match resp["items"].as_array() {
            Some(a) => a,
            None => continue,
        };

        for item in items {
            if item["status"].as_str().unwrap_or("confirmed") == "cancelled" {
                continue;
            }
            let event_id = match item["id"].as_str() {
                Some(id) => id,
                None => continue,
            };
            let gcal_key = format!("{cal_id}_{event_id}");
            let title = item["summary"].as_str().unwrap_or("(senza titolo)");
            let body = build_body(item);
            let start = parse_start(item).unwrap_or_else(|| Utc::now().timestamp());
            let color = map_gcal_color(item["colorId"].as_str().unwrap_or(""));

            db.upsert_gcal_note(&gcal_key, title, &body, &color, start, folder_id)
                .map_err(|e| format!("upsert fallito per {gcal_key}: {e}"))?;
            all_event_ids.push(gcal_key);
            total += 1;
        }
    }

    db.trash_stale_gcal_notes(&all_event_ids, &settings.gcal_calendar_ids)
        .map_err(|e| e.to_string())?;

    *state.last_sync.lock().unwrap() = Some(crate::db::now());
    *state.last_error.lock().unwrap() = None;
    notify_changed_from(app, "gcal");
    Ok(total)
}

// -- Syncer -------------------------------------------------------------------

pub struct Syncer(#[allow(dead_code)] Sender<()>);

#[allow(dead_code)]
impl Syncer {
    pub fn wake(&self) {
        let _ = self.0.send(());
    }
}

pub fn start_syncer(app: &AppHandle) -> Syncer {
    let (tx, rx) = channel::<()>();
    let handle = app.clone();
    thread::spawn(move || {
        let mut last_sync_at = 0u64;
        loop {
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now_secs.saturating_sub(last_sync_at) >= SYNC_INTERVAL_SECS {
                let state = handle.state::<GcalState>();
                if state.is_connected() {
                    match sync(&handle) {
                        Ok(n) => eprintln!("gcal sync: {n} eventi importati"),
                        Err(e) => {
                            eprintln!("gcal sync errore: {e}");
                            *state.last_error.lock().unwrap() = Some(e);
                        }
                    }
                }
                last_sync_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
            }
            match rx.recv_timeout(Duration::from_secs(MAX_WAIT_SECS)) {
                Ok(_) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
    Syncer(tx)
}

// -- Helpers ------------------------------------------------------------------

fn build_body(item: &serde_json::Value) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(when) = format_when(item) {
        parts.push(format!("🕘 {when}"));
    }
    if let Some(loc) = item["location"].as_str().filter(|s| !s.is_empty()) {
        parts.push(format!("📍 {loc}"));
    }
    if let Some(link) = item["hangoutLink"].as_str() {
        parts.push(format!("🔗 [Apri Meet]({link})"));
    }
    if let Some(desc) = item["description"].as_str().filter(|s| !s.is_empty()) {
        let clean = desc
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("</p>", "\n")
            .replace("</div>", "\n");
        let no_tags: String = clean
            .chars()
            .scan(false, |in_tag, c| {
                if c == '<' {
                    *in_tag = true;
                }
                let emit = !*in_tag;
                if c == '>' {
                    *in_tag = false;
                }
                Some(if emit { c } else { '\0' })
            })
            .filter(|&c| c != '\0')
            .collect();
        let trimmed = no_tags.trim().to_string();
        if !trimmed.is_empty() {
            parts.push(trimmed);
        }
    }
    parts.join("\n")
}

/// "02/10/2026 09:00–10:00", oppure "02/10/2026 · tutto il giorno".
fn format_when(item: &serde_json::Value) -> Option<String> {
    let local = |key: &str| {
        DateTime::parse_from_rfc3339(item[key]["dateTime"].as_str()?)
            .ok()
            .map(|d| d.with_timezone(&Local))
    };
    if let Some(start) = local("start") {
        let end = local("end");
        let range = match end {
            Some(e) if e.date_naive() == start.date_naive() => format!("{}–{}", start.format("%H:%M"), e.format("%H:%M")),
            Some(e) => format!("{} → {}", start.format("%H:%M"), e.format("%d/%m %H:%M")),
            None => start.format("%H:%M").to_string(),
        };
        return Some(format!("{} {range}", start.format("%d/%m/%Y")));
    }
    let date: chrono::NaiveDate = item["start"]["date"].as_str()?.parse().ok()?;
    Some(format!("{} · tutto il giorno", date.format("%d/%m/%Y")))
}

fn parse_start(item: &serde_json::Value) -> Option<i64> {
    let start = &item["start"];
    if let Some(dt_str) = start["dateTime"].as_str() {
        return DateTime::parse_from_rfc3339(dt_str)
            .ok()
            .map(|d| d.timestamp());
    }
    if let Some(date_str) = start["date"].as_str() {
        use chrono::NaiveDate;
        if let Ok(date) = date_str.parse::<NaiveDate>() {
            let naive_dt = date.and_hms_opt(9, 0, 0)?;
            return Local
                .from_local_datetime(&naive_dt)
                .earliest()
                .map(|d| d.timestamp());
        }
    }
    None
}

fn map_gcal_color(color_id: &str) -> String {
    let gcal_palette: &[(&str, [u8; 3])] = &[
        ("1",  [121, 134, 203]),
        ("2",  [97,  97,  97 ]),
        ("3",  [51,  182, 121]),
        ("4",  [231, 145, 103]),
        ("5",  [245, 116, 100]),
        ("6",  [25,  174, 212]),
        ("7",  [66,  133, 244]),
        ("8",  [240, 230, 130]),
        ("9",  [30,  154,  88]),
        ("10", [240, 134, 180]),
        ("11", [230, 124, 115]),
    ];
    let tabby_palette: &[([u8; 3], &str)] = &[
        ([181, 211, 247], "#b5d3f7"),
        ([179, 229, 207], "#b3e5cf"),
        ([212, 200, 242], "#d4c8f2"),
        ([247, 220, 130], "#f7dc82"),
        ([247, 196, 181], "#f7c4b5"),
        ([242, 184, 212], "#f2b8d4"),
    ];
    let source = gcal_palette
        .iter()
        .find(|(id, _)| *id == color_id)
        .map(|(_, rgb)| *rgb)
        .unwrap_or([181, 211, 247]);
    tabby_palette
        .iter()
        .min_by_key(|(t, _)| {
            let dr = source[0] as i32 - t[0] as i32;
            let dg = source[1] as i32 - t[1] as i32;
            let db = source[2] as i32 - t[2] as i32;
            dr * dr + dg * dg + db * db
        })
        .map(|(_, hex)| hex.to_string())
        .unwrap_or_else(|| "#b5d3f7".to_string())
}

fn urlencoded(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
