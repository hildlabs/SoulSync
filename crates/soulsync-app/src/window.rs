//! The app's window: the dashboard in the system's web view.
//!
//! Windows and macOS bring a web view along (WebView2, WKWebView). Where
//! there is none, the dashboard opens in the default browser instead.

/// Work to do once the window is up, before the dashboard can show: it gets
/// a call that opens the dashboard when it is done.
pub type Starting = Box<dyn FnOnce(Box<dyn FnOnce() + Send>) + Send>;

/// Shows the dashboard and returns when the user closes the app. With
/// `starting`, the window first shows that SoulSync is starting.
pub fn show(url: &str, starting: Option<Starting>) {
    #[cfg(any(windows, target_os = "macos"))]
    desktop::show(url, starting);
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        if let Some(starting) = starting {
            starting(Box::new(|| {}));
        }
        let _ = open::that(url);
        println!("SoulSync is running: {url}");
        println!("Press Ctrl+C to quit.");
        loop {
            std::thread::park();
        }
    }
}

#[cfg(any(windows, target_os = "macos"))]
mod desktop {
    use tao::dpi::LogicalSize;
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    #[cfg(windows)]
    use tao::platform::windows::IconExtWindows;
    #[cfg(windows)]
    use tao::window::Icon;
    use tao::window::WindowBuilder;
    use wry::{WebContext, WebViewBuilder};

    /// Shown while a freshly updated SoulSync takes over from the old one.
    const STARTING: &str = r#"<!doctype html><meta charset="utf-8"><style>
html{color-scheme:dark;background:#0D0F13;color:#EDEFF3;font:15px/1.5 system-ui,sans-serif;height:100%}
body{margin:0;height:100%;display:grid;place-items:center;text-align:center}
i{display:block;width:28px;height:28px;margin:0 auto 16px;border-radius:50%;border:3px solid #272C36;border-top-color:#FF8D4D;animation:s 1s linear infinite}
@keyframes s{to{transform:rotate(1turn)}}small{color:#9CA3B0}
</style><body><div><i></i>SoulSync wird aktualisiert<br><small>SoulSync is updating</small></div>"#;

    pub fn show(url: &str, starting: Option<super::Starting>) {
        let event_loop = EventLoopBuilder::<()>::with_user_event().build();
        let window = WindowBuilder::new().with_title("SoulSync");
        // The icon is the first resource of the program; see `build.rs`.
        #[cfg(windows)]
        let window = window.with_window_icon(Icon::from_resource(1, None).ok());
        let window = window
            .with_inner_size(LogicalSize::new(1280.0, 800.0))
            .with_min_inner_size(LogicalSize::new(420.0, 560.0))
            .build(&event_loop);
        let Ok(window) = window else {
            return browser(url);
        };
        // Keeps the web view's files with the app's settings instead of next
        // to the program.
        let mut context = WebContext::new(crate::local::data_dir().map(|d| d.join("webview")));
        let own = url.to_owned();
        let builder = WebViewBuilder::with_web_context(&mut context);
        let builder = if starting.is_some() {
            builder.with_html(STARTING)
        } else {
            builder.with_url(url)
        };
        let webview = builder
            // Links to other sites belong in the browser.
            .with_navigation_handler(move |to| {
                let inside =
                    to.starts_with(&own) || to.starts_with("about:") || to.starts_with("data:");
                if !inside {
                    let _ = open::that(&to);
                }
                inside
            })
            .with_new_window_req_handler(|to| {
                let _ = open::that(&to);
                false
            })
            .build(&window);
        let Ok(webview) = webview else {
            // No web view on this computer, for example a Windows without
            // the WebView2 runtime.
            drop(window);
            if let Some(starting) = starting {
                starting(Box::new(|| {}));
            }
            return browser(url);
        };
        if let Some(starting) = starting {
            let proxy = event_loop.create_proxy();
            std::thread::spawn(move || {
                starting(Box::new(move || {
                    let _ = proxy.send_event(());
                }))
            });
        }
        let url = url.to_owned();
        event_loop.run(move |event, _, flow| {
            *flow = ControlFlow::Wait;
            if let Event::UserEvent(()) = event {
                let _ = webview.load_url(&url);
            }
            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } = event
            {
                *flow = ControlFlow::Exit;
            }
        });
    }

    /// Without a window of its own the app has nothing to close, so a small
    /// dialog stands in for it.
    fn browser(url: &str) {
        let _ = open::that(url);
        rfd::MessageDialog::new()
            .set_title("SoulSync")
            .set_description(
                "SoulSync is open in your browser. Close this message to quit SoulSync.\n\n\
                 SoulSync ist in deinem Browser geöffnet. Schließe diese Meldung, um SoulSync zu beenden.",
            )
            .show();
    }
}
