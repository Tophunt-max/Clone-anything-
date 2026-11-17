//! The tool catalogue, straight from the engine registry (engine role only).

use std::collections::HashSet;

use craft_anything_engine::{Effects, Engine, Registry, Requires};
use serde_json::{Value, json};

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

/// Every tool with id, layer, toolset, title, summary, effects, consent, requires, availability
/// in this session and the input JSON Schema. The API adds its platform policy on top.
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
