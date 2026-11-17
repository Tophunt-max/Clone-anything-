//! Redacts secret-looking fields from tool parameters before they are stored or logged. The
//! field list mirrors the engine's own journal redaction.

use serde_json::{Map, Value};

const SECRET_FIELDS: &[&str] = &[
    "api_key",
    "apikey",
    "secret",
    "password",
    "passphrase",
    "token",
    "access_token",
    "refresh_token",
    "authorization",
    "private_key",
    "hidden_values",
    "key",
];

fn is_secret(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    SECRET_FIELDS.contains(&n.as_str())
}

/// `v` with every secret-named field (at any depth) replaced by `"(redacted)"`.
pub fn params(v: &Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter().map(|(k, val)| (k.clone(), if is_secret(k) { Value::String("(redacted)".into()) } else { params(val) })).collect::<Map<_, _>>(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(params).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secrets_at_any_depth_are_hidden() {
        let out = params(&json!({"path": "/x", "api_key": "sk-real", "nested": {"Token": "t", "ok": 1}, "list": [{"password": "p"}]}));
        assert_eq!(out, json!({"path": "/x", "api_key": "(redacted)", "nested": {"Token": "(redacted)", "ok": 1}, "list": [{"password": "(redacted)"}]}));
    }
}
