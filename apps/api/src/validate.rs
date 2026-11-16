//! Field rules shared by handlers. Each returns the cleaned value or a 422 naming the field.

use crate::error::ApiError;

pub fn email(raw: &str) -> Result<String, ApiError> {
    let e = raw.trim().to_lowercase();
    let ok = e.len() <= 254
        && !e.chars().any(char::is_whitespace)
        && e.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.') && !domain.contains('@')
        });
    if ok { Ok(e) } else { Err(ApiError::invalid("email", "Enter a valid email address.")) }
}

/// Names shown to people: trimmed, 1–100 characters, no control characters.
pub fn name(field: &str, raw: &str) -> Result<String, ApiError> {
    let n = raw.trim();
    let len = n.chars().count();
    if len == 0 || len > 100 || n.chars().any(char::is_control) {
        return Err(ApiError::invalid(field, "Use 1 to 100 characters, without line breaks."));
    }
    Ok(n.to_string())
}

/// 3–40 lowercase letters, digits and dashes; starts and ends with a letter or digit.
pub fn slug(raw: &str) -> Result<String, ApiError> {
    let s = raw.trim();
    let b = s.as_bytes();
    let edge = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    let ok =
        (3..=40).contains(&b.len()) && b.first().is_some_and(|&c| edge(c)) && b.last().is_some_and(|&c| edge(c)) && b.iter().all(|&c| edge(c) || c == b'-');
    if ok {
        Ok(s.to_string())
    } else {
        Err(ApiError::invalid("slug", "Use 3 to 40 lowercase letters, digits or dashes, starting and ending with a letter or digit."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert_eq!(email(" A@Example.COM ").unwrap(), "a@example.com");
        for bad in ["", "a", "a@b", "@b.co", "a@.co", "a@b.co.", "a b@c.co", "a@b@c.co"] {
            assert!(email(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn names_and_slugs() {
        assert_eq!(name("name", "  Team  ").unwrap(), "Team");
        assert!(name("name", "").is_err());
        assert!(name("name", "a\nb").is_err());
        assert!(name("name", &"x".repeat(101)).is_err());
        assert!(slug("my-team-1").is_ok());
        for bad in ["ab", "-ab", "ab-", "AB c", "a_b_c"] {
            assert!(slug(bad).is_err(), "{bad}");
        }
    }
}
