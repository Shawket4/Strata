//! Argon2id password hashing (PLAN §8) with parameters from `auth.argon2`.
//!
//! `users.password_hash` holds a PHC string (`$argon2id$v=19$m=…`). A password set by an admin
//! reset is flagged with `users.must_change_password`: it verifies like any other, but the
//! account is restricted until the user sets a new password (`PATCH /me`). Verification uses
//! the parameters recorded in the PHC string, so raising the configured cost only affects new
//! hashes; [`PasswordHasher::needs_rehash`] tells login to upgrade an old hash.

use std::fmt;

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher as _, PasswordVerifier as _};
use argon2::{Algorithm, Argon2, Params, Version};
use strata_common::config::Argon2Config;

/// Longest accepted password in bytes (bounds hashing work per request).
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Hashing failed (bad parameters or RNG failure). Never contains the password.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("password hashing failed: {0}")]
pub struct HashError(String);

/// Hashes and verifies passwords with Argon2id.
#[derive(Clone)]
pub struct PasswordHasher {
    params: Params,
    /// A hash of a random password, verified against when the username is unknown so that
    /// the response time does not reveal whether an account exists.
    dummy: String,
}

impl fmt::Debug for PasswordHasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordHasher")
            .field("m_cost", &self.params.m_cost())
            .field("t_cost", &self.params.t_cost())
            .field("p_cost", &self.params.p_cost())
            .finish_non_exhaustive()
    }
}

impl PasswordHasher {
    /// A hasher with the configured cost.
    pub fn new(config: &Argon2Config) -> Result<Self, HashError> {
        let params = Params::new(
            config.memory_kib,
            config.iterations,
            config.parallelism,
            None,
        )
        .map_err(|e| HashError(e.to_string()))?;
        let mut hasher = Self {
            params,
            dummy: String::new(),
        };
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| HashError(e.to_string()))?;
        hasher.dummy = hasher.hash(&base64_of(&random))?;
        Ok(hasher)
    }

    fn argon2(&self) -> Argon2<'static> {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, self.params.clone())
    }

    /// A PHC string for `password` with a fresh random salt.
    pub fn hash(&self, password: &str) -> Result<String, HashError> {
        self.argon2()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
            .map_err(|e| HashError(e.to_string()))
    }

    /// Whether `password` matches the stored PHC string. Malformed stored values never match.
    pub fn verify(&self, password: &str, stored: &str) -> bool {
        if password.len() > MAX_PASSWORD_BYTES {
            return false;
        }
        match PasswordHash::new(stored) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    }

    /// Spends the same work as [`Self::verify`] against a throwaway hash (unknown username).
    pub fn verify_dummy(&self, password: &str) {
        let _ = self.verify(password, &self.dummy);
    }

    /// Whether a stored hash uses different parameters than configured (rehash on login).
    pub fn needs_rehash(&self, stored: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(stored) else {
            return true;
        };
        let Ok(params) = Params::try_from(&parsed) else {
            return true;
        };
        parsed.algorithm.as_str() != "argon2id"
            || params.m_cost() != self.params.m_cost()
            || params.t_cost() != self.params.t_cost()
            || params.p_cost() != self.params.p_cost()
    }
}

fn base64_of(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD_NO_PAD.encode(bytes)
}

/// A random temporary password (admin reset): 16 characters from an alphabet without
/// look-alikes (no `0/O`, `1/l/I`), about 90 bits.
pub fn temporary_password() -> Result<String, HashError> {
    const ALPHABET: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut out = String::with_capacity(16);
    while out.len() < 16 {
        let mut byte = [0u8; 1];
        getrandom::fill(&mut byte).map_err(|e| HashError(e.to_string()))?;
        // Rejection sampling keeps the distribution uniform (57 symbols; 4 × 57 = 228).
        if usize::from(byte[0]) < ALPHABET.len() * 4 {
            out.push(char::from(ALPHABET[usize::from(byte[0]) % ALPHABET.len()]));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cheap() -> Argon2Config {
        Argon2Config {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
        }
    }

    #[test]
    fn hashes_are_argon2id_phc_strings_that_verify() {
        let hasher = PasswordHasher::new(&cheap()).expect("params");
        let hash = hasher.hash("correct horse").expect("hash");
        assert!(hash.starts_with("$argon2id$v=19$m=64,t=1,p=1$"), "{hash}");
        assert!(hasher.verify("correct horse", &hash));
        assert!(!hasher.verify("correct horsE", &hash));
        assert_ne!(hasher.hash("correct horse").expect("hash"), hash);
    }

    #[test]
    fn malformed_or_oversized_inputs_never_match() {
        let hasher = PasswordHasher::new(&cheap()).expect("params");
        assert!(!hasher.verify("x", "not a phc string"));
        assert!(!hasher.verify("x", ""));
        // The retired `temporary:` prefix (migration 010 strips it) is not a PHC string.
        assert!(!hasher.verify(
            "x",
            &format!("temporary:{}", hasher.hash("x").expect("hash"))
        ));
        let hash = hasher.hash("x").expect("hash");
        assert!(!hasher.verify(&"x".repeat(MAX_PASSWORD_BYTES + 1), &hash));
        hasher.verify_dummy("anything");
    }

    #[test]
    fn rehash_is_needed_when_parameters_change() {
        let old = PasswordHasher::new(&cheap()).expect("params");
        let new = PasswordHasher::new(&Argon2Config {
            memory_kib: 128,
            iterations: 1,
            parallelism: 1,
        })
        .expect("params");
        let hash = old.hash("pw").expect("hash");
        assert!(!old.needs_rehash(&hash));
        assert!(new.needs_rehash(&hash));
        assert!(new.needs_rehash("garbage"));
    }

    #[test]
    fn invalid_parameters_are_rejected() {
        let err = PasswordHasher::new(&Argon2Config {
            memory_kib: 1,
            iterations: 1,
            parallelism: 1,
        })
        .expect_err("too little memory");
        assert!(err.to_string().starts_with("password hashing failed"));
    }

    #[test]
    fn temporary_passwords_use_the_unambiguous_alphabet() {
        let a = temporary_password().expect("rng");
        let b = temporary_password().expect("rng");
        assert_eq!(a.chars().count(), 16);
        assert_ne!(a, b);
        assert!(
            a.chars()
                .all(|c| c.is_ascii_alphanumeric() && !matches!(c, '0' | 'O' | '1' | 'l' | 'I'))
        );
    }
}
