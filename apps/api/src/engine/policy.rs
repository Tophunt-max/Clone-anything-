//! The hosted-mode permission policy for each engine tool, derived from its declared effects and
//! requirements (docs/platform/03-tool-inventory.md). This is the server-side gate; it mirrors
//! the logic the inventory doc is generated with.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;

/// Tools that change the host install, open host windows or manage agent-app registrations.
/// Not offered to tenants in hosted mode.
const HOST_ADMIN: &[&str] = &[
    "engines_install",
    "engines_app_open",
    "settings_set",
    "plugins_refresh",
    "plugins_add",
    "plugins_remove",
    "project_open_folder",
    "setup_plan",
    "setup_apply",
    "setup_doctor",
    "setup_uninstall",
    "assets_key_set",
    "assets_key_remove",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    /// Read-only: anyone who can view the project.
    Viewer,
    /// Changes session or project state, no code execution.
    Editor,
    /// Launches a process or uses the network; runs in the sandboxed worker.
    EditorWorker,
    /// Runs foreign code or changes a device: needs a human approval first.
    ApprovalGate,
    /// Operator-only (host administration); not offered to tenants.
    OperatorOnly,
    /// Not usable here.
    Unavailable,
}

impl Policy {
    pub fn as_str(&self) -> &'static str {
        match self {
            Policy::Viewer => "viewer",
            Policy::Editor => "editor",
            Policy::EditorWorker => "editor_worker",
            Policy::ApprovalGate => "approval_gate",
            Policy::OperatorOnly => "operator_only",
            Policy::Unavailable => "unavailable",
        }
    }
}

/// One tool as the API understands it, built from a catalogue entry.
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub id: String,
    pub needs_consent: bool,
    pub read_only: bool,
    pub requires: String,
    pub effects: Vec<String>,
    pub policy: Policy,
}

pub fn policy_for(id: &str, needs_consent: bool, read_only: bool, requires: &str, effects: &[String]) -> Policy {
    let eff: HashSet<&str> = effects.iter().map(String::as_str).collect();
    if HOST_ADMIN.contains(&id) {
        return Policy::OperatorOnly;
    }
    if let Some(oses) = requires.strip_prefix("os:")
        && !oses.split('|').any(|o| o == "linux")
    {
        return Policy::Unavailable;
    }
    if requires == "engine:adb" {
        return Policy::Unavailable; // needs a physical device
    }
    if needs_consent {
        return Policy::ApprovalGate;
    }
    if eff.contains("ACCESSES_NETWORK") || eff.contains("LAUNCHES_PROCESS") {
        return Policy::EditorWorker;
    }
    if read_only {
        return Policy::Viewer;
    }
    Policy::Editor
}

/// Builds a tool from a catalogue JSON entry (the worker's catalogue shape).
pub fn from_catalog(entry: &Value) -> Option<ToolInfo> {
    let id = entry.get("id")?.as_str()?.to_string();
    let needs_consent = entry.get("needs_consent").and_then(Value::as_bool).unwrap_or(false);
    let read_only = entry.get("read_only").and_then(Value::as_bool).unwrap_or(false);
    let requires = entry.get("requires").and_then(Value::as_str).unwrap_or("nothing").to_string();
    let effects: Vec<String> =
        entry.get("effects").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let policy = policy_for(&id, needs_consent, read_only, &requires, &effects);
    Some(ToolInfo { id, needs_consent, read_only, requires, effects, policy })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str, consent: bool, ro: bool, req: &str, eff: &[&str]) -> Policy {
        policy_for(id, consent, ro, req, &eff.iter().map(|s| s.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn the_policy_table() {
        assert_eq!(p("core_session_show", false, true, "nothing", &["IDEMPOTENT"]), Policy::Viewer);
        assert_eq!(p("core_target_open", false, false, "nothing", &["MUTATES_SESSION"]), Policy::Editor);
        assert_eq!(p("web_site_capture", false, false, "engine:chromium", &["LAUNCHES_PROCESS", "ACCESSES_NETWORK"]), Policy::EditorWorker);
        assert_eq!(p("build_command_run", true, false, "nothing", &["EXECUTES_FOREIGN_CODE"]), Policy::ApprovalGate);
        assert_eq!(p("web_site_open", true, false, "engine:chromium", &["EXECUTES_FOREIGN_CODE"]), Policy::ApprovalGate);
        assert_eq!(p("engines_install", false, false, "nothing", &["ACCESSES_NETWORK"]), Policy::OperatorOnly);
        assert_eq!(p("adb_devices_find", false, false, "engine:adb", &["LAUNCHES_PROCESS"]), Policy::Unavailable);
        assert_eq!(p("runtime_ui_read", true, false, "os:macos|windows", &["MUTATES_DEVICE"]), Policy::Unavailable);
    }
}
