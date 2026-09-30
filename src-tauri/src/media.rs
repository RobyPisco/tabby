//! Immagini incollate nelle note, salvate in `%APPDATA%\it.pisco.tabby\media`
//! e referenziate dal Markdown come `![](media/<nome>)`.

use std::{collections::HashSet, path::PathBuf};
use tauri::{
    ipc::{InvokeBody, Request},
    AppHandle, Manager,
};
use tauri_plugin_opener::OpenerExt;

use crate::db::Db;

const ALLOWED_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];
/// Limite di sicurezza per un singolo file incollato.
const MAX_BYTES: usize = 20 * 1024 * 1024;
/// Gli allegati (PDF, documenti…) possono essere più grandi delle immagini.
const MAX_ATTACHMENT_BYTES: usize = 100 * 1024 * 1024;

pub fn media_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = crate::data_dir(app).map_err(|e| e.to_string())?.join("media");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Hash FNV-1a a 64 bit: la stessa immagine incollata due volte diventa un solo file.
fn content_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[tauri::command]
pub fn media_path(app: AppHandle) -> Result<String, String> {
    Ok(media_dir(&app)?.to_string_lossy().into_owned())
}

/// Riceve i byte dell'immagine come corpo grezzo della richiesta (niente JSON)
/// e l'estensione nell'header `x-image-ext`; restituisce il nome del file salvato.
#[tauri::command]
pub fn save_image(app: AppHandle, request: Request<'_>) -> Result<String, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("atteso il contenuto grezzo dell'immagine".into());
    };
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("immagine vuota o più grande di 20 MB".into());
    }
    let ext = request
        .headers()
        .get("x-image-ext")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("png")
        .to_ascii_lowercase();
    if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
        return Err(format!("formato immagine non supportato: {ext}"));
    }
    let name = format!("{:016x}.{ext}", content_hash(bytes));
    let path = media_dir(&app)?.join(&name);
    if !path.exists() {
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(name)
}

/// Parte "sicura" del nome originale di un allegato: lettere, cifre, punto, trattino.
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || ".-_".contains(c) { c } else { '_' })
        .collect();
    let cleaned = cleaned.trim_matches(['.', '_']).chars().take(80).collect::<String>();
    if cleaned.is_empty() {
        "allegato".into()
    } else {
        cleaned
    }
}

/// Allegato trascinato o incollato (qualsiasi tipo di file): salvato come
/// `<hash>-<nome originale>`, nome restituito al frontend.
#[tauri::command]
pub fn save_attachment(app: AppHandle, request: Request<'_>) -> Result<String, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("atteso il contenuto grezzo del file".into());
    };
    if bytes.len() > MAX_ATTACHMENT_BYTES {
        return Err("file più grande di 100 MB".into());
    }
    let original = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(|v| percent_decode(v))
        .unwrap_or_else(|| "allegato".into());
    let name = format!("{:016x}-{}", content_hash(bytes), safe_file_name(&original));
    let path = media_dir(&app)?.join(&name);
    if !path.exists() {
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(name)
}

/// Gli header HTTP accettano solo ASCII: il nome arriva codificato con encodeURIComponent.
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Apre un file della cartella media con il programma predefinito di Windows.
#[tauri::command]
pub fn open_media(app: AppHandle, name: String) -> Result<(), String> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return Err("nome di file non valido".into());
    }
    let path = media_dir(&app)?.join(&name);
    if !path.exists() {
        return Err(format!("il file {name} non esiste più"));
    }
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Copia un file della cartella media in un percorso scelto dall'utente ("Salva con nome").
#[tauri::command]
pub fn save_media_as(app: AppHandle, name: String, dest: String) -> Result<(), String> {
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return Err("nome di file non valido".into());
    }
    let path = media_dir(&app)?.join(&name);
    std::fs::copy(path, dest).map(|_| ()).map_err(|e| e.to_string())
}

/// All'avvio elimina dalla cartella media i file creati dall'app che nessuna nota
/// (né il cestino, né la cronologia) cita più.
pub fn remove_unused(app: &AppHandle) -> Result<usize, String> {
    let referenced: HashSet<String> = app
        .state::<Db>()
        .all_texts()
        .map_err(|e| e.to_string())?
        .iter()
        .flat_map(|text| crate::export::referenced_media(text))
        .collect();
    let mut removed = 0;
    for entry in std::fs::read_dir(media_dir(app)?).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Solo i file con il nome generato dall'app (16 cifre esadecimali in testa).
        let generated = name.len() > 16 && name[..16].chars().all(|c| c.is_ascii_hexdigit());
        if generated && !referenced.contains(&name) && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::{content_hash, percent_decode, safe_file_name};

    #[test]
    fn attachment_names() {
        assert_eq!(safe_file_name("Fattura luglio.pdf"), "Fattura_luglio.pdf");
        assert_eq!(safe_file_name("../../x"), "x");
        assert_eq!(percent_decode("caff%C3%A8.txt"), "caffè.txt");
    }

    #[test]
    fn same_bytes_same_name() {
        assert_eq!(content_hash(b"abc"), content_hash(b"abc"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abd"));
    }
}
