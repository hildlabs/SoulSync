//! `soulsync-server`: runs the SoulSync run server.
//!
//! Set `SOULSYNC_DATA` to a directory to keep runs across restarts.

use soulsync_server::{app, AppState};
use std::net::SocketAddr;
use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "soulsync_server=info".into()),
        )
        .init();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let mut state = AppState::from_env();
    match std::env::var_os("SOULSYNC_DATA").filter(|d| !d.is_empty()) {
        Some(dir) => state = state.with_store(dir).expect("SOULSYNC_DATA is not usable"),
        None => tracing::warn!("SOULSYNC_DATA is not set: runs are lost on restart"),
    }
    let saver = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            saver.save();
        }
    });
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    tracing::info!("SoulSync server on http://localhost:{port}");
    axum::serve(listener, app(state.clone()))
        .with_graceful_shutdown(shutdown())
        .await
        .expect("serve");
    let n = state.save();
    tracing::info!(runs = n, "stored before shutdown");
}

/// Ctrl+C, or the SIGTERM that `docker stop` sends.
async fn shutdown() {
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("signal handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
