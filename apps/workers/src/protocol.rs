//! The worker protocol: newline-delimited JSON, one message per line. The API (control plane)
//! sends [`Request`]s; the worker answers with [`Event`]s. This module has no engine dependency,
//! so `forge-api` can share these exact types (it depends on this crate with the engine off).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A message from the API to the worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Is the worker alive and which engine session is open.
    Health,
    /// The tool catalogue from the registry (id, effects, requires, schema, availability).
    Catalog,
    /// Turn Deep mode on or off for this workspace's engine (reads inside targets).
    SetDeep { deep: bool },
    /// Run one tool. `consent_granted` is decided by the API after any approval; the worker never
    /// decides consent itself.
    Call {
        call_id: String,
        tool: String,
        #[serde(default)]
        params: Value,
        #[serde(default)]
        consent_granted: bool,
        #[serde(default)]
        deadline_ms: Option<u64>,
    },
    /// Cooperatively cancel a running call.
    Cancel { call_id: String },
    /// Stop accepting work and exit after in-flight calls finish.
    Shutdown,
}

/// A message from the worker to the API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Ready {
        session: SessionInfo,
    },
    Catalog {
        tools: Vec<Value>,
    },
    /// Progress of a running call (forwarded to clients over SSE/WebSocket).
    Progress {
        call_id: String,
        fraction: Option<f32>,
        message: Option<String>,
    },
    /// A call finished well. `output` is the engine's `ToolOutput` JSON; `evidence_id` is set
    /// when the tool recorded evidence.
    Result {
        call_id: String,
        output: Value,
        evidence_id: Option<String>,
        duration_ms: u64,
    },
    /// A call failed. `code` is the engine's machine-readable error code.
    CallError {
        call_id: String,
        code: String,
        message: String,
        remediation: Option<String>,
        retryable: bool,
        duration_ms: u64,
    },
    /// Deep mode changed.
    DeepSet {
        deep: bool,
    },
    /// A protocol-level problem (unparseable request, unknown call id to cancel). Not tied to a call.
    Fault {
        message: String,
    },
    /// The worker is shutting down.
    GoodBye,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionInfo {
    pub worker_version: String,
    pub deep: bool,
    /// Engine tool count in the registry.
    pub tools: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_events_round_trip_as_tagged_json() {
        let call = Request::Call {
            call_id: "c1".into(),
            tool: "core_target_triage".into(),
            params: serde_json::json!({"path": "/x"}),
            consent_granted: false,
            deadline_ms: Some(600_000),
        };
        let line = serde_json::to_string(&call).unwrap();
        assert!(line.contains("\"op\":\"call\""));
        assert_eq!(serde_json::from_str::<Request>(&line).unwrap(), call);

        // Fields the API may omit default sensibly.
        let minimal: Request = serde_json::from_str(r#"{"op":"call","call_id":"c","tool":"t"}"#).unwrap();
        assert_eq!(minimal, Request::Call { call_id: "c".into(), tool: "t".into(), params: Value::Null, consent_granted: false, deadline_ms: None });

        let ev = Event::Result { call_id: "c1".into(), output: serde_json::json!({"result": 1}), evidence_id: Some("ev_abc".into()), duration_ms: 12 };
        let line = serde_json::to_string(&ev).unwrap();
        assert!(line.contains("\"event\":\"result\""));
        assert_eq!(serde_json::from_str::<Event>(&line).unwrap(), ev);
    }
}
