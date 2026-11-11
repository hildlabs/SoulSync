//! `SoulSync`: the desktop app. Double-click it, pick your ROM and save,
//! start or join a run, and it keeps your game in sync and shows the
//! dashboard in its own window.
//!
//! Inside, the app is a small web server that only this computer can reach,
//! and a window that shows it. The server serves the dashboard and adds a
//! local API the dashboard's Settings tab talks to. The ROM and the save
//! never leave the computer; only what the dashboard shows is sent to the run
//! server.

// No console window on Windows.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod local;
mod window;

use std::net::SocketAddr;

/// Fixed, so a second start finds the first.
const PORT: u16 = 4777;

fn main() {
    // The app has no console on Windows, so a crash would go unseen. What went
    // wrong is kept next to the settings.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = local::data_dir() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("crash.txt"), info.to_string());
        }
        default_hook(info);
    }));
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    // Resuming a run at start spawns its sync task from here, outside of any
    // task, which needs the runtime entered.
    let _in_runtime = runtime.enter();
    let addr = SocketAddr::from(([127, 0, 0, 1], PORT));
    let url = format!("http://127.0.0.1:{PORT}");
    // If the port is taken, SoulSync is most likely running already, and the
    // window shows that instance.
    if let Ok(listener) = runtime.block_on(tokio::net::TcpListener::bind(addr)) {
        let server = soulsync_server::AppState::from_env();
        let app = soulsync_server::app(server.clone()).merge(local::routes(server, url.clone()));
        runtime.spawn(async move { axum::serve(listener, app).await.expect("serve") });
    } else if std::env::var_os("SOULSYNC_NO_WINDOW").is_some() {
        return;
    }
    if std::env::var_os("SOULSYNC_NO_WINDOW").is_some() {
        println!("SoulSync is running: {url}");
        runtime.block_on(std::future::pending::<()>());
    }
    // Returns when the user closes the app.
    window::show(&url);
}
