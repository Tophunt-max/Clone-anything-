//! `forge-worker`: the data plane (docs/platform/01-architecture.md §2).
//!
//! Phase 1 scope: link the original engine and expose its tool catalogue, straight from the
//! registry. The worker protocol, `Engine::call` execution, terminal and build roles come later.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::collections::HashSet;
use std::path::Path;

use craft_anything_engine::{Effects, Engine, Registry, Requires, Session};
use serde_json::{Value, json};

/// Every engine tool plus the AI and setup layer tools, registered the same way the upstream
/// CLI registers them.
pub fn engine(config_dir: &Path) -> std::sync::Arc<Engine> {
    let mut extra = craft_anything_ai::tools();
    extra.extend(craft_anything_setup::tools::specs());
    Engine::shared(Session::isolated(config_dir), extra)
}

fn effects(e: Effects) -> Vec<String> {
    e.iter_names().map(|(n, _)| n.to_string()).collect()
}

fn requires(r: Requires) -> String {
    match r {
        Requires::Nothing => "nothing".into(),
        Requires::Target => "target".into(),
        Requires::Ghidra => "deep_engine".into(),
        Requires::Engine(id) => format!("engine:{id}"),
        Requires::Os(list) => format!("os:{}", list.join("|")),
    }
}

/// The tool catalogue as JSON: id, layer, toolset, title, summary, effects, consent, requires,
/// availability on this host with this session's settings, and the input JSON Schema.
pub fn catalog(engine: &Engine) -> Vec<Value> {
    let reg = engine.registry();
    let builtin: HashSet<&str> = Registry::with_all_tools().all().iter().map(|s| s.id).collect();
    let session = engine.session();
    reg.all()
        .iter()
        .map(|s| {
            let av = engine.availability(s, &session);
            json!({
                "id": s.id,
                "layer": if builtin.contains(s.id) { "engine" } else { "ai_or_setup" },
                "toolset": s.toolset.id(),
                "title": s.title,
                "summary": s.summary,
                "effects": effects(s.effects),
                "needs_consent": s.effects.needs_consent(),
                "read_only": s.effects.read_only_hint(),
                "requires": requires(s.requires),
                "available_here": av.available,
                "unavailable_reason": av.reason,
                "input_schema": reg.input_schema(s),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("forge-worker-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        d
    }

    #[test]
    fn catalog_lists_every_engine_tool_and_the_layer_tools() {
        let e = engine(&temp());
        let cat = catalog(&e);
        let engine_tools = cat.iter().filter(|t| t["layer"] == "engine").count();
        assert!(engine_tools >= craft_anything_engine::tool_list::MIN_TOOLS, "{engine_tools}");
        assert_eq!(engine_tools, Registry::with_all_tools().len());
        assert!(cat.iter().any(|t| t["id"] == "rebuild_start" && t["layer"] == "ai_or_setup"));
    }

    #[test]
    fn consent_flags_come_from_the_engine() {
        let e = engine(&temp());
        let cat = catalog(&e);
        let tool = |id: &str| cat.iter().find(|t| t["id"] == id).cloned().unwrap();
        assert_eq!(tool("build_command_run")["needs_consent"], true);
        assert_eq!(tool("core_session_show")["needs_consent"], false);
        assert!(tool("core_target_open")["input_schema"].is_object());
    }
}
