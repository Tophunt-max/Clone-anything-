//! Session cookies for browsers. The access and refresh cookies are HttpOnly, so page scripts
//! can't read them; the CSRF cookie is readable on purpose (double-submit).

use axum::http::header::COOKIE;
use axum::http::{HeaderMap, HeaderValue};

pub const ACCESS: &str = "forge_at";
pub const REFRESH: &str = "forge_rt";
pub const CSRF: &str = "forge_csrf";

pub const ACCESS_PATH: &str = "/api";
pub const REFRESH_PATH: &str = "/api/v1/auth";
pub const CSRF_PATH: &str = "/";

/// The value of cookie `name` from the request's `Cookie` headers.
pub fn get<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .filter_map(|part| part.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

pub fn set(name: &str, value: &str, path: &str, max_age_secs: i64, http_only: bool, secure: bool) -> Option<HeaderValue> {
    let mut s = format!("{name}={value}; Path={path}; Max-Age={max_age_secs}; SameSite=Strict");
    if http_only {
        s.push_str("; HttpOnly");
    }
    if secure {
        s.push_str("; Secure");
    }
    HeaderValue::from_str(&s).ok()
}

pub fn clear(name: &str, path: &str, http_only: bool, secure: bool) -> Option<HeaderValue> {
    set(name, "", path, 0, http_only, secure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_named_cookie_from_any_header() {
        let mut h = HeaderMap::new();
        h.append(COOKIE, HeaderValue::from_static("a=1; forge_at=tok"));
        h.append(COOKIE, HeaderValue::from_static("forge_csrf=c"));
        assert_eq!(get(&h, ACCESS), Some("tok"));
        assert_eq!(get(&h, CSRF), Some("c"));
        assert_eq!(get(&h, REFRESH), None);
    }

    #[test]
    fn set_cookie_has_the_safety_attributes() {
        let v = set(ACCESS, "t", ACCESS_PATH, 600, true, true).unwrap();
        let s = v.to_str().unwrap();
        for part in ["forge_at=t", "Path=/api", "Max-Age=600", "SameSite=Strict", "HttpOnly", "Secure"] {
            assert!(s.contains(part), "{s}");
        }
        assert!(!set(CSRF, "c", "/", 1, false, true).unwrap().to_str().unwrap().contains("HttpOnly"));
        assert!(set(ACCESS, "bad\nvalue", "/", 1, true, true).is_none());
    }
}
