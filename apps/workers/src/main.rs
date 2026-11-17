//! `forge-worker` binary: see `engine_role::main`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

fn main() -> std::process::ExitCode {
    forge_worker::engine_role::main()
}
