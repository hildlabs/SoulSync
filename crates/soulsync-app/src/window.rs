//! The app's window: the dashboard in the system's web view.
//!
//! Windows and macOS bring a web view along (WebView2, WKWebView). Where
//! there is none, the dashboard opens in the default browser instead.

/// Shows the dashboard and returns when the user closes the app.
pub fn show(url: &str) {
    #[cfg(any(windows, target_os = "macos"))]
    desktop::show(url);
    #[cfg(not(any(windows, target_os = "macos")))]
    {
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
    use tao::event_loop::{ControlFlow, EventLoop};
    #[cfg(windows)]
    use tao::platform::windows::IconExtWindows;
    #[cfg(windows)]
    use tao::window::Icon;
    use tao::window::WindowBuilder;
    use wry::{WebContext, WebViewBuilder};

    pub fn show(url: &str) {
        let event_loop = EventLoop::new();
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
        let webview = WebViewBuilder::with_web_context(&mut context)
            .with_url(url)
            // Links to other sites belong in the browser.
            .with_navigation_handler(move |to| {
                let inside = to.starts_with(&own);
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
        let Ok(_webview) = webview else {
            // No web view on this computer, for example a Windows without
            // the WebView2 runtime.
            drop(window);
            return browser(url);
        };
        event_loop.run(move |event, _, flow| {
            *flow = ControlFlow::Wait;
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
