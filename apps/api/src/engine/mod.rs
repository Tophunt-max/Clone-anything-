//! The engine integration: the worker client, a small pool of worker processes, the tool
//! catalogue with its hosted-mode policy, and the deadlines per tool family.

pub mod policy;
pub mod pool;
pub mod redact;
pub mod worker;

use std::time::Duration;

pub use policy::{Policy, ToolInfo};
pub use pool::EnginePool;
pub use worker::{WorkerError, WorkerLaunch};

/// How long a tool may run before the worker cancels it. Most analysis is quick; tools that
/// launch a browser or a build get longer.
pub fn deadline_for(info: &ToolInfo) -> Duration {
    let eff = &info.effects;
    if eff.iter().any(|e| e == "ACCESSES_NETWORK" || e == "LAUNCHES_PROCESS" || e == "EXECUTES_FOREIGN_CODE") {
        Duration::from_secs(600)
    } else {
        Duration::from_secs(120)
    }
}
