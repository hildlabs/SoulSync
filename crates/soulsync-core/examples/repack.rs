//! Rebuilds a ROM unchanged: `repack in.nds out.nds`.
use soulsync_core::nds::Nds;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(from), Some(to)) = (args.next(), args.next()) else {
        eprintln!("usage: repack <in.nds> <out.nds>");
        std::process::exit(2);
    };
    let rom = std::fs::read(&from).expect("the ROM is readable");
    let nds = Nds::parse(&rom).expect("the file is a DS ROM");
    println!("{} files", nds.len());
    std::fs::write(&to, nds.build()).expect("the new ROM can be written");
}
