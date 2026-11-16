//! Server settings from environment variables. Secrets come only from the environment, never
//! from files in the repository.

use std::net::SocketAddr;
use std::time::Duration;

use crate::auth::tokens::TokenKey;

#[derive(Debug, Clone)]
pub struct Config {
    /// `FORGE_API_ADDR`, default `127.0.0.1:8080`. Behind the reverse proxy in production.
    pub addr: SocketAddr,
    /// `FORGE_API_REQUEST_TIMEOUT_SECS`, default 30.
    pub request_timeout: Duration,
    /// `FORGE_API_BODY_LIMIT_BYTES`, default 1 MiB.
    pub body_limit: usize,
    /// `FORGE_DATABASE_URL`, required, e.g. `postgres://forge@127.0.0.1/forge`.
    pub database_url: String,
    /// `FORGE_DATABASE_MAX_CONNECTIONS`, default 10.
    pub database_max_connections: u32,
    /// `FORGE_TOKEN_KEY`, required: base64 of at least 32 random bytes. Signs access tokens and
    /// keys the hashes of IP addresses in the audit log.
    pub token_key: TokenKey,
    /// `FORGE_COOKIE_SECURE`, default `true`. Only turn off for plain-HTTP local development.
    pub cookie_secure: bool,
    /// `FORGE_TRUST_PROXY`, default `false`. When `true`, the client address is the last
    /// `X-Forwarded-For` entry (set by our reverse proxy); never enable without a proxy.
    pub trust_proxy: bool,
    /// `FORGE_MIGRATE_ON_START`, default `true`: apply pending migrations before serving.
    pub migrate_on_start: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// Testable form: `get` returns a variable's value.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let addr = match get("FORGE_API_ADDR") {
            Some(v) => v.parse().map_err(|_| format!("FORGE_API_ADDR is not an address like 127.0.0.1:8080: {v:?}"))?,
            None => SocketAddr::from(([127, 0, 0, 1], 8080)),
        };
        let secs: u64 = number(&get, "FORGE_API_REQUEST_TIMEOUT_SECS", 30)?;
        if !(1..=3600).contains(&secs) {
            return Err("FORGE_API_REQUEST_TIMEOUT_SECS must be between 1 and 3600".into());
        }
        let body_limit: usize = number(&get, "FORGE_API_BODY_LIMIT_BYTES", 1024 * 1024)?;
        let database_url = get("FORGE_DATABASE_URL").filter(|v| !v.trim().is_empty()).ok_or("FORGE_DATABASE_URL is required (postgres://user@host/db)")?;
        let database_max_connections: u32 = number(&get, "FORGE_DATABASE_MAX_CONNECTIONS", 10)?;
        if !(1..=500).contains(&database_max_connections) {
            return Err("FORGE_DATABASE_MAX_CONNECTIONS must be between 1 and 500".into());
        }
        let token_key = TokenKey::from_base64(
            &get("FORGE_TOKEN_KEY").ok_or("FORGE_TOKEN_KEY is required: base64 of at least 32 random bytes (e.g. `openssl rand -base64 32`)")?,
        )
        .map_err(|e| format!("FORGE_TOKEN_KEY: {e}"))?;
        Ok(Config {
            addr,
            request_timeout: Duration::from_secs(secs),
            body_limit,
            database_url,
            database_max_connections,
            token_key,
            cookie_secure: flag(&get, "FORGE_COOKIE_SECURE", true)?,
            trust_proxy: flag(&get, "FORGE_TRUST_PROXY", false)?,
            migrate_on_start: flag(&get, "FORGE_MIGRATE_ON_START", true)?,
        })
    }
}

fn number<T: std::str::FromStr>(get: &impl Fn(&str) -> Option<String>, key: &str, default: T) -> Result<T, String> {
    match get(key) {
        Some(v) => v.trim().parse().map_err(|_| format!("{key} is not a whole number: {v:?}")),
        None => Ok(default),
    }
}

fn flag(get: &impl Fn(&str) -> Option<String>, key: &str, default: bool) -> Result<bool, String> {
    match get(key).as_deref().map(str::trim) {
        None => Ok(default),
        Some("1" | "true" | "yes") => Ok(true),
        Some("0" | "false" | "no") => Ok(false),
        Some(v) => Err(format!("{key} must be true or false: {v:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

    fn with(extra: &[(&str, &str)]) -> Result<Config, String> {
        let base = [("FORGE_DATABASE_URL", "postgres://u@h/db"), ("FORGE_TOKEN_KEY", KEY)];
        Config::from_lookup(|k| extra.iter().chain(base.iter()).find(|(n, _)| *n == k).map(|(_, v)| v.to_string()))
    }

    #[test]
    fn defaults() {
        let c = with(&[]).unwrap();
        assert_eq!(c.addr.to_string(), "127.0.0.1:8080");
        assert_eq!(c.request_timeout, Duration::from_secs(30));
        assert_eq!(c.body_limit, 1024 * 1024);
        assert!(c.cookie_secure && !c.trust_proxy && c.migrate_on_start);
        assert!(!format!("{c:?}").contains(KEY), "the key never prints");
    }

    #[test]
    fn required_and_invalid_values_are_reported() {
        assert!(Config::from_lookup(|_| None).unwrap_err().contains("FORGE_DATABASE_URL"));
        assert!(Config::from_lookup(|k| (k == "FORGE_DATABASE_URL").then(|| "postgres://x".to_string())).unwrap_err().contains("FORGE_TOKEN_KEY"));
        assert!(with(&[("FORGE_TOKEN_KEY", "c2hvcnQ=")]).is_err(), "a short key");
        assert!(with(&[("FORGE_API_ADDR", "nope")]).is_err());
        assert!(with(&[("FORGE_API_REQUEST_TIMEOUT_SECS", "0")]).is_err());
        assert!(with(&[("FORGE_COOKIE_SECURE", "maybe")]).is_err());
        assert!(!with(&[("FORGE_COOKIE_SECURE", "false")]).unwrap().cookie_secure);
    }
}
