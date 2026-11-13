//! Randomizes a ROM with SoulSync's own randomizer, as far as it is
//! built: `randomize in.nds out.nds <seed> [option ...]`, with options
//! `dex`, `types`, `egg`, `held`, `goodheld`, `impossible`, `easier`,
//! `time`, `wild`, `starters` and `hm`.
use soulsync_core::rando::gen5;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [from, to, seed, flags @ ..] = args.as_slice() else {
        eprintln!("usage: randomize <in.nds> <out.nds> <seed> [option ...]");
        std::process::exit(2);
    };
    let on = |name: &str| flags.iter().any(|f| f == name);
    let options = gen5::Options {
        national_dex: on("dex"),
        update_type_effectiveness: on("types"),
        ban_lucky_egg: on("egg"),
        random_wild_held_items: on("held") || on("goodheld"),
        good_wild_held_items: on("goodheld"),
        change_impossible_evolutions: on("impossible"),
        easier_evolutions: on("easier"),
        no_time_evolutions: on("time"),
        random_wild: on("wild"),
        random_starters: on("starters"),
        full_hm: on("hm"),
    };
    let rom = std::fs::read(from).expect("the ROM is readable");
    let seed = seed.parse().expect("the seed is a number");
    let out = gen5::randomize(&rom, &options, seed).expect("the ROM is Black 2 or White 2");
    std::fs::write(to, out).expect("the new ROM can be written");
}
