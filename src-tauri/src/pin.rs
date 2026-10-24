//! PIN globale dell'app: protegge la lettura/modifica delle note protette (vedi GLOSSARY.md).
//! Nessuna cifratura reale (ADR-0001): l'hash impedisce solo di leggere il PIN da
//! settings.json, non protegge il corpo delle note, che resta in chiaro nel database.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Testo al posto di titolo di una nota protetta non ancora sbloccata in questa apertura.
pub const PLACEHOLDER: &str = "🔒 Nota protetta";

/// Vero se `pin` è una sequenza di 4-6 cifre.
pub fn is_valid(pin: &str) -> bool {
    (4..=6).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit())
}

/// Hash del PIN con un salt casuale; `hash`/`salt` tornano entrambi codificati base64.
pub fn hash(pin: &str) -> (String, String) {
    let mut raw_salt = [0u8; 16];
    rand::rng().fill_bytes(&mut raw_salt);
    let salt = URL_SAFE_NO_PAD.encode(raw_salt);
    (digest(pin, &salt), salt)
}

/// Vero se `pin` corrisponde all'hash salvato con quel salt.
pub fn verify(pin: &str, salt: &str, expected_hash: &str) -> bool {
    digest(pin, salt) == expected_hash
}

fn digest(pin: &str, salt: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(format!("{salt}:{pin}").as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::{hash, is_valid, verify};

    #[test]
    fn pin_length_and_digits_are_validated() {
        assert!(is_valid("1234"));
        assert!(is_valid("123456"));
        assert!(!is_valid("123"));
        assert!(!is_valid("1234567"));
        assert!(!is_valid("12a4"));
        assert!(!is_valid(""));
    }

    #[test]
    fn verify_accepts_the_pin_that_produced_the_hash() {
        let (digest, salt) = hash("1234");
        assert!(verify("1234", &salt, &digest));
    }

    #[test]
    fn verify_rejects_a_different_pin() {
        let (digest, salt) = hash("1234");
        assert!(!verify("4321", &salt, &digest));
    }

    #[test]
    fn hashing_the_same_pin_twice_uses_different_salts() {
        let (digest1, salt1) = hash("1234");
        let (digest2, salt2) = hash("1234");
        assert_ne!(salt1, salt2);
        assert_ne!(digest1, digest2);
    }
}
