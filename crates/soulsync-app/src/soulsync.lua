-- SoulSync live link for DeSmuME (Pokémon Black 2 / White 2).
--
-- Load it in DeSmuME under Tools > Lua Scripting > New Lua Script Window.
-- While it runs, it copies the party the game has in memory to a file that
-- the SoulSync app reads, so the dashboard follows without saving in-game.
-- SoulSync writes this file itself; changes are overwritten.

local OUT = [[@OUT@]]

local RAM_START, RAM_END = 0x02000000, 0x02400000
local MON, HEADER, SLOTS = 220, 8, 6
local LEN = HEADER + SLOTS * MON
-- Bytes of memory searched per frame while the party is not found.
local CHUNK = 0x10000

local floor = math.floor

local function xor16(a, b)
  local r, bit = 0, 1
  for _ = 1, 16 do
    if a % 2 ~= b % 2 then r = r + bit end
    a, b, bit = floor(a / 2), floor(b / 2), bit * 2
  end
  return r
end

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
    total = (total + xor16(memory.readword(addr + o), floor(seed / 65536))) % 65536
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

local cursor = RAM_START

-- Searches the next piece of memory. Returns the party's address if found.
local function search()
  local last = RAM_END - LEN
  local stop = math.min(cursor + CHUNK, last)
  for addr = cursor, stop - 4, 4 do
    local count = memory.readdword(addr + 4)
    if count >= 1 and count <= SLOTS and is_party(addr) then
      cursor = RAM_START
      return addr
    end
  end
  cursor = stop >= last and RAM_START or stop
end

local function read(addr)
  local parts = {}
  for o = 0, LEN - 1, 8 do
    parts[#parts + 1] = string.char(
      memory.readbyte(addr + o), memory.readbyte(addr + o + 1),
      memory.readbyte(addr + o + 2), memory.readbyte(addr + o + 3),
      memory.readbyte(addr + o + 4), memory.readbyte(addr + o + 5),
      memory.readbyte(addr + o + 6), memory.readbyte(addr + o + 7))
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

print("SoulSync: looking for your party ...")

local party, sent, sent_at, frame = nil, nil, 0, 0
while true do
  frame = frame + 1
  if not party then
    party = search()
    if party then print("SoulSync: connected, the dashboard follows your party live.") end
  elseif frame % 30 == 0 then
    if is_party(party) then
      local data, now = read(party), os.time()
      -- Written again every few seconds so the app can tell the script runs.
      if data ~= sent or now - sent_at >= 5 then
        if write(data) then sent, sent_at = data, now end
      end
    else
      party = nil
      print("SoulSync: party lost (reset or loading), looking again ...")
    end
  end
  emu.frameadvance()
end
