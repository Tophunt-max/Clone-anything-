//! Talks the worker protocol to a `forge-worker serve` child process: one engine per project.
//!
//! In this phase the worker runs as a plain child process. Sandbox isolation (a rootless
//! container per workspace, quotas, no host access) wraps this spawn in Phase 8; the protocol
//! and everything above it stay the same.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use forge_worker::protocol::{Event, Request, SessionInfo};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::{Mutex, mpsc};

/// How the worker binary is launched. The default is the installed `forge-worker`; tests point
/// it at the one cargo just built.
#[derive(Clone, Debug)]
pub struct WorkerLaunch {
    pub program: PathBuf,
    pub workspace_root: PathBuf,
}

impl WorkerLaunch {
    pub fn from_env() -> Self {
        WorkerLaunch {
            program: std::env::var_os("FORGE_WORKER_BIN").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("forge-worker")),
            workspace_root: std::env::var_os("FORGE_WORKSPACE_ROOT").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("forge-workspaces")),
        }
    }
}

#[derive(Debug)]
pub enum WorkerError {
    Spawn(String),
    Protocol(String),
    Timeout,
    /// The engine refused the call. `code` is the engine's machine code.
    Call {
        code: String,
        message: String,
        remediation: Option<String>,
    },
}

/// The outcome of a tool call.
pub struct CallOk {
    pub output: Value,
    pub evidence_id: Option<String>,
    pub duration_ms: u64,
}

/// One worker child. Calls are serialized (the engine has one session anyway), so a single
/// reader can attribute every event to the one in-flight call.
pub struct Worker {
    stdin: Mutex<ChildStdin>,
    events: Mutex<mpsc::Receiver<Event>>,
    child: Mutex<Child>,
    pub session: SessionInfo,
}

impl Worker {
    pub async fn spawn(launch: &WorkerLaunch, workspace_id: &str, project_id: &str, deep: bool) -> Result<Worker, WorkerError> {
        let dir = launch.workspace_root.join(workspace_id).join(project_id);
        let config = launch.workspace_root.join(workspace_id).join(".craft");
        for d in [&dir, &config] {
            tokio::fs::create_dir_all(d).await.map_err(|e| WorkerError::Spawn(format!("workspace dir {}: {e}", d.display())))?;
        }
        let mut cmd = tokio::process::Command::new(&launch.program);
        // Run in the project dir so a tool's relative path (e.g. an uploaded `targets/x`) resolves there.
        cmd.current_dir(&dir);
        cmd.arg("serve").arg("--project").arg(&dir).arg("--config").arg(&config);
        if deep {
            cmd.arg("--deep");
        }
        cmd.env("CRAFT_ANYTHING_NO_KEYCHAIN", "1")
            .env("CRAFT_ANYTHING_CONFIG_DIR", &config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| WorkerError::Spawn(format!("{}: {e}", launch.program.display())))?;
        let stdin = child.stdin.take().ok_or_else(|| WorkerError::Spawn("no stdin".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| WorkerError::Spawn("no stdout".into()))?;
        let (tx, mut rx) = mpsc::channel::<Event>(64);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Event>(&line) {
                    Ok(ev) => {
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        // The worker greets with Ready (or Fault on a startup failure).
        let session = match tokio::time::timeout(Duration::from_secs(30), rx.recv()).await {
            Ok(Some(Event::Ready { session })) => session,
            Ok(Some(Event::Fault { message })) => return Err(WorkerError::Protocol(message)),
            Ok(Some(_)) => return Err(WorkerError::Protocol("worker did not greet with ready".into())),
            Ok(None) => return Err(WorkerError::Protocol("worker stopped before ready".into())),
            Err(_) => return Err(WorkerError::Timeout),
        };
        Ok(Worker { stdin: Mutex::new(stdin), events: Mutex::new(rx), child: Mutex::new(child), session })
    }

    async fn send(&self, req: &Request) -> Result<(), WorkerError> {
        let line = serde_json::to_string(req).map_err(|e| WorkerError::Protocol(e.to_string()))?;
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await.map_err(|e| WorkerError::Protocol(e.to_string()))?;
        stdin.write_all(b"\n").await.map_err(|e| WorkerError::Protocol(e.to_string()))?;
        stdin.flush().await.map_err(|e| WorkerError::Protocol(e.to_string()))?;
        Ok(())
    }

    /// Runs one tool and waits for its result. `progress` receives any progress messages.
    pub async fn call(
        &self,
        call_id: &str,
        tool: &str,
        params: Value,
        consent: bool,
        deadline: Duration,
        mut progress: impl FnMut(Option<f32>, Option<String>),
    ) -> Result<CallOk, WorkerError> {
        let mut events = self.events.lock().await; // serialize calls
        self.send(&Request::Call {
            call_id: call_id.to_string(),
            tool: tool.to_string(),
            params,
            consent_granted: consent,
            deadline_ms: Some(deadline.as_millis() as u64),
        })
        .await?;
        // A little past the engine's own deadline, so the worker reports the timeout itself.
        let overall = deadline + Duration::from_secs(10);
        loop {
            match tokio::time::timeout(overall, events.recv()).await {
                Ok(Some(Event::Progress { call_id: id, fraction, message })) if id == call_id => progress(fraction, message),
                Ok(Some(Event::Result { call_id: id, output, evidence_id, duration_ms })) if id == call_id => {
                    return Ok(CallOk { output, evidence_id, duration_ms });
                }
                Ok(Some(Event::CallError { call_id: id, code, message, remediation, .. })) if id == call_id => {
                    return Err(WorkerError::Call { code, message, remediation });
                }
                Ok(Some(_)) => {} // progress/events for other ids never happen (calls are serialized)
                Ok(None) => return Err(WorkerError::Protocol("worker stopped".into())),
                Err(_) => return Err(WorkerError::Timeout),
            }
        }
    }

    /// Switches Deep mode on the live worker and waits for the acknowledgement.
    pub async fn set_deep(&self, deep: bool) -> Result<(), WorkerError> {
        let mut events = self.events.lock().await;
        self.send(&Request::SetDeep { deep }).await?;
        match tokio::time::timeout(Duration::from_secs(10), events.recv()).await {
            Ok(Some(Event::DeepSet { .. })) => Ok(()),
            Ok(Some(_)) => Err(WorkerError::Protocol("expected deep_set".into())),
            _ => Err(WorkerError::Timeout),
        }
    }

    pub async fn catalog(&self) -> Result<Vec<Value>, WorkerError> {
        let mut events = self.events.lock().await;
        self.send(&Request::Catalog).await?;
        match tokio::time::timeout(Duration::from_secs(30), events.recv()).await {
            Ok(Some(Event::Catalog { tools })) => Ok(tools),
            Ok(Some(_)) => Err(WorkerError::Protocol("expected a catalogue".into())),
            _ => Err(WorkerError::Timeout),
        }
    }

    pub async fn shutdown(&self) {
        let _ = self.send(&Request::Shutdown).await;
        let mut child = self.child.lock().await;
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
        let _ = child.start_kill();
    }
}

/// Produces the global tool catalogue by running `forge-worker catalog` once. Used for `GET
/// /tools`; it needs no project.
pub async fn global_catalog(launch: &WorkerLaunch) -> Result<Vec<Value>, WorkerError> {
    let out = tokio::process::Command::new(&launch.program)
        .arg("catalog")
        .env("CRAFT_ANYTHING_NO_KEYCHAIN", "1")
        .env("CRAFT_ANYTHING_CONFIG_DIR", launch.workspace_root.join(".catalog-config"))
        .output()
        .await
        .map_err(|e| WorkerError::Spawn(format!("{}: {e}", launch.program.display())))?;
    if !out.status.success() {
        return Err(WorkerError::Protocol(String::from_utf8_lossy(&out.stderr).trim().to_string()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| WorkerError::Protocol(e.to_string()))
}

pub fn is_dir_safe(root: &Path, workspace_id: &str, project_id: &str) -> bool {
    // UUIDs only, so a crafted id can't escape the workspace root.
    let ok = |s: &str| uuid::Uuid::parse_str(s).is_ok();
    ok(workspace_id) && ok(project_id) && root.is_absolute()
}
