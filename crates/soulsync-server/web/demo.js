// Scripted run for /demo: feeds the dashboard the same state messages the
// server would send, so the page can be seen without a save file.
"use strict";
// A few wild Pokémon for the catch calculator: [english, german, catch rate].
window.SoulSyncDemoSpecies = {
  from_rom: true,
  species: [
    ["Purrloin", "Felilou", 255], ["Patrat", "Nagelotz", 255], ["Sewaddle", "Strawickl", 255], ["Azurill", "Azurill", 150],
    ["Riolu", "Riolu", 75], ["Psyduck", "Enton", 190], ["Magnemite", "Magnetilo", 190], ["Growlithe", "Fukano", 190],
    ["Elekid", "Elekid", 45], ["Magby", "Magby", 45], ["Eevee", "Evoli", 45], ["Zorua", "Zorua", 75],
    ["Sandile", "Ganovil", 180], ["Trapinch", "Knacklion", 255], ["Drilbur", "Rotomurf", 120], ["Onix", "Onix", 45],
    ["Scyther", "Sichlor", 45], ["Larvitar", "Larvitar", 45], ["Axew", "Milza", 75], ["Metang", "Metang", 3],
    ["Cresselia", "Cresselia", 3], ["Volcarona", "Ramoth", 15], ["Rotom", "Rotom", 45], ["Togetic", "Togetic", 75],
  ],
};
// What a player's ROM would supply for the Pokémon card: a few moves, items
// and abilities, and one learnset every demo species shares.
window.SoulSyncDemoDex = (() => {
  const moves = [], abilities = [], items = [];
  const mv = (id, name, type, category, power, accuracy, pp, text) => { moves[id] = { name, text, type, category, power, accuracy, pp, priority: id === 98 ? 1 : 0 }; };
  mv(33, "Tackle", "normal", "physical", 50, 100, 35, "A physical attack in which the user charges and slams into the target.");
  mv(45, "Growl", "normal", "status", 0, 100, 40, "The user growls in an endearing way, lowering the foe's Attack.");
  mv(98, "Quick Attack", "normal", "physical", 40, 100, 30, "The user lunges at the target at a speed that makes it almost invisible. It is sure to strike first.");
  mv(44, "Bite", "dark", "physical", 60, 100, 25, "The target is bitten with viciously sharp fangs. It may make the target flinch.");
  mv(232, "Metal Claw", "steel", "physical", 50, 95, 35, "The target is raked with steel claws. It may also raise the user's Attack stat.");
  mv(93, "Confusion", "psychic", "special", 50, 100, 25, "The target is hit by a weak telekinetic force. It may also leave the target confused.");
  mv(36, "Take Down", "normal", "physical", 90, 85, 20, "A reckless, full-body charge attack. It also damages the user a little.");
  mv(334, "Iron Defense", "steel", "status", 0, 101, 15, "The user hardens its body's surface like iron, sharply raising its Defense stat.");
  abilities[29] = { name: "Clear Body", text: "Prevents other Pokémon from lowering its stats." };
  abilities[149] = { name: "Illusion", text: "Enters battle disguised as the last Pokémon in the party." };
  abilities[145] = { name: "Big Pecks", text: "Protects the Pokémon from Defense-lowering attacks." };
  items[234] = { name: "Leftovers", text: "An item to be held by a Pokémon. The holder's HP is gradually restored during battle." };
  items[155] = { name: "Oran Berry", text: "A Berry to be consumed by Pokémon. If a Pokémon holds one, it can restore its own HP by 10 points during battle." };
  const learnsets = Array.from({ length: 649 }, () => [[1, 33], [1, 45], [5, 98], [9, 44], [13, 232], [17, 93], [21, 36], [25, 334]]);
  return { moves, abilities, items, learnsets };
})();
window.SoulSyncDemo = function (apply) {
  const DEX = {
    375: ["Metang", "Metang", ["steel", "psychic"]], 570: ["Zorua", "Zorua", ["dark"]],
    519: ["Pidove", "Dusselgurr", ["normal", "flying"]], 551: ["Sandile", "Ganovil", ["ground", "dark"]],
    448: ["Lucario", "Lucario", ["fighting", "steel"]], 207: ["Gligar", "Skorgla", ["ground", "flying"]],
    176: ["Togetic", "Togetic", ["normal", "flying"]], 123: ["Scyther", "Sichlor", ["bug", "flying"]],
    479: ["Rotom", "Rotom", ["electric", "ghost"]], 246: ["Larvitar", "Larvitar", ["rock", "ground"]],
    215: ["Sneasel", "Sniebel", ["dark", "ice"]], 610: ["Axew", "Milza", ["dragon"]],
    58: ["Growlithe", "Fukano", ["fire"]], 179: ["Mareep", "Voltilamm", ["electric"]],
    328: ["Trapinch", "Knacklion", ["ground"]],
  };
  const LOC = {
    1: ["Aspertia City", "Eventura City"], 2: ["Floccesy Town", "Dausing"], 3: ["Route 19", "Route 19"],
    4: ["Floccesy Ranch", "Dausing-Hof"], 5: ["Route 20", "Route 20"], 6: ["Virbank City", "Virbank City"],
    7: ["Castelia Sewers", "Stratos-Kanalisation"], 8: ["Route 4", "Route 4"],
  };
  const names = a => ({ en: a[0], de: a[1] });
  const players = [["Hannes", "black2"], ["Mika", "white2"]].map(([name, game]) => ({ name, game, joined: false, badges: 0, mons: [], at: null }));
  const log = [];
  let pid = 1, attempt = 3, timer = null;

  function emit(who) {
    if (who) who.at = Date.now();
    const joined = players.filter(p => p.joined);
    const links = Object.keys(LOC).map(Number).map(location => ({
      location, location_name: names(LOC[location]),
      sides: joined.map(p => {
        const m = p.mons.find(x => x.met_location === location);
        return m ? { pokemon: m, status: !m.battle ? "boxed" : m.battle.hp === 0 ? "fainted" : "party" } : null;
      }),
    })).filter(l => l.sides.some(Boolean));
    apply(JSON.parse(JSON.stringify({
      code: "DEMO-0000", seed: 4821, attempt, settings: { caps: [13, 18, 24, 30, 33, 39, 48, 51] },
      players: joined.map(p => ({
        name: p.name, connected: true, updated_at: p.at,
        snapshot: { trainer: { name: p.name, badges: p.badges, game: p.game }, party: p.mons.filter(m => m.battle), boxes: p.mons.filter(m => !m.battle), wild: p.wild || null },
      })),
      links, log: log.slice(0, 30),
    })));
  }
  const say = (p, event) => log.unshift({ at: Date.now(), player: p.name, event });
  const find = (p, species) => p.mons.find(m => m.species === species);

  function caught(p, species, loc, nick, level, boxed) {
    const max = 12 + level * 2 + (species % 5);
    const m = {
      pid: pid++, species, species_name: names(DEX[species]), types: DEX[species][2], nickname: nick || "", is_nicknamed: !!nick,
      is_egg: false, met_location: loc, met_location_name: names(LOC[loc]), met_level: level,
      place: boxed ? { kind: "box", box: 1, slot: 0 } : { kind: "party", slot: 0 },
      battle: boxed ? null : { level, hp: max, max_hp: max },
      moves: window.SoulSyncDemoDex.learnsets[0].filter(([l]) => l <= level).slice(-4).map(([, id]) => ({ id, pp: window.SoulSyncDemoDex.moves[id].pp - (species % 4), pp_ups: 0 })),
      item: [234, 155, 0][species % 3], ability: [29, 149, 145][species % 3], nature: species % 25,
      stats: boxed ? null : { attack: 8 + level * 2, defense: 7 + level * 2, speed: 9 + level * 2, sp_attack: 6 + level * 2, sp_defense: 7 + level * 2 },
    };
    p.joined = true; p.mons.push(m); say(p, { type: "caught", pokemon: m }); emit(p);
  }
  function level(p, species, by) {
    const m = find(p, species), b = m.battle;
    b.level += by; b.max_hp += 2 * by; b.hp = Math.min(b.max_hp, b.hp + 2 * by);
    say(p, { type: "level_up", pokemon: m }); emit(p);
  }
  function hurt(p, species, ratio) {
    const b = find(p, species).battle; b.hp = Math.round(b.max_hp * ratio);
    if (b.hp === 0) say(p, { type: "fainted", pokemon: find(p, species) });
    emit(p);
  }
  function toBox(p, species) {
    const m = find(p, species); m.battle = null; m.place = { kind: "box", box: 1, slot: 0 };
    say(p, { type: "moved", pokemon: m }); emit(p);
  }
  // A wild battle on a player's card: species, HP and status as they change.
  function battle(p, species, level, ratio, ailment) {
    if (species == null) { p.wild = null; emit(p); return; }
    const max = 12 + level * 2;
    p.wild = { pid: 9000 + species, species, species_name: names(DEX[species]), types: DEX[species][2], nickname: "", is_nicknamed: false, is_egg: false, met_location: 0, met_level: level, place: { kind: "party", slot: 0 }, battle: { level, hp: Math.round(max * ratio), max_hp: max }, ailment: ailment || null };
    emit(p);
  }
  function badge(p) { p.badges++; say(p, { type: "badges", count: p.badges }); emit(p); }

  const [H, M] = players;
  const steps = [
    [300, () => caught(H, 375, 1, "Bolt", 5)],
    [1800, () => caught(M, 570, 1, "Kitsune", 5)],
    [1500, () => caught(H, 519, 2, "Taube", 4)],
    [1100, () => caught(M, 551, 2, "", 4)],
    [1300, () => caught(M, 207, 3, "Batsy", 6)],
    [1100, () => caught(H, 448, 3, "", 6)],
    [1300, () => caught(H, 176, 4, "Pip", 7)],
    [1100, () => caught(M, 123, 4, "", 7)],
    [1400, () => level(H, 375, 4)], [500, () => level(M, 570, 4)], [500, () => level(H, 448, 3)],
    [500, () => level(M, 207, 3)], [500, () => hurt(M, 207, .55)], [600, () => level(H, 176, 2)],
    [500, () => level(M, 123, 3)], [700, () => hurt(H, 519, .6)], [600, () => level(H, 375, 3)], [500, () => level(M, 570, 3)],
    [1500, () => badge(H)], [900, () => badge(M)],
    [1200, () => battle(M, 246, 11, 1)], [1400, () => battle(M, 246, 11, .45)], [1400, () => battle(M, 246, 11, .45, "paralysis")], [1600, () => battle(M, null)],
    [1500, () => caught(H, 479, 5, "", 11)],
    [1100, () => caught(M, 246, 5, "Pebble", 11)],
    [1300, () => caught(H, 215, 6, "", 12)],
    [1100, () => caught(M, 610, 6, "Fang", 12)],
    [1500, () => toBox(H, 215)], [900, () => toBox(M, 610)],
    [1300, () => level(H, 375, 3)], [450, () => level(M, 570, 3)], [450, () => level(H, 448, 5)], [450, () => level(M, 123, 5)],
    [450, () => hurt(M, 207, .26)], [600, () => level(H, 176, 5)], [450, () => level(M, 246, 3)], [450, () => level(H, 479, 3)],
    [1400, () => caught(H, 58, 7, "", 14)],
    [1100, () => caught(M, 179, 7, "Wolle", 14)],
    [1500, () => caught(H, 328, 8, "", 13, true)],
    [1800, () => hurt(H, 519, .3)], [900, () => hurt(H, 519, .1)], [1100, () => hurt(H, 519, 0)],
    [2500, () => badge(H)], [900, () => badge(M)],
  ];
  let i = 0;
  function idle() {
    const p = Math.random() < .5 ? H : M;
    const alive = p.mons.filter(m => m.battle && m.battle.hp > 0);
    const m = alive[Math.floor(Math.random() * alive.length)];
    const r = Math.random();
    if (r < .3 && m.battle.level < 24) level(p, m.species, 1);
    else hurt(p, m.species, r < .65 ? .35 + Math.random() * .5 : 1);
    timer = setTimeout(idle, 1800 + Math.random() * 1600);
  }
  function next() {
    if (i >= steps.length) { timer = setTimeout(idle, 2500); return; }
    const [wait, fn] = steps[i++];
    timer = setTimeout(() => { fn(); next(); }, wait);
  }
  // "Start next attempt" in the demo: count up and play the run again.
  window.SoulSyncDemoRestart = () => {
    clearTimeout(timer); attempt++; i = 0; log.length = 0;
    players.forEach(p => { p.mons = []; p.badges = 0; p.at = null; });
    emit(); next();
  };
  next();
};
