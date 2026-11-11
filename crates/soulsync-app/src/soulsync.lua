-- SoulSync live link for DeSmuME (Pokémon Black 2 / White 2).
--
-- Load it in DeSmuME under Tools > Lua Scripting > New Lua Script Window.
-- While it runs, it copies what the game has in memory to a file that the
-- SoulSync app reads, so the dashboard follows without saving in-game:
-- your party, where you are, and the Pokémon you meet in battles.
-- SoulSync writes this file itself; changes are overwritten.

local OUT = [[@OUT@]]
-- Written by the app: the ID of the trainer whose save it syncs.
local TRAINER = (OUT:gsub("[^\\/]+$", "live-trainer.txt"))

local RAM_START, RAM_END = 0x02000000, 0x02400000
local MON, HEADER, SLOTS = 220, 8, 6
-- Bytes copied from the party block on: the party, then the trainer and the
-- player's position, which follow it.
local WINDOW = 0x800
-- Bytes of memory searched per frame.
local PARTY_CHUNK, CHUNK = 0x10000, 0x4000
-- Most Pokémon from battles kept in the report, and most places watched.
local REPORTED, WATCHED = 8, 800

local floor = math.floor

local function xor16(a, b)
  local r, place = 0, 1
  for _ = 1, 16 do
    if a % 2 ~= b % 2 then r = r + place end
    a, b, place = floor(a / 2), floor(b / 2), place * 2
  end
  return r
end
-- DeSmuME brings its own, faster one.
local xor = (bit and bit.bxor) or XOR or xor16

-- The game's random number generator, which also encrypts Pokémon.
local function advance(seed)
  local lo, hi = seed % 65536, floor(seed / 65536)
  return (0x41C64E6D * lo + ((0x41C64E6D * hi) % 65536) * 65536 + 0x6073) % 4294967296
end

-- Whether a Pokémon with a correct checksum is stored at addr.
local function is_pokemon(addr)
  local sum = memory.readword(addr + 6)
  if sum == 0 and memory.readdword(addr) == 0 then return false end
  local seed, total = sum, 0
  for o = 8, 134, 2 do
    seed = advance(seed)
    total = (total + xor(memory.readword(addr + o), floor(seed / 65536))) % 65536
  end
  return total == sum
end

-- Whether a party block starts at addr: a count from 1 to 6 at addr + 4,
-- followed by that many Pokémon.
local function is_party(addr)
  local count = memory.readdword(addr + 4)
  if count < 1 or count > SLOTS then return false end
  for i = 0, count - 1 do
    if not is_pokemon(addr + HEADER + i * MON) then return false end
  end
  return true
end

local function read(addr, len)
  local parts = {}
  for o = 0, len - 1, 4 do
    parts[#parts + 1] = string.char(
      memory.readbyte(addr + o), memory.readbyte(addr + o + 1),
      memory.readbyte(addr + o + 2), memory.readbyte(addr + o + 3))
  end
  return table.concat(parts)
end

local function write(data)
  local file = io.open(OUT, "wb")
  if not file then return false end
  file:write(data)
  file:close()
  return true
end

-- The trainer the app syncs, if it said so: the ID, and the bytes of the
-- name as the game stores them.
local trainer_id, trainer_name, trainer_note = nil, {}, nil

-- Reads what the app wrote: the ID in the first line, the name's bytes in
-- hex in the second. Returns whether it changed.
local function read_trainer()
  local file = io.open(TRAINER, "r")
  local note = file and file:read("*a") or nil
  if file then file:close() end
  if note == trainer_note then return false end
  trainer_note = note
  local id, hex = (note or ""):match("^(%d+)%s*(%x*)")
  trainer_id, trainer_name = tonumber(id), {}
  for byte in (hex or ""):gmatch("%x%x") do
    trainer_name[#trainer_name + 1] = tonumber(byte, 16)
  end
  return true
end

local function is_trainer(addr)
  if memory.readdword(addr + 0x14) ~= trainer_id then return false end
  for i = 1, #trainer_name do
    if memory.readbyte(addr + 3 + i) ~= trainer_name[i] then return false end
  end
  return true
end

-- Whether the trainer's data follows the party block at addr, as it does in
-- the game's own save data. An opponent's party is laid out like the
-- player's; only this tells them apart. The app checks its reports the same
-- way and takes none that fails.
local function has_trainer(addr)
  if not trainer_id then return true end
  for t = HEADER + SLOTS * MON, WINDOW - 0x18 - 4, 4 do
    if is_trainer(addr + t) then return true end
  end
  return false
end

local party, party_cursor = nil, RAM_START

-- Searches the next piece of memory for the party block.
local function find_party()
  local last = RAM_END - WINDOW
  local stop = math.min(party_cursor + PARTY_CHUNK, last)
  for addr = party_cursor, stop - 4, 4 do
    local count = memory.readdword(addr + 4)
    if count >= 1 and count <= SLOTS and is_party(addr) and has_trainer(addr) then
      party_cursor = RAM_START
      return addr
    end
  end
  party_cursor = stop >= last and RAM_START or stop
end

-- Pokémon besides the party. The game keeps the opponent of a battle in
-- memory like one of yours, so every Pokémon that turns up while the script
-- runs is reported, and the app tells wild ones from the rest.
local known = {}    -- personality values seen so far
local watched = {}  -- addresses where a Pokémon was found, checked often
local watching = {}
-- The newest ones: where they are, and their bytes as last seen there.
local reported = {}
-- What is in memory when the script starts is old: a first pass over the
-- memory only takes note of it.
local cursor, settled = RAM_START, false

-- During a battle the game works on its own copy of each Pokémon and
-- leaves the one above as it was when the battle began. That copy starts
-- with a pointer to the Pokémon it was made from, and has the species 12
-- bytes on, then the most HP and the HP now.
local BATTLE_SPECIES, BATTLE_MAX, BATTLE_HP, BATTLE_LEVEL = 0x0C, 0x0E, 0x10, 0x18
-- Most battle copies followed per Pokémon; the game keeps two.
local COPIES = 4
-- Checks for which a Pokémon's battle copy is looked for, about a minute.
local HUNT = 120
-- Reported Pokémon still in memory, by personality value.
local facing, hunting = {}, false

-- The word that encrypts the n-th word of a Pokémon's battle stats.
local function stat_key(pid, n)
  local seed = pid
  for _ = 1, n do seed = advance(seed) end
  return floor(seed / 65536)
end

-- The HP the battle copy at addr has for the reported Pokémon r, or nil if
-- there is none there (any more).
local function battle_hp(addr, r)
  local from = memory.readdword(addr)
  if from < RAM_START or from >= RAM_END - MON or from % 4 ~= 0 then return nil end
  if memory.readdword(from) ~= r.pid then return nil end
  local most, hp = memory.readword(addr + BATTLE_MAX), memory.readword(addr + BATTLE_HP)
  local level = memory.readbyte(addr + BATTLE_LEVEL)
  if most == 0 or hp > most or level < 1 or level > 100 then return nil end
  -- The same most HP as the Pokémon it was made from.
  if most ~= xor(memory.readword(from + 0x90), stat_key(r.pid, 5)) then return nil end
  return hp
end

-- Whether addr is a battle copy of a reported Pokémon; then it is followed.
local function battle_copy(addr)
  local from = memory.readdword(addr)
  if from < RAM_START or from >= RAM_END - MON or from % 4 ~= 0 then return end
  local r = facing[memory.readdword(from)]
  if not r or #r.battle >= COPIES or not battle_hp(addr, r) then return end
  for i = 1, #r.battle do
    if r.battle[i] == addr then return end
  end
  r.battle[#r.battle + 1] = addr
  return true
end

-- The Pokémon's bytes with the HP of its battle copies in place of its own,
-- so that whoever reads them sees the battle as it stands.
local function with_battle_hp(r, data)
  local hp = nil
  for i = #r.battle, 1, -1 do
    local now = battle_hp(r.battle[i], r)
    if not now then
      table.remove(r.battle, i)
    elseif not hp or now < hp then
      hp = now
    end
  end
  if not hp then return data end
  local word = xor(hp, stat_key(r.pid, 4))
  return data:sub(1, 0x8E) .. string.char(word % 256, floor(word / 256)) .. data:sub(0x91)
end

local function notice(addr, pid)
  known[pid] = true
  if not watching[addr] and #watched < WATCHED then
    watching[addr] = true
    watched[#watched + 1] = addr
  end
  if settled then
    reported[#reported + 1] = { addr = addr, pid = pid, data = read(addr, MON), present = true, battle = {} }
    if #reported > REPORTED then table.remove(reported, 1) end
    return true
  end
end

-- A Pokémon's second field is zero and its checksum is not, which rules out
-- most of the memory cheaply.
local function new_pokemon(addr)
  local mid = memory.readdword(addr + 4)
  if mid == 0 or mid % 65536 ~= 0 then return end
  local pid = memory.readdword(addr)
  if pid ~= 0 and not known[pid] and is_pokemon(addr) then return pid end
end

local function sweep()
  local last = RAM_END - MON
  local stop = math.min(cursor + CHUNK, last)
  local found = false
  for addr = cursor, stop - 4, 4 do
    local pid = new_pokemon(addr)
    if pid and notice(addr, pid) then found = true end
    if hunting and battle_copy(addr) then found = true end
  end
  if stop >= last then
    cursor = RAM_START
    if not settled then
      settled = true
      print("SoulSync: connected, the dashboard follows your game live.")
    end
  else
    cursor = stop
  end
  return found
end

local function watch()
  local found = false
  for i = 1, #watched do
    local pid = new_pokemon(watched[i])
    if pid and notice(watched[i], pid) then found = true end
  end
  return found
end

-- Follows the reported Pokémon while they stay where they were found: a
-- wild Pokémon loses HP and gets a status during the battle.
local function refresh()
  facing, hunting = {}, false
  for i = 1, #reported do
    local r = reported[i]
    r.present = memory.readdword(r.addr) == r.pid and is_pokemon(r.addr)
    if r.present then
      r.data = with_battle_hp(r, read(r.addr, MON))
      facing[r.pid] = r
      -- Looked for only for a while: what is left over from a battle that
      -- is over has no copy any more.
      if #r.battle > 0 then
        r.hunted = 0
      elseif (r.hunted or 0) < HUNT then
        r.hunted = (r.hunted or 0) + 1
        hunting = true
      end
    else
      r.battle, r.hunted = {}, 0
    end
  end
end

local function report()
  local parts = { "SSL3", read(party, WINDOW) }
  for i = 1, #reported do
    parts[#parts + 1] = string.char(reported[i].present and 1 or 0) .. reported[i].data
  end
  return table.concat(parts)
end

local function know_party()
  local count = memory.readdword(party + 4)
  for i = 0, count - 1 do
    known[memory.readdword(party + HEADER + i * MON)] = true
  end
end

print("SoulSync: looking for your party ...")

-- Checks in a row at which the party did not read as one. While the game
-- rearranges it, as when Pokémon go to a box, it does not for a moment.
local MISSES = 20

local sent, sent_at, frame, changed, misses = nil, 0, 0, false, 0
read_trainer()
while true do
  frame = frame + 1
  if frame % 300 == 0 then
    if read_trainer() then
      party, party_cursor = nil, RAM_START
    end
  end
  if not party then
    party = find_party()
    if party then
      know_party()
      print("SoulSync: party found, taking stock of the memory ...")
    end
  else
    if sweep() then changed = true end
    if frame % 30 == 0 then
      if not has_trainer(party) then
        party, misses = nil, 0
        print("SoulSync: party lost (reset or loading), looking again ...")
      elseif is_party(party) then
        misses = 0
        know_party()
        if watch() then changed = true end
        refresh()
        local data, now = report(), os.time()
        -- Written again every few seconds so the app can tell the script runs.
        if changed or data ~= sent or now - sent_at >= 5 then
          if write(data) then sent, sent_at, changed = data, now, false end
        end
      else
        misses = misses + 1
        if misses >= MISSES then
          party, misses = nil, 0
          print("SoulSync: party lost (reset or loading), looking again ...")
        end
      end
    end
  end
  emu.frameadvance()
end
