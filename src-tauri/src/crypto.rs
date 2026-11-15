//! Cifratura della cartella condivisa con una frase segreta (vedi `sync.rs`).
//!
//! Dalla frase si ricava una chiave AES-256 con PBKDF2-SHA256 e un sale casuale salvato
//! nella cartella condivisa; ogni file è cifrato con AES-256-GCM e un nonce casuale.
//! Senza la frase, chi apre la cartella (anche Google) vede solo dati illeggibili.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::RngCore;

/// I file cifrati iniziano così; quelli in chiaro (JSON) no.
const MAGIC: &[u8] = b"TABBYENC1";
const NONCE_LEN: usize = 12;
const ITERATIONS: u32 = 210_000;
/// Testo noto cifrato nella cartella: serve a capire se la frase inserita è quella giusta.
const CHECK_TEXT: &[u8] = b"tabby-ok";
/// Lunghezza minima della frase segreta.
pub const MIN_PASSPHRASE: usize = 8;

pub type Key = [u8; 32];

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    rand::rng().fill_bytes(&mut out);
    out
}

pub fn derive_key(passphrase: &str, salt: &[u8]) -> Key {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(passphrase.trim().as_bytes(), salt, ITERATIONS, &mut key);
    key
}

pub fn is_encrypted(data: &[u8]) -> bool {
    data.starts_with(MAGIC)
}

pub fn encrypt(key: &Key, plain: &[u8]) -> Vec<u8> {
    let cipher = Aes256Gcm::new(key.into());
    let nonce: [u8; NONCE_LEN] = random_bytes();
    let sealed = cipher
        .encrypt(Nonce::from_slice(&nonce), plain)
        .expect("la cifratura AES-GCM in memoria non fallisce");
    [MAGIC, &nonce, &sealed].concat()
}

pub fn decrypt(key: &Key, data: &[u8]) -> Result<Vec<u8>, String> {
    let rest = data.strip_prefix(MAGIC).ok_or("file non cifrato")?;
    if rest.len() < NONCE_LEN {
        return Err("file cifrato incompleto".into());
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    Aes256Gcm::new(key.into())
        .decrypt(Nonce::from_slice(nonce), sealed)
        .map_err(|_| "frase segreta sbagliata o file rovinato".to_string())
}

/// Controllo da salvare nella cartella insieme al sale.
pub fn make_check(key: &Key) -> String {
    STANDARD.encode(encrypt(key, CHECK_TEXT))
}

pub fn key_matches(key: &Key, check: &str) -> bool {
    STANDARD
        .decode(check)
        .ok()
        .and_then(|data| decrypt(key, &data).ok())
        .is_some_and(|plain| plain == CHECK_TEXT)
}

pub fn encode(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

pub fn decode_key(text: &str) -> Option<Key> {
    STANDARD.decode(text).ok()?.try_into().ok()
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
    STANDARD.decode(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_wrong_key() {
        let salt: [u8; 16] = random_bytes();
        let key = derive_key("la mia frase segreta", &salt);
        let data = encrypt(&key, b"lista della spesa");
        assert!(is_encrypted(&data));
        assert_eq!(decrypt(&key, &data).unwrap(), b"lista della spesa");
        let wrong = derive_key("un'altra frase", &salt);
        assert!(decrypt(&wrong, &data).is_err());
    }

    #[test]
    fn the_check_recognises_the_right_passphrase() {
        let salt: [u8; 16] = random_bytes();
        let key = derive_key("frase giusta", &salt);
        let check = make_check(&key);
        assert!(key_matches(&derive_key(" frase giusta ", &salt), &check));
        assert!(!key_matches(&derive_key("frase sbagliata", &salt), &check));
    }
}
