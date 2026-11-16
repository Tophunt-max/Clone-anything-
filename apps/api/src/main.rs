//! `forge-api` server binary. Exit codes: 0 clean shutdown, 1 runtime failure, 2 bad settings.
//!
//! `forge-api migrate` applies pending database migrations and exits.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use forge_api::{AppState, Config, MIGRATOR, router};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    tracing_subscriber::fmt().json().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))).init();
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let migrate_only = match std::env::args().nth(1).as_deref() {
        None => false,
        Some("migrate") => true,
        Some(other) => {
            eprintln!("error: unknown command {other:?}; usage: forge-api [migrate]");
            return ExitCode::from(2);
        }
    };
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: couldn't start the async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(run(config, migrate_only)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "stopped");
            ExitCode::FAILURE
        }
    }
}

async fn run(config: Config, migrate_only: bool) -> Result<(), String> {
    let db = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await
        .map_err(|e| format!("couldn't connect to the database: {e}"))?;
    if migrate_only || config.migrate_on_start {
        MIGRATOR.run(&db).await.map_err(|e| format!("migrations failed: {e}"))?;
        tracing::info!(migrations = MIGRATOR.iter().count(), "database is up to date");
    }
    if migrate_only {
        return Ok(());
    }
    let listener = tokio::net::TcpListener::bind(config.addr).await.map_err(|e| format!("couldn't listen on {}: {e}", config.addr))?;
    tracing::info!(addr = %config.addr, "listening");
    let app = router(AppState::new(&config, db.clone()), config.body_limit);
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| e.to_string())?;
    db.close().await;
    tracing::info!("shut down cleanly");
    Ok(())
}

/// Ctrl-C or SIGTERM (what Docker sends on `stop`): stop accepting, finish in-flight requests.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = term => {},
    }
    tracing::info!("shutdown requested");
}
