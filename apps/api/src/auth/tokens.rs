//! Access tokens, refresh tokens and keyed hashes.
//!
//! Access token: `fa1.<base64url(JSON claims)>.<base64url(HMAC-SHA256)>`, valid 10 minutes.
//! Only this API issues and checks them, so a symmetric key held by the API is enough (no
//! public-key JWT needed). Refresh token: `fr1.<base64url(32 random bytes)>`, stored as SHA-256.

use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD as B64};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

pub const ACCESS_TTL_SECS: i64 = 600;
pub const SESSION_TTL_DAYS: i64 = 30;
const ACCESS_PREFIX: &str = "fa1.";
const REFRESH_PREFIX: &str = "fr1.";
const MAX_TOKEN_LEN: usize = 1024;

#[derive(Clone)]
pub struct TokenKey(Vec<u8>);

impl TokenKey {
    pub fn new(bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 32 {
            return Err(format!("needs at least 32 bytes, got {}", bytes.len()));
        }
        Ok(TokenKey(bytes))
    }

    pub fn from_base64(s: &str) -> Result<Self, String> {
        let bytes = STANDARD.decode(s.trim()).or_else(|_| B64.decode(s.trim())).map_err(|_| "is not valid base64".to_string())?;
        Self::new(bytes)
    }

    fn mac(&self) -> Option<HmacSha256> {
        HmacSha256::new_from_slice(&self.0).ok()
    }
}

impl std::fmt::Debug for TokenKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TokenKey(redacted)")
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub struct Claims {
    /// User ID.
    pub sub: Uuid,
    /// Session ID.
    pub sid: Uuid,
    /// Expiry, Unix seconds.
    pub exp: i64,
}

pub fn issue_access(key: &TokenKey, claims: &Claims) -> Result<String, String> {
    let json = serde_json::to_vec(claims).map_err(|e| e.to_string())?;
    let body = format!("{ACCESS_PREFIX}{}", B64.encode(json));
    let mut mac = key.mac().ok_or("bad key")?;
    mac.update(body.as_bytes());
    Ok(format!("{body}.{}", B64.encode(mac.finalize().into_bytes())))
}

/// The claims of a well-formed, correctly signed, unexpired token; `None` for anything else.
pub fn verify_access(key: &TokenKey, token: &str, now_unix: i64) -> Option<Claims> {
    if token.len() > MAX_TOKEN_LEN {
        return None;
    }
    let (body, sig) = token.rsplit_once('.')?;
    let payload = body.strip_prefix(ACCESS_PREFIX)?;
    let sig = B64.decode(sig).ok()?;
    let mut mac = key.mac()?;
    mac.update(body.as_bytes());
    mac.verify_slice(&sig).ok()?;
    let claims: Claims = serde_json::from_slice(&B64.decode(payload).ok()?).ok()?;
    (claims.exp > now_unix).then_some(claims)
}

fn random_bytes() -> Result<[u8; 32], String> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| format!("system random source failed: {e}"))?;
    Ok(b)
}

/// A new refresh token and the hash to store.
pub fn new_refresh() -> Result<(String, Vec<u8>), String> {
    let token = format!("{REFRESH_PREFIX}{}", B64.encode(random_bytes()?));
    let hash = hash_refresh(&token);
    Ok((token, hash))
}

pub fn hash_refresh(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn looks_like_refresh(token: &str) -> bool {
    token.len() <= MAX_TOKEN_LEN && token.starts_with(REFRESH_PREFIX)
}

/// A random value for the CSRF cookie.
pub fn new_csrf() -> Result<String, String> {
    Ok(B64.encode(random_bytes()?))
}

/// Keyed hash for values that must be comparable but not readable (client IPs in the audit log).
pub fn keyed_hash(key: &TokenKey, value: &str) -> String {
    match key.mac() {
        Some(mut mac) => {
            mac.update(b"ip:");
            mac.update(value.as_bytes());
            hex_prefix(&mac.finalize().into_bytes())
        }
        None => String::new(),
    }
}

fn hex_prefix(bytes: &[u8]) -> String {
    bytes.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> TokenKey {
        TokenKey::new(vec![7; 32]).unwrap()
    }

    fn claims(exp: i64) -> Claims {
        Claims { sub: Uuid::now_v7(), sid: Uuid::now_v7(), exp }
    }

    #[test]
    fn a_token_round_trips_until_it_expires() {
        let c = claims(1_000);
        let t = issue_access(&key(), &c).unwrap();
        assert_eq!(verify_access(&key(), &t, 999), Some(c));
        assert_eq!(verify_access(&key(), &t, 1_000), None);
    }

    #[test]
    fn tampering_or_another_key_is_refused() {
        let t = issue_access(&key(), &claims(i64::MAX)).unwrap();
        let other = TokenKey::new(vec![8; 32]).unwrap();
        assert_eq!(verify_access(&other, &t, 0), None);
        let (body, sig) = t.rsplit_once('.').unwrap();
        let forged = format!("{}.{}", body.replace("fa1.", "fa1.x"), sig);
        assert_eq!(verify_access(&key(), &forged, 0), None);
        assert_eq!(verify_access(&key(), "fa1.e30.", 0), None);
        assert_eq!(verify_access(&key(), "garbage", 0), None);
        assert_eq!(verify_access(&key(), &"a".repeat(5000), 0), None);
    }

    #[test]
    fn keys_must_be_long_enough_and_never_print() {
        assert!(TokenKey::new(vec![1; 31]).is_err());
        assert!(TokenKey::from_base64("not base64!").is_err());
        assert!(TokenKey::from_base64(&STANDARD.encode([3u8; 32])).is_ok());
        assert_eq!(format!("{:?}", key()), "TokenKey(redacted)");
    }

    #[test]
    fn refresh_tokens_are_random_and_hashed() {
        let (a, ha) = new_refresh().unwrap();
        let (b, _) = new_refresh().unwrap();
        assert_ne!(a, b);
        assert!(looks_like_refresh(&a));
        assert_eq!(ha, hash_refresh(&a));
        assert_eq!(ha.len(), 32);
        assert_eq!(keyed_hash(&key(), "1.2.3.4"), keyed_hash(&key(), "1.2.3.4"));
        assert_ne!(keyed_hash(&key(), "1.2.3.4"), keyed_hash(&key(), "1.2.3.5"));
    }
}
