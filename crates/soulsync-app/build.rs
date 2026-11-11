//! Gives `SoulSync.exe` its icon.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/soulsync.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // The window icon is loaded from this resource as well, by its id 1.
        winresource::WindowsResource::new()
            .set_icon("../../assets/soulsync.ico")
            .compile()
            .expect("icon resource");
    }
}
