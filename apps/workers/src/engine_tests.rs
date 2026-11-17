#![cfg(feature = "engine")]
//! The engine role driven through the real protocol: it runs actual engine tools.

use std::io::Cursor;
use std::sync::Arc;

use craft_anything_platform::sync::Mutex;

use serde_json::{Value, json};

use crate::engine_role::EngineRole;
use crate::protocol::{Event, Request};

/// Feeds `requests` through a fresh engine over a temp project, returns every event emitted.
fn run(requests: &[Request], deep: bool) -> Vec<Event> {
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let role = EngineRole::open(dir.path(), config.path(), deep).unwrap();
    let mut input = String::new();
    for r in requests {
        input.push_str(&serde_json::to_string(r).unwrap());
        input.push('\n');
    }
    input.push_str(&serde_json::to_string(&Request::Shutdown).unwrap());
    input.push('\n');
    let out: Arc<Mutex<Vec<u8>>> = Default::default();
    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    role.serve(Cursor::new(input.into_bytes()), Shared(out.clone()));
    let bytes = out.lock().unwrap().clone();
    String::from_utf8(bytes).unwrap().lines().filter(|l| !l.is_empty()).map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn call(call_id: &str, tool: &str, params: Value) -> Request {
    Request::Call { call_id: call_id.into(), tool: tool.into(), params, consent_granted: false, deadline_ms: None }
}

#[test]
fn it_opens_ready_and_lists_the_catalogue() {
    let events = run(&[Request::Catalog], false);
    let ready = events.iter().find_map(|e| if let Event::Ready { session } = e { Some(session) } else { None }).expect("ready");
    assert!(ready.tools >= 300 && !ready.deep);
    let tools = events.iter().find_map(|e| if let Event::Catalog { tools } = e { Some(tools) } else { None }).expect("catalog");
    assert!(tools.iter().any(|t| t["id"] == "core_target_triage"));
    assert!(matches!(events.last(), Some(Event::GoodBye)));
}

#[test]
fn it_runs_a_real_tool_and_records_evidence() {
    // Triage a real file on disk and get back an evidence id.
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), b"#!/bin/sh\necho hello\n").unwrap();
    let events = run(&[call("c1", "core_target_triage", json!({"path": file.path()}))], true);
    let result = events
        .iter()
        .find_map(|e| if let Event::Result { call_id, evidence_id, output, .. } = e { (call_id == "c1").then_some((evidence_id, output)) } else { None });
    let (evidence_id, output) = result.expect("a result for c1");
    assert!(evidence_id.as_deref().is_some_and(|id| id.starts_with("ev_")), "triage records evidence: {evidence_id:?}");
    assert!(output.is_object());
}

#[test]
fn a_read_only_tool_needs_no_consent_but_a_code_running_tool_does() {
    let events = run(&[call("ro", "core_session_show", Value::Null), call("run", "build_command_run", json!({"command": "echo hi"}))], false);
    assert!(events.iter().any(|e| matches!(e, Event::Result { call_id, .. } if call_id == "ro")), "session_show runs");
    let err = events.iter().find_map(|e| if let Event::CallError { call_id, code, .. } = e { (call_id == "run").then_some(code.as_str()) } else { None });
    assert_eq!(err, Some("consent_required"), "running a command needs consent");
}

#[test]
fn granting_consent_passes_the_gate() {
    // build_command_run needs consent. Without it the gate refuses before the tool runs; with it
    // the call reaches the tool (which then reports it needs a build workspace). Either way this
    // proves the worker applies the consent the API decided, not a decision of its own.
    let granted = Request::Call {
        call_id: "run".into(),
        tool: "build_command_run".into(),
        params: json!({"command": "echo hi"}),
        consent_granted: true,
        deadline_ms: None,
    };
    let events = run(&[granted], true);
    let code = events.iter().find_map(|e| if let Event::CallError { call_id, code, .. } = e { (call_id == "run").then_some(code.as_str()) } else { None });
    assert_ne!(code, Some("consent_required"), "granted consent must pass the gate: {events:?}");
}

#[test]
fn an_unknown_tool_and_a_bad_request_are_reported_without_crashing() {
    let events = run(&[call("x", "no_such_tool", Value::Null)], false);
    assert!(events.iter().any(|e| matches!(e, Event::CallError { call_id, code, .. } if call_id == "x" && code == "bad_input")));

    // A malformed line is a fault, and the worker keeps going.
    let dir = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    let role = EngineRole::open(dir.path(), config.path(), false).unwrap();
    let input = "not json\n{\"op\":\"health\"}\n{\"op\":\"shutdown\"}\n";
    let out: Vec<u8> = Vec::new();
    let buf = Arc::new(Mutex::new(out));
    struct W(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for W {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    role.serve(Cursor::new(input.as_bytes().to_vec()), W(buf.clone()));
    let text = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert!(text.contains("\"event\":\"fault\""), "a bad line is a fault: {text}");
    assert!(text.matches("\"event\":\"ready\"").count() >= 2, "health answered after the bad line");
}

#[test]
fn deep_mode_toggles_what_is_available() {
    let events = run(&[Request::SetDeep { deep: true }, Request::Health], false);
    assert!(events.iter().any(|e| matches!(e, Event::DeepSet { deep: true })));
    assert!(events.iter().any(|e| matches!(e, Event::Ready { session } if session.deep)), "health reflects deep mode");
}
