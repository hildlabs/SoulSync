//! Randomizes a ROM with SoulSync's own randomizer, as far as it is
//! built: `randomize in.nds out.nds <seed> [option ...]`, with options
//! `dex`, `types`, `egg`, `held`, `goodheld`, `impossible`, `easier`,
//! `time`, `wild`, `starters`, `hm`, `tutors`, `trainers`, `forms`,
//! `rival`, `evolved` (from level 40), `names`, `statics`, `wildforms`, `trades` and `tradeall` (wanted
//! Pokémon, nicknames, IVs and items of the trades), `field`,
//! `goodfield`, `shops`, `goodshops`, `pickup`, `goodpickup`, `tms`, and
//! `tmrandom`, `tmtype`, `tmfull`, `tutorrandom`, `tutortype`, `tutorfull`
//! for who learns the moves of TMs and tutors; `statsshuffle`,
//! `statsrandom`, `statsevo`, `statsgrow`, `ptypes`, `ptypesevo`,
//! `twotypes`, `abilities`, `abilitiesevo` and `wonderguard` for what the
//! species are like; `movepower`, `moveaccuracy`, `movepp` and `movetypes`
//! for the moves.
use soulsync_core::rando::gen5;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [from, to, seed, flags @ ..] = args.as_slice() else {
        eprintln!("usage: randomize <in.nds> <out.nds> <seed> [option ...]");
        std::process::exit(2);
    };
    let on = |name: &str| flags.iter().any(|f| f == name);
    let compat = |name: &str| {
        use soulsync_core::rnqs::Compat;
        if on(&format!("{name}full")) {
            Compat::Full
        } else if on(&format!("{name}type")) {
            Compat::RandomPreferType
        } else if on(&format!("{name}random")) {
            Compat::Random
        } else {
            Compat::Unchanged
        }
    };
    let change = |name: &str| {
        if on(&format!("{name}evo")) {
            gen5::Change::FollowEvolutions
        } else if on(name) {
            gen5::Change::Random
        } else {
            gen5::Change::Unchanged
        }
    };
    let options = gen5::Options {
        random_move_powers: on("movepower"),
        random_move_accuracies: on("moveaccuracy"),
        random_move_pps: on("movepp"),
        random_move_types: on("movetypes"),
        stats: if on("statsshuffle") {
            gen5::Stats::Shuffle
        } else if on("statsrandom") {
            gen5::Stats::Random
        } else {
            gen5::Stats::Unchanged
        },
        stats_follow_evolutions: on("statsevo"),
        stats_grow_at_random: on("statsgrow"),
        types: change("ptypes"),
        two_types: on("twotypes"),
        abilities: change("abilities"),
        wonder_guard: on("wonderguard"),
        random_tm_moves: on("tms"),
        machines: compat("tm"),
        tutors: compat("tutor"),
        national_dex: on("dex"),
        update_type_effectiveness: on("types"),
        ban_lucky_egg: on("egg"),
        random_wild_held_items: on("held") || on("goodheld"),
        good_wild_held_items: on("goodheld"),
        change_impossible_evolutions: on("impossible"),
        easier_evolutions: on("easier"),
        no_time_evolutions: on("time"),
        random_wild: on("wild"),
        wild_forms: on("wildforms"),
        random_starters: on("starters"),
        full_hm: on("hm"),
        random_tutor_moves: on("tutors"),
        random_trainers: on("trainers"),
        trainer_forms: on("forms"),
        rival_carries_starter: on("rival"),
        trainers_fully_evolved_from: if on("evolved") { 40 } else { 0 },
        random_trainer_names: on("names"),
        random_statics: on("statics"),
        random_shops: on("shops") || on("goodshops"),
        good_shop_items: on("goodshops"),
        random_pickup: on("pickup") || on("goodpickup"),
        good_pickup_items: on("goodpickup"),
        random_field_items: on("field") || on("goodfield"),
        good_field_items: on("goodfield"),
        random_trades: on("trades") || on("tradeall"),
        trades_want_random: on("tradeall"),
        random_trade_nicknames: on("tradeall"),
        random_trade_ivs: on("tradeall"),
        random_trade_items: on("tradeall"),
    };
    let rom = std::fs::read(from).expect("the ROM is readable");
    let seed = seed.parse().expect("the seed is a number");
    let out = gen5::randomize(&rom, &options, seed).expect("the ROM is Black 2 or White 2");
    std::fs::write(to, out).expect("the new ROM can be written");
}
