//! Dumps every registered tool (engine + AI layer + setup layer) as JSON, straight from the
//! registry, so the Phase 0 inventory is generated rather than hand-written.
use craft_anything_engine::{Effects, Engine, Requires, Session, Settings};
use serde_json::{Value, json};

fn effects(e: Effects) -> Vec<String> {
    e.iter_names().map(|(n, _)| n.to_string()).collect()
}

fn requires(r: Requires) -> Value {
    match r {
        Requires::Nothing => json!("nothing"),
        Requires::Target => json!("target"),
        Requires::Ghidra => json!("deep_engine"),
        Requires::Engine(id) => json!(format!("engine:{id}")),
        Requires::Os(list) => json!(format!("os:{}", list.join("|"))),
    }
}

fn main() {
    let mut extra = craft_anything_ai::tools();
    extra.extend(craft_anything_setup::tools::specs());
    let engine = Engine::shared(Session::new(Settings::default()), extra);
    let reg = engine.registry();
    let session = engine.session();
    let builtin: std::collections::HashSet<&str> =
        craft_anything_engine::Registry::with_all_tools().all().iter().map(|s| s.id).collect();
    let tools: Vec<Value> = reg
        .all()
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
        .collect();
    println!("{}", serde_json::to_string_pretty(&tools).unwrap_or_default());
}
