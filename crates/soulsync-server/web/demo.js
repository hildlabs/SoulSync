// Scripted run for /demo: feeds the dashboard the same state messages the
// server would send, so the page can be seen without a save file.
"use strict";
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
        snapshot: { trainer: { name: p.name, badges: p.badges, game: p.game }, party: p.mons.filter(m => m.battle), boxes: p.mons.filter(m => !m.battle) },
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
