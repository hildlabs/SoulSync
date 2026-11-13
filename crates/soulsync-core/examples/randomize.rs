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
//! for the moves; `movesets`, `movesetstype`, `metronome`, `fourmoves`, `goodattacks`
//! (40 percent), `nobroken` and `moveorder` for what the species learn;
//! `evolutions` with `evostrength`, `evotype`, `evothree` and `evochange`
//! for what they evolve into; `trainerseven`, `trainersstory`,
//! `trainerthemes`, `trainergyms`, `trainerweights`,
//! `trainerstrength`, `trainernolegends`, `trainernoguard` and
//! `trainerlevels` (20 percent up) for the trainers.
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
    // `preset=<file>` takes everything from a settings file instead.
    let preset = flags.iter().find_map(|f| f.strip_prefix("preset="));
    let preset = preset.map(|path| {
        let file = std::fs::read(path).expect("reading the settings file");
        let preset = soulsync_core::rnqs::Preset::read(&file).expect("a settings file");
        preset.native().unwrap_or_else(|| {
            eprintln!("these settings are not built yet");
            std::process::exit(3);
        })
    });
    let options = preset.unwrap_or(gen5::Options {
        random_move_powers: on("movepower"),
        random_move_accuracies: on("moveaccuracy"),
        random_move_pps: on("movepp"),
        random_move_types: on("movetypes"),
        movesets: if on("movesets") {
            gen5::Movesets::Random
        } else if on("movesetstype") {
            gen5::Movesets::PreferType
        } else if on("metronome") {
            gen5::Movesets::Metronome
        } else {
            gen5::Movesets::Unchanged
        },
        moves_at_start: if on("fourmoves") { 4 } else { 0 },
        good_attacks: if on("goodattacks") { 40 } else { 0 },
        no_broken_moves: on("nobroken"),
        order_moves_by_damage: on("moveorder"),
        random_evolutions: on("evolutions").then_some(gen5::Evolving {
            similar_strength: on("evostrength"),
            same_type: on("evotype"),
            three_stages: on("evothree"),
            force_change: on("evochange"),
        }),
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
        trainer_rules: gen5::TrainerRules {
            spread: if on("trainerseven") {
                gen5::Spread::Even
            } else if on("trainersstory") {
                gen5::Spread::EvenInStory
            } else {
                gen5::Spread::Free
            },
            themes: if on("trainerthemes") {
                gen5::Themes::All
            } else if on("trainergyms") {
                gen5::Themes::GymsAndElite
            } else {
                gen5::Themes::None
            },
            type_weights: on("trainerweights"),
            similar_strength: on("trainerstrength"),
            no_legendaries: on("trainernolegends"),
            no_early_wonder_guard: on("trainernoguard"),
            levels: if on("trainerlevels") { 20 } else { 0 },
        },
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
    });
    let rom = std::fs::read(from).expect("the ROM is readable");
    let seed = seed.parse().expect("the seed is a number");
    let out = gen5::randomize(&rom, &options, seed).expect("the ROM is Black 2 or White 2");
    std::fs::write(to, out).expect("the new ROM can be written");
}
