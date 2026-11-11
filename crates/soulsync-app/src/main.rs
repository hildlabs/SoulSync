//! `SoulSync`: the desktop app. Double-click it, pick your ROM and save,
//! start or join a run, and it keeps your game in sync and shows the
//! dashboard.
//!
//! The app is a small local web server. It serves the dashboard on this
//! computer only and adds a local API the dashboard's Settings tab talks to.
//! The ROM and the save never leave the computer; only what the dashboard
//! shows is sent to the run server.

mod local;

use std::net::SocketAddr;

/// Fixed, so a second start finds the first.
const PORT: u16 = 4777;

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
    let url = format!("http://127.0.0.1:{PORT}");
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(_) => {
            // Most likely SoulSync is already running: show it.
            let _ = open::that(&url);
            return;
        }
    };
    let server = soulsync_server::AppState::from_env();
    let app = soulsync_server::app(server.clone()).merge(local::routes(server, url.clone()));
    println!("SoulSync is running: {url}");
    println!("SoulSync läuft. Schließe dieses Fenster, um SoulSync zu beenden.");
    if std::env::var_os("SOULSYNC_NO_BROWSER").is_none() {
        let _ = open::that(&url);
    }
    axum::serve(listener, app).await.expect("serve");
}
