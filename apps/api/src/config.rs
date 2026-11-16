//! Server settings from environment variables. Secrets will also come from the environment
//! (never from files in the repository).

use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    /// `FORGE_API_ADDR`, default `127.0.0.1:8080`. Behind the reverse proxy in production.
    pub addr: SocketAddr,
    /// `FORGE_API_REQUEST_TIMEOUT_SECS`, default 30.
    pub request_timeout: Duration,
    /// `FORGE_API_BODY_LIMIT_BYTES`, default 1 MiB.
    pub body_limit: usize,
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
        Ok(Config { addr, request_timeout: Duration::from_secs(secs), body_limit })
    }
}

fn number<T: std::str::FromStr>(get: &impl Fn(&str) -> Option<String>, key: &str, default: T) -> Result<T, String> {
    match get(key) {
        Some(v) => v.trim().parse().map_err(|_| format!("{key} is not a whole number: {v:?}")),
        None => Ok(default),
    }
}
