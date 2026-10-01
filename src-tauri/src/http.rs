//! Client HTTP minimale sopra WinHTTP, lo stesso stack di browser e aggiornamenti di Windows.
//!
//! Rispetto a un client generico gestisce da solo ciò che serve dietro un proxy aziendale:
//! file PAC / rilevamento automatico, autenticazione integrata (NTLM/Negotiate con l'utente
//! collegato) e certificati dell'archivio di Windows.

use std::{
    ffi::c_void,
    iter::once,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};
use windows_sys::Win32::{
    Foundation::{GetLastError, GlobalFree},
    Networking::WinHttp::*,
};

/// Se falso (impostazione dell'utente) si va sempre in diretta: niente proxy né credenziali.
static USE_PROXY: AtomicBool = AtomicBool::new(false);

pub fn set_use_proxy(on: bool) {
    USE_PROXY.store(on, Ordering::Relaxed);
}

pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// Handle WinHTTP chiuso automaticamente.
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void, what: &str) -> Result<Self, String> {
        if raw.is_null() {
            Err(last_error(what))
        } else {
            Ok(Handle(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

fn last_error(what: &str) -> String {
    let hint = if USE_PROXY.load(Ordering::Relaxed) {
        ""
    } else {
        " (se sei dietro un proxy aziendale, attiva «Usa il proxy di Windows» nelle impostazioni)"
    };
    format!("{what}: errore di rete WinHTTP {}{hint}", unsafe { GetLastError() })
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(once(0)).collect()
}

/// Copia una stringa UTF-16 terminata da zero e libera l'originale (allocata da WinHTTP).
unsafe fn take_wide(p: *mut u16) -> Option<Vec<u16>> {
    if p.is_null() {
        return None;
    }
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    let copy = std::slice::from_raw_parts(p, len + 1).to_vec();
    GlobalFree(p as *mut c_void);
    Some(copy)
}

enum Route {
    /// Lascia decidere a WinHTTP (impostazioni di sistema).
    Default,
    Direct,
    Proxy { proxy: Vec<u16>, bypass: Option<Vec<u16>> },
}

/// Risolve il proxy per `url` come fa il browser: PAC o rilevamento automatico, poi
/// proxy manuale delle impostazioni utente.
fn resolve_route(session: *mut c_void, url: &[u16]) -> Route {
    unsafe {
        let mut ie = WINHTTP_CURRENT_USER_IE_PROXY_CONFIG::default();
        if WinHttpGetIEProxyConfigForCurrentUser(&mut ie) == 0 {
            return Route::Default;
        }
        let auto_detect = ie.fAutoDetect != 0;
        let pac = take_wide(ie.lpszAutoConfigUrl);
        let manual = take_wide(ie.lpszProxy);
        let manual_bypass = take_wide(ie.lpszProxyBypass);

        if auto_detect || pac.is_some() {
            let mut opts = WINHTTP_AUTOPROXY_OPTIONS::default();
            if auto_detect {
                opts.dwFlags |= WINHTTP_AUTOPROXY_AUTO_DETECT;
                opts.dwAutoDetectFlags = WINHTTP_AUTO_DETECT_TYPE_DHCP | WINHTTP_AUTO_DETECT_TYPE_DNS_A;
            }
            if let Some(p) = &pac {
                opts.dwFlags |= WINHTTP_AUTOPROXY_CONFIG_URL;
                opts.lpszAutoConfigUrl = p.as_ptr();
            }
            opts.fAutoLogonIfChallenged = 1;
            let mut info = WINHTTP_PROXY_INFO::default();
            if WinHttpGetProxyForUrl(session, url.as_ptr(), &mut opts, &mut info) != 0 {
                let proxy = take_wide(info.lpszProxy);
                let bypass = take_wide(info.lpszProxyBypass);
                return match proxy {
                    Some(proxy) if info.dwAccessType == WINHTTP_ACCESS_TYPE_NAMED_PROXY => {
                        Route::Proxy { proxy, bypass }
                    }
                    _ => Route::Direct,
                };
            }
        }
        match manual {
            Some(proxy) => Route::Proxy { proxy, bypass: manual_bypass },
            None => Route::Default,
        }
    }
}

/// `https://host[:porta]/percorso?query` -> (https, host, porta, percorso con query).
fn split_url(url: &str) -> Result<(bool, String, u16, String), String> {
    let (secure, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return Err(format!("URL non valido: {url}"));
    };
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().map_err(|_| format!("porta non valida: {url}"))?),
        None => (hostport, if secure { 443 } else { 80 }),
    };
    Ok((secure, host.to_string(), port, path.to_string()))
}

pub fn request(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
) -> Result<Response, String> {
    let (secure, host, port, path) = split_url(url)?;
    let use_proxy = USE_PROXY.load(Ordering::Relaxed);
    unsafe {
        let session = Handle::new(
            WinHttpOpen(
                wide("Tabby").as_ptr(),
                if use_proxy { WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY } else { WINHTTP_ACCESS_TYPE_NO_PROXY },
                ptr::null(),
                ptr::null(),
                0,
            ),
            "WinHttpOpen",
        )?;
        WinHttpSetTimeouts(session.0, 10_000, 10_000, 30_000, 30_000);
        let connection = Handle::new(
            WinHttpConnect(session.0, wide(&host).as_ptr(), port, 0),
            "WinHttpConnect",
        )?;
        let req = Handle::new(
            WinHttpOpenRequest(
                connection.0,
                wide(method).as_ptr(),
                wide(&path).as_ptr(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                if secure { WINHTTP_FLAG_SECURE } else { 0 },
            ),
            "WinHttpOpenRequest",
        )?;

        let route = if use_proxy { resolve_route(session.0, &wide(url)) } else { Route::Default };
        match route {
            Route::Default => {}
            Route::Direct => {
                let info = WINHTTP_PROXY_INFO {
                    dwAccessType: WINHTTP_ACCESS_TYPE_NO_PROXY,
                    lpszProxy: ptr::null_mut(),
                    lpszProxyBypass: ptr::null_mut(),
                };
                WinHttpSetOption(req.0, WINHTTP_OPTION_PROXY, &info as *const _ as *const c_void, size_of::<WINHTTP_PROXY_INFO>() as u32);
            }
            Route::Proxy { mut proxy, bypass } => {
                let mut bypass = bypass;
                let info = WINHTTP_PROXY_INFO {
                    dwAccessType: WINHTTP_ACCESS_TYPE_NAMED_PROXY,
                    lpszProxy: proxy.as_mut_ptr(),
                    lpszProxyBypass: bypass.as_mut().map_or(ptr::null_mut(), |b| b.as_mut_ptr()),
                };
                WinHttpSetOption(req.0, WINHTTP_OPTION_PROXY, &info as *const _ as *const c_void, size_of::<WINHTTP_PROXY_INFO>() as u32);
            }
        }

        let header_text: String = headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect();
        let header_w = wide(&header_text);
        let (body_ptr, body_len) = match body {
            Some(b) => (b.as_ptr() as *const c_void, b.len() as u32),
            None => (ptr::null(), 0),
        };

        // Dopo un 407 si ripete l'invio con le credenziali di Windows dell'utente corrente.
        let mut status = 0u32;
        for _ in 0..4 {
            if WinHttpSendRequest(
                req.0,
                if headers.is_empty() { ptr::null() } else { header_w.as_ptr() },
                if headers.is_empty() { 0 } else { u32::MAX },
                body_ptr,
                body_len,
                body_len,
                0,
            ) == 0
            {
                return Err(last_error("WinHttpSendRequest"));
            }
            if WinHttpReceiveResponse(req.0, ptr::null_mut()) == 0 {
                return Err(last_error("WinHttpReceiveResponse"));
            }
            let mut size = size_of::<u32>() as u32;
            if WinHttpQueryHeaders(
                req.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                ptr::null(),
                &mut status as *mut u32 as *mut c_void,
                &mut size,
                ptr::null_mut(),
            ) == 0
            {
                return Err(last_error("WinHttpQueryHeaders"));
            }
            if !use_proxy || (status != 407 && status != 401) {
                break;
            }
            let (mut supported, mut first, mut target) = (0u32, 0u32, 0u32);
            if WinHttpQueryAuthSchemes(req.0, &mut supported, &mut first, &mut target) == 0 {
                break;
            }
            let scheme = if supported & WINHTTP_AUTH_SCHEME_NEGOTIATE != 0 {
                WINHTTP_AUTH_SCHEME_NEGOTIATE
            } else if supported & WINHTTP_AUTH_SCHEME_NTLM != 0 {
                WINHTTP_AUTH_SCHEME_NTLM
            } else {
                break;
            };
            if WinHttpSetCredentials(req.0, target, scheme, ptr::null(), ptr::null(), ptr::null_mut()) == 0 {
                break;
            }
        }

        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let mut read = 0u32;
            if WinHttpReadData(req.0, buf.as_mut_ptr() as *mut c_void, buf.len() as u32, &mut read) == 0 {
                return Err(last_error("WinHttpReadData"));
            }
            if read == 0 {
                break;
            }
            out.extend_from_slice(&buf[..read as usize]);
        }
        Ok(Response { status: status as u16, body: out })
    }
}
