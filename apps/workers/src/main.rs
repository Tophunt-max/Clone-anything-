//! `forge-worker <command>`. Exit codes: 0 success, 1 failure, 2 usage error.
//!
//! Commands:
//! - `catalog`: print the tool catalogue (JSON) from the engine registry.
//! - `version`: print the worker version.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cmd = std::env::args().nth(1);
    match cmd.as_deref() {
        Some("catalog") => {
            // The engine's own settings folder for this run: CRAFT_ANYTHING_CONFIG_DIR inside a
            // workspace sandbox, otherwise a scratch folder, never the person's own settings.
            let dir = std::env::var_os("CRAFT_ANYTHING_CONFIG_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("forge-worker"));
            let engine = forge_worker::engine(&dir);
            let tools = forge_worker::catalog(&engine);
            let text = match serde_json::to_string_pretty(&tools) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("error: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{text}").is_err() {
                return ExitCode::FAILURE;
            }
            ExitCode::SUCCESS
        }
        Some("version") | Some("--version") => {
            println!("forge-worker {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: forge-worker <catalog | version>");
            ExitCode::from(2)
        }
    }
}
