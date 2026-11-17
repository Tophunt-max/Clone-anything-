//! The engine role: one `Engine` for one workspace project, served over the worker protocol.
//! Calls run on worker threads so the reader can still receive `Cancel` and `Shutdown` while a
//! call is in flight; the engine serializes them on its own session lock.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, PoisonError};

use craft_anything_platform::sync::Mutex;
use std::time::Instant;

use craft_anything_engine::evidence::ToolError;
use craft_anything_engine::project::{Project, Purpose};
use craft_anything_engine::{CallOptions, Caller, CancelToken, Consent, Engine, Progress, Session, ToolOutput};
use serde_json::{Value, json};

use crate::catalog;
use crate::protocol::{Event, Request, SessionInfo};

/// Builds the engine for `project_dir`, creating the project if needed. Settings come from
/// `config_dir` (set per sandbox); the key store is never the plaintext file (the API injects
/// provider keys as environment variables when a job needs them).
pub struct EngineRole {
    engine: Arc<Engine>,
    cancels: Arc<Mutex<HashMap<String, CancelToken>>>,
}

impl EngineRole {
    pub fn open(project_dir: &Path, config_dir: &Path, deep: bool) -> Result<Self, ToolError> {
        // Keep settings and the engine store inside the sandbox, never the person's own folders.
        let mut session = Session::isolated(config_dir);
        session.settings.deep_mode = deep;
        session.settings.ask_before_running = true;
        session.open_project(open_or_create(project_dir)?)?;
        let extra = {
            let mut t = craft_anything_ai::tools();
            t.extend(craft_anything_setup::tools::specs());
            t
        };
        Ok(EngineRole { engine: Engine::shared(session, extra), cancels: Default::default() })
    }

    fn session_info(&self) -> SessionInfo {
        let deep = !self.engine.session().clean_room();
        SessionInfo { worker_version: env!("CARGO_PKG_VERSION").to_string(), deep, tools: self.engine.registry().all().len() }
    }

    /// Serves one protocol conversation until end-of-input or `Shutdown`.
    pub fn serve(self, reader: impl BufRead, writer: impl Write + Send + 'static) {
        let (tx, rx) = channel::<Event>();
        // One thread owns the writer; every producer sends events to it, so lines never interleave.
        let writer_thread = std::thread::spawn(move || {
            let mut w = writer;
            for ev in rx {
                let line = serde_json::to_string(&ev).unwrap_or_else(|e| format!(r#"{{"event":"fault","message":"serialize: {e}"}}"#));
                if writeln!(w, "{line}").is_err() || w.flush().is_err() {
                    break;
                }
            }
        });

        let _ = tx.send(Event::Ready { session: self.session_info() });
        let engine = self.engine;
        let cancels = self.cancels;
        let mut calls = Vec::new();
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Request>(&line) {
                Err(e) => {
                    let _ = tx.send(Event::Fault { message: format!("bad request: {e}") });
                }
                Ok(Request::Shutdown) => break,
                Ok(Request::Health) => {
                    let _ = tx.send(Event::Ready { session: session_info(&engine) });
                }
                Ok(Request::Catalog) => {
                    let _ = tx.send(Event::Catalog { tools: catalog::catalog(&engine) });
                }
                Ok(Request::SetDeep { deep }) => {
                    engine.session().settings.deep_mode = deep;
                    let _ = tx.send(Event::DeepSet { deep });
                }
                Ok(Request::Cancel { call_id }) => {
                    if let Some(tok) = cancels.lock().unwrap_or_else(PoisonError::into_inner).get(&call_id) {
                        tok.cancel();
                    }
                }
                Ok(Request::Call { call_id, tool, params, consent_granted, deadline_ms }) => {
                    let (engine, cancels, tx) = (engine.clone(), cancels.clone(), tx.clone());
                    calls.push(std::thread::spawn(move || run_call(&engine, &cancels, &tx, call_id, tool, params, consent_granted, deadline_ms)));
                }
            }
        }
        for c in calls {
            let _ = c.join();
        }
        let _ = tx.send(Event::GoodBye);
        drop(tx);
        let _ = writer_thread.join();
    }
}

fn session_info(engine: &Engine) -> SessionInfo {
    SessionInfo { worker_version: env!("CARGO_PKG_VERSION").to_string(), deep: !engine.session().clean_room(), tools: engine.registry().all().len() }
}

#[allow(clippy::too_many_arguments)]
fn run_call(
    engine: &Engine,
    cancels: &Mutex<HashMap<String, CancelToken>>,
    tx: &Sender<Event>,
    call_id: String,
    tool: String,
    params: Value,
    consent_granted: bool,
    deadline_ms: Option<u64>,
) {
    let cancel = CancelToken::default();
    cancels.lock().unwrap_or_else(PoisonError::into_inner).insert(call_id.clone(), cancel.clone());
    // A deadline cancels the token from a timer thread; tools also carry their own limits.
    if let Some(ms) = deadline_ms {
        let (c, t) = (cancel.clone(), ms);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(t));
            c.cancel();
        });
    }
    let progress = {
        let (tx, call_id) = (tx.clone(), call_id.clone());
        move |p: Progress| {
            let _ = tx.send(Event::Progress { call_id: call_id.clone(), fraction: p.fraction, message: Some(p.message).filter(|m| !m.is_empty()) });
        }
    };
    let consent = if consent_granted { Consent::Granted } else { Consent::NotAsked };
    let opts = CallOptions::new(Caller::Mcp).with_consent(consent).with_cancel(cancel).with_progress(&progress);
    let started = Instant::now();
    let result = engine.call(&tool, params, opts);
    let duration_ms = started.elapsed().as_millis() as u64;
    cancels.lock().unwrap_or_else(PoisonError::into_inner).remove(&call_id);
    let ev = match result {
        Ok(output) => {
            let evidence_id = match &output {
                ToolOutput::Evidence(e) => Some(e.id.clone()),
                ToolOutput::Value(_) => None,
            };
            Event::Result { call_id, output: output.to_json(), evidence_id, duration_ms }
        }
        Err(e) => Event::CallError {
            call_id,
            code: serde_json::to_value(e.code).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_else(|| "internal".into()),
            message: e.message,
            remediation: e.remediation,
            retryable: e.retryable,
            duration_ms,
        },
    };
    let _ = tx.send(ev);
}

fn open_or_create(dir: &Path) -> Result<Project, ToolError> {
    let dir: PathBuf = std::path::absolute(dir).map_err(|e| ToolError::io(dir.display(), e))?;
    if dir.join("project.toml").is_file() {
        Project::open(&dir)
    } else {
        let name = dir.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "project".into());
        Project::create(&dir, &name, Purpose::default())
    }
}

/// `forge-worker serve --project <dir> [--config <dir>] [--deep]` over stdin/stdout.
pub fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let mut project: Option<PathBuf> = None;
    let mut config: Option<PathBuf> = None;
    let mut deep = false;
    let mut mode = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "serve" => mode = Some("serve"),
            "catalog" => mode = Some("catalog"),
            "version" | "--version" => {
                println!("forge-worker {}", env!("CARGO_PKG_VERSION"));
                return std::process::ExitCode::SUCCESS;
            }
            "--project" => project = args.next().map(PathBuf::from),
            "--config" => config = args.next().map(PathBuf::from),
            "--deep" => deep = true,
            other => {
                eprintln!("forge-worker: unexpected argument {other:?}");
                return std::process::ExitCode::from(2);
            }
        }
    }
    let config = config.unwrap_or_else(|| std::env::temp_dir().join("forge-worker-config"));
    match mode {
        Some("serve") => {
            let Some(project) = project else {
                eprintln!("forge-worker serve: --project <dir> is required");
                return std::process::ExitCode::from(2);
            };
            match EngineRole::open(&project, &config, deep) {
                Ok(role) => {
                    let stdin = std::io::stdin();
                    role.serve(stdin.lock(), std::io::stdout());
                    std::process::ExitCode::SUCCESS
                }
                Err(e) => {
                    // A startup failure is reported as a single protocol fault, then exit.
                    println!("{}", serde_json::to_string(&Event::Fault { message: e.message }).unwrap_or_default());
                    std::process::ExitCode::FAILURE
                }
            }
        }
        Some("catalog") => {
            let role = match EngineRole::open(&std::env::temp_dir().join("forge-worker-catalog"), &config, deep) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("forge-worker: {}", e.message);
                    return std::process::ExitCode::FAILURE;
                }
            };
            println!("{}", serde_json::to_string_pretty(&json!(catalog::catalog(&role.engine))).unwrap_or_default());
            std::process::ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: forge-worker <serve --project <dir> [--config <dir>] [--deep] | catalog | version>");
            std::process::ExitCode::from(2)
        }
    }
}
