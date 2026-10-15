//! Process entry point: settings, tracing, listener, serve.
//! All logic lives in the library so tests can drive it in-process.

use std::process::ExitCode;

use steleofd_server::api;
use steleofd_server::{Settings, Store};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("steleofd-server: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Bind the configured address and serve until the process is asked to stop.
async fn run() -> Result<(), String> {
    let settings = Settings::from_env().map_err(|err| err.to_string())?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let store = Store::new(settings.max_documents);
    let app = api::router(store, settings.max_body_bytes);
    let listener = tokio::net::TcpListener::bind(settings.bind)
        .await
        .map_err(|err| format!("cannot bind {}: {err}", settings.bind))?;
    let local = listener
        .local_addr()
        .map_err(|err| format!("cannot read the bound address: {err}"))?;
    tracing::info!(address = %local, "steleofd-server is listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|err| format!("the server stopped: {err}"))
}

/// Resolve once the process is interrupted, so in-flight work can finish.
async fn shutdown() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::warn!(error = %err, "no interrupt handler, waiting for a signal");
    }
}
