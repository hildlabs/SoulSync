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

mod fresh;
mod local;
mod tools;
mod update;
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
    update::tidy();
    // Started by an update: the old program still holds the port. Its window
    // stays until this one shows; then it is told to quit.
    let updated = std::env::args().any(|a| a == update::UPDATED_FLAG);
    let kept = std::sync::Arc::new(std::sync::Mutex::new(None));
    let handle = runtime.handle().clone();
    let serve = {
        let (kept, url) = (kept.clone(), url.clone());
        move |wait: bool| -> bool {
            let _in_runtime = handle.enter();
            let bind = || handle.block_on(tokio::net::TcpListener::bind(addr));
            let mut listener = bind();
            for _ in 0..120 {
                if listener.is_ok() || !wait {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(250));
                listener = bind();
            }
            // If the port is taken, SoulSync is most likely running already,
            // and the window shows that instance.
            let Ok(listener) = listener else {
                return false;
            };
            let mut server = soulsync_server::AppState::from_env();
            // Runs on this computer outlive the app, like runs on a server.
            if let Some(dir) = local::data_dir().map(|d| d.join("runs")) {
                if let Ok(stored) = server.clone().with_store(dir) {
                    server = stored;
                }
            }
            let saver = server.clone();
            handle.spawn(async move {
                let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
                loop {
                    tick.tick().await;
                    saver.save();
                }
            });
            *kept.lock().unwrap() = Some(server.clone());
            let app =
                soulsync_server::app(server.clone()).merge(local::routes(server, url.clone()));
            handle.spawn(async move { axum::serve(listener, app).await.expect("serve") });
            true
        }
    };
    let no_window = std::env::var_os("SOULSYNC_NO_WINDOW").is_some();
    if no_window || !updated {
        let served = serve(updated);
        if no_window {
            if !served {
                return;
            }
            println!("SoulSync is running: {url}");
            runtime.block_on(std::future::pending::<()>());
        }
        // Returns when the user closes the app.
        window::show(&url, None);
    } else {
        update::log("new program started, window up");
        let handoff = format!("{url}/api/local/handoff");
        window::show(
            &url,
            Some(Box::new(move |open: Box<dyn FnOnce() + Send>| {
                let _ = ureq::post(&handoff)
                    .header("x-soulsync", "1")
                    .config()
                    .timeout_global(Some(std::time::Duration::from_secs(3)))
                    .build()
                    .send_empty();
                let served = serve(true);
                update::log(if served {
                    "new program serves the dashboard"
                } else {
                    "port still taken, showing the running program"
                });
                open();
            })),
        );
    }
    let kept = kept.lock().unwrap().take();
    if let Some(server) = kept {
        server.save();
    }
}
