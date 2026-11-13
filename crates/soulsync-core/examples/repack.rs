//! Rebuilds a ROM unchanged: `repack in.nds out.nds`. With `code` as a
//! third argument the ARM9 program and every overlay are unpacked and
//! packed again on the way.
use soulsync_core::nds::Nds;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(from), Some(to)) = (args.next(), args.next()) else {
        eprintln!("usage: repack <in.nds> <out.nds> [code]");
        std::process::exit(2);
    };
    let rom = std::fs::read(&from).expect("the ROM is readable");
    let mut nds = Nds::parse(&rom).expect("the file is a DS ROM");
    println!("{} files", nds.len());
    if args.next().as_deref() == Some("code") {
        let arm9 = nds.arm9().expect("the ARM9 program unpacks");
        println!("arm9: {} bytes", arm9.len());
        nds.set_arm9(&arm9).expect("the ARM9 program packs");
        let mut overlays = 0;
        while let Some(code) = nds.overlay(overlays) {
            nds.set_overlay(overlays, &code);
            overlays += 1;
        }
        println!("{overlays} overlays");
    }
    std::fs::write(&to, nds.build()).expect("the new ROM can be written");
}
