RELICS REFORGED - ELDEN RING NIGHTREIGN
=======================================

Got the effect you wanted on a relic with two useless ones? Relics Reforged adds a forge to
Relic Rites: take that one good effect off the bad relic (the donor) and move it onto a relic
that will actually use it (the target). It costs murk and 3 sacrifice relics, and the result is
always a relic the game itself could have rolled.

REQUIREMENTS
- Elden Ring Nightreign 1.3.3. On any other game version the mod stays off and shows a notice
  for the first minute; the game runs normally.
- me3 0.13 or newer: https://me3.help

INSTALL
Keep RelicsReforged.dll and the RelicsReforged folder together (the settings file and the log
are created in that folder). Pick the option that matches how you play:

A. No mods yet: download the optional "Standalone" file, unzip it anywhere outside the game
   folder and double-click RelicsReforged.me3. It plays offline on a copy of your save.

B. With ME3 Manager: drag the downloaded zip into it and launch the game from there.

C. With a .me3 file (yours, or the one another mod comes with): put RelicsReforged.dll and the
   RelicsReforged folder next to the .me3 and add these lines at the end of it:
       [[natives]]
       path = 'RelicsReforged.dll'
   If the file has a natives = [] line, delete it, or me3 refuses to load it ("duplicate key").
   me3's own profile is %LOCALAPPDATA%\garyttierney\me3\config\profiles\nightreign-default.me3.

Other mods: Relics Reforged works alongside other mods, including ones that change relics: the
effects they add to the random pools can be forged too. Tested with More Map Variations and
Weapons Mod Merge 2.1.8.3.

Seamless Co-op: Seamless Co-op has to load first. In ME3 Manager, add
Game\SeamlessCoop\nrsc.dll as an external mod with load_early on (advanced options). In a .me3,
add another block with the path to your own nrsc.dll:
       [[natives]]
       path = 'C:/Program Files (x86)/Steam/steamapps/common/ELDEN RING NIGHTREIGN/Game/SeamlessCoop/nrsc.dll'
       load_early = true

Playing online with Seamless Co-op or on the More Map Variations server, only the players who
want to forge need the mod.

HOW TO USE
In Relic Rites, with the cursor on your equipped relic slots, press F7 (keyboard) or R2 / RT
(controller), or click "Forge relics" at the bottom.
1. Choose the target (the relic to improve) and the effect to replace.
2. Choose the donor and the effect to move. Normal relics forge with normal relics, Deep relics
   with Deep relics.
3. Choose 3 sacrifices of the same kind. Each one is worth 1, 2 or 3 points (Delicate, Polished,
   Grand), and the target needs at least 3 / 5 / 7 points if it has 1 / 2 / 3 effects.
4. Check the preview and forge. It costs 2,000 / 4,000 / 7,000 murk for a target with 1 / 2 / 3
   effects, x1.5 for Deep relics. Forging asks for confirmation twice.
Works with mouse, keyboard (arrows + Enter) and controller. After a forge, leaving the forge
closes Relic Rites, so the game updates its relic list.

RULES
- Each relic can be forged 1 / 1 / 2 times (1 / 2 / 3 effects). Forged effects are shown in gold.
- Curses travel with their effect, and a cursed effect can replace an uncursed one (or vice
  versa) only if the game also rolls that relic with that mix.
- Unique relics (such as those from the Small Jar Bazaar, the Collector Signboard, Remembrances
  and bosses) don't appear in the forge.
- Favorites and relics in use (equipped, in a vessel or in a preset) can't be sacrificed. They
  can still be donors: the forge warns you first, and forging removes them from there.

SETTINGS
RelicsReforged\relics_reforged.ini is created the first time the game starts with the mod. Edit
it and restart the game to change the key (F1-F12, Insert, Delete, Home, End, PageUp, PageDown)
and controller button (R2 / RT or L2 / LT) that open the forge, the number of sacrifices (0-9),
the minimum points, the murk cost, the Deep multiplier, the forge limit (0 = no limit), the menu
language (auto, en, es) and how many save backups to keep (0 = none).
Updates don't overwrite it; delete it to get the defaults back. A wrong value falls back to its
default, and the log says so.

SAVES AND SAFETY
The mod works on whatever save the game loads:
- Standalone profile: a copy of your save (NR0000_RelicsReforged.sl2). Your normal save is never
  touched.
- Seamless Co-op: the Seamless Co-op save (NR0000.co2 by default).
- Your own profile without Seamless Co-op: your normal save, unless you add
  savefile = "NR0000_RelicsReforged.sl2" to the .me3, above its first line that starts with [
  (recommended: me3 copies your save to it the first time).
- Backups: forging can't be undone, so before the first forge of every session your save is
  copied to %APPDATA%\RelicsReforged\backups (the last 10 are kept). To restore one, close the
  game and copy it back to %APPDATA%\Nightreign\<your Steam ID>\ with the original name.
- Never use it on FromSoftware's official servers. me3 keeps the game off them by default.
  Playing online through mods with their own connection, like Seamless Co-op or the More Map
  Variations server, doesn't touch the official servers.

KEEPING YOUR FORGED RELICS
Forged relics are stored in the save like any other relic and don't need the mod to exist.
- Unsupported game update: keep playing the same way. The mod stays off, but the save and your
  forged relics load as usual.
- Seamless Co-op: they are in the Seamless Co-op save; play Seamless Co-op without the mod and
  they are still there.
- On a save copy (the standalone profile, or a savefile line in your own profile): they are only
  in NR0000_RelicsReforged.sl2. To keep them without the mod:
    1. Close the game and open %APPDATA%\Nightreign\<your Steam ID>\.
    2. Back up NR0000.sl2 (copy it somewhere else).
    3. Copy NR0000_RelicsReforged.sl2 and rename the copy to NR0000.sl2, replacing the old one.
  This brings over all the progress of the copy, not only the relics.
- Taking a save used with the mod to the official servers is at your own risk: the relics are
  combinations the game can roll, but I can't promise nothing gets flagged.

FILES
- RelicsReforged\relics_reforged.log: written every time the game starts. Please attach it to
  any bug report.
- %APPDATA%\RelicsReforged\relics_reforged_forjas.txt: the forge record (for the forge limit).
  It stays there when you update or delete the mod.

UNINSTALL
Remove the mod from ME3 Manager, or delete RelicsReforged.dll and the RelicsReforged folder (and
the lines you added to your .me3), or the standalone folder. Delete %APPDATA%\RelicsReforged too
if you don't want the forge record and the save backups. If you played on a save copy, read
KEEPING YOUR FORGED RELICS first.

Source code (MIT license): https://github.com/IgnacioBianchi-cmd/relics-reforged
