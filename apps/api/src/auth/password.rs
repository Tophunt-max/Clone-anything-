//! Argon2id password hashing (default parameters: 19 MiB, 2 passes, 1 lane). CPU-heavy: call
//! these from `spawn_blocking`.

use std::sync::OnceLock;

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};

pub const MIN_LEN: usize = 12;
pub const MAX_LEN: usize = 128;

/// `None` if the password is acceptable, otherwise why not.
pub fn policy_problem(password: &str) -> Option<String> {
    let n = password.chars().count();
    if n < MIN_LEN {
        return Some(format!("Use at least {MIN_LEN} characters."));
    }
    if n > MAX_LEN {
        return Some(format!("Use at most {MAX_LEN} characters."));
    }
    if password.trim().is_empty() {
        return Some("The password can't be only spaces.".into());
    }
    None
}

pub fn hash(password: &str) -> Result<String, String> {
    Argon2::default().hash_password(password.as_bytes()).map(|h| h.to_string()).map_err(|e| e.to_string())
}

pub fn verify(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// A real hash of a random password, checked when an email isn't registered, so a login for an
/// unknown address takes as long as one for a known address.
pub fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| hash(&uuid::Uuid::now_v7().to_string()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_verify_and_differ_per_salt() {
        let a = hash("correct horse battery").unwrap();
        let b = hash("correct horse battery").unwrap();
        assert!(a.starts_with("$argon2id$"), "{a}");
        assert_ne!(a, b);
        assert!(verify("correct horse battery", &a));
        assert!(!verify("correct horse batterx", &a));
        assert!(!verify("anything", "not a hash"));
        assert!(!dummy_hash().is_empty());
    }

    #[test]
    fn policy() {
        assert!(policy_problem("short").is_some());
        assert!(policy_problem(&"x".repeat(129)).is_some());
        assert!(policy_problem("            ").is_some());
        assert!(policy_problem("a long enough passphrase").is_none());
    }
}
