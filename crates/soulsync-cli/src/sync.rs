//! `soulsync new` and `soulsync sync`: talk to a run server.

use crate::i18n::Text;
use crate::{describe, Res};
use soulsync_client::{base, create_run, load, session, ws_url, End, Update};
use soulsync_core::protocol::RunSettings;
use soulsync_core::Lang;
use std::path::PathBuf;
use std::time::Duration;

pub use soulsync_client::DEFAULT_SERVER;

pub fn new_run(server: &str, t: &Text) -> Res {
    let run = create_run(server, &RunSettings::default()).map_err(|e| e.to_string())?;
    let server_arg = if base(server) == DEFAULT_SERVER {
        String::new()
    } else {
        format!(" --server {}", base(server))
    };
    println!("{}", t.run_created.replace("{code}", &run.code));
    println!(
        "{}",
        t.run_share
            .replace("{code}", &run.code)
            .replace("{server}", &server_arg)
            .replace("{dashboard}", &format!("{}/run/{}", base(server), run.code))
    );
    Ok(())
}

pub fn sync(
    path: PathBuf,
    server: &str,
    code: &str,
    name: &str,
    catch_rates: Option<Vec<u8>>,
    lang: Lang,
    t: &'static Text,
) -> Res {
    let url = ws_url(server, code, name);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move {
        let mut last = load(&path).map_err(|e| e.to_string())?;
        let mut backoff = 1u64;
        let mut report = |u: Update| match u {
            Update::Connected => eprintln!(
                "{}",
                t.connected
                    .replace("{name}", name)
                    .replace("{path}", &path.display().to_string())
            ),
            Update::Sent(events) => {
                for e in events {
                    println!("{}", describe(e, lang, t));
                }
                eprintln!("{}", t.sent);
            }
        };
        loop {
            eprintln!("{}", t.connecting.replace("{url}", &url));
            match session(&url, &path, catch_rates.as_deref(), &mut last, &mut report).await {
                Ok(End::Refused(msg)) => return Err(format!("{}: {msg}", t.server_error).into()),
                Ok(End::Lost) => backoff = 1,
                Err(e) => eprintln!("{}: {e}", t.error),
            }
            eprintln!("{}", t.disconnected.replace("{secs}", &backoff.to_string()));
            tokio::time::sleep(Duration::from_secs(backoff)).await;
            backoff = (backoff * 2).min(30);
        }
    })
}
