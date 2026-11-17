RELICS REFORGED - ELDEN RING NIGHTREIGN
=======================================

Got the effect you wanted on a relic with two useless ones? Take that one good effect off the
bad relic (the donor) and move it onto a relic that will actually use it. The donor and
3 sacrifices are destroyed and you pay murk. The result is always a relic the game itself could
have rolled: the mod never creates impossible relics, it just cuts down the RNG.
The forge speaks English and Spanish, following the game's language (see SETTINGS).

REQUIREMENTS
- Elden Ring Nightreign, game version 1.3.3 (the mod turns itself off on any other version).
- me3 0.13 or newer: https://me3.help (install it with its installer). ME3 Manager, the usual
  way to manage me3 mods, is optional.
- Optional: Seamless Co-op for Nightreign, to play with friends.

INSTALL
Relics Reforged is a DLL mod: RelicsReforged.dll and, next to it, the RelicsReforged folder (this
readme; the settings file and the log are created there). Keep them together. Pick one:

A. ME3 Manager (recommended): drag the downloaded zip into ME3 Manager, or install it from Nexus
   inside ME3 Manager, and launch the game from it.
   With Seamless Co-op: keep Seamless in your game folder and add Game\SeamlessCoop\nrsc.dll to
   ME3 Manager as an external mod, with load_early turned on in its advanced options (this is
   ME3 Manager's own recommendation for Seamless).

B. Your own me3 profile: unzip RelicsReforged.dll and the RelicsReforged folder into your mods
   folder and add the DLL to your .me3 file. For the profile me3 creates
   (%LOCALAPPDATA%\garyttierney\me3\config\profiles\nightreign-default.me3), unzip into the
   nightreign-mods folder next to it, open nightreign-default.me3 with a text editor and
   replace the line natives = [] with these lines (if you keep natives = [], me3 refuses the
   profile with "duplicate key"):
       savefile = "NR0000_RelicsReforged.sl2"

       [[natives]]
       path = 'nightreign-mods/RelicsReforged.dll'
   The savefile line keeps the mod off your normal save (see SAVES). With Seamless Co-op you
   can leave it out (Seamless uses its own save) and add one more block instead (use your own
   game path):
       [[natives]]
       path = 'C:/Program Files (x86)/Steam/steamapps/common/ELDEN RING NIGHTREIGN/Game/SeamlessCoop/nrsc.dll'
       load_early = true
   Any other .me3: add a [[natives]] block like the one above, and delete its natives = []
   line if it has one.

C. No other mods: download the optional "Standalone" file, unzip its folder anywhere (not inside
   the game folder) and double-click RelicsReforged.me3. It plays offline on a copy of your save.

Every player who wants to forge needs the mod; the others don't.
Other DLL loaders (like Elden Mod Loader) should work too, but I haven't tested them.

HOW TO USE
At the Roundtable Hold, open Relic Rites. With the cursor on the equipped relic slots, press F7
(keyboard) or R2/RT (controller), or click "F7: Forge relics". Pick the relic to improve and the
effect to replace, the donor and its effect, and 3 sacrifices (all of this can be changed, see
SETTINGS); check the preview and forge.
The donor can be a favorite or a relic in use (equipped, in a vessel or in a preset): the forge
warns you before confirming and leaves that slot empty. Sacrifices can't be favorites or in use.
Works with mouse, keyboard (arrows + Enter) and controller.
After a forge, leaving the forge closes Relic Rites: the game only updates its relic list when
the menu is opened again.

SETTINGS
The first time the game starts with the mod, it creates RelicsReforged\relics_reforged.ini
(next to the DLL). Edit it with any text editor and restart the game to change:
- the key (F1-F12, Insert, Delete, Home, End, PageUp, PageDown) and controller button (R2 or L2)
  that open the forge;
- the number of sacrifices (0-9), the minimum sacrifice points, the murk cost, the Deep murk
  multiplier and the forge limit per relic (0 = no limit);
- the menu language (auto, en, es) and how many save backups to keep (0 = none).
Updating the mod doesn't overwrite it. Delete it to get the defaults back. If a value is wrong,
the default is used and relics_reforged.log says so.

SAVES
The mod works on whatever save the game loads:
- Standalone profile (RelicsReforged.me3): a copy of your save (NR0000_RelicsReforged.sl2). Your
  normal save is never touched.
- With Seamless Co-op: the Seamless save (NR0000.co2 by default).
- Your own me3 profile without Seamless: your normal save (NR0000.sl2), unless the profile has a
  savefile line. To keep the modded game apart, add this to your .me3, above the first line
  that starts with [ (me3 copies your save to it the first time):
      savefile = "NR0000_RelicsReforged.sl2"
- Forging cannot be undone. Before the first forge of every game session, the mod copies your
  save to %APPDATA%\RelicsReforged\backups (the last 10 are kept). To restore one, close the
  game and copy it back to %APPDATA%\Nightreign\<your Steam ID>\ with the original name.
- me3 keeps the game offline. Do not use this mod online.

OTHER MODS (MORE MAP VARIATIONS AND SIMILAR)
Relics Reforged loads alongside other mods in the same me3 profile. The forge uses the relic
rules the game loaded, so mods that change relics through their own regulation.bin work: the
effects they add to the random pools can be forged too.
If a mod comes with its own .me3 file, add Relics Reforged to that file: copy RelicsReforged.dll
and the RelicsReforged folder next to it and add these lines at the end:
    [[natives]]
    path = 'RelicsReforged.dll'
I tested it offline with "More Map Variations and Weapons Mod Merge" 2.1.8.3; I haven't tested
it on that mod's own online server.

FILES
- RelicsReforged\relics_reforged.log (next to the DLL): written every time the game starts.
  Attach it to any bug report.
- %APPDATA%\RelicsReforged\relics_reforged_forjas.txt: which relics were forged (each relic has
  a forge limit). It stays there when you update or delete the mod.
- If the game updates and the mod doesn't support the new version yet, a notice shows up for the
  first minute and the mod stays off; the game runs normally.

KEEPING YOUR FORGED RELICS (GAME UPDATES AND UNINSTALLING)
Forged relics are stored in the save like any other relic: they don't need the mod to exist.
What matters is which save they are in (see SAVES).
- Game update not supported yet: keep playing the same way. The mod stays off (you can't
  forge), but the game loads the same save with all your forged relics.
- Seamless: your forged relics are in the Seamless save (NR0000.co2). Play Seamless without the
  mod and they are still there.
- On a save copy (the standalone profile, or a savefile line in your own profile): your forged
  relics are only in NR0000_RelicsReforged.sl2, which has moved on separately from your normal
  save (NR0000.sl2). To keep playing without the mod with everything you did on the copy:
    1. Close the game and open %APPDATA%\Nightreign\<your Steam ID>\.
    2. Back up NR0000.sl2 (copy it somewhere else).
    3. Copy NR0000_RelicsReforged.sl2 and rename the copy to NR0000.sl2, replacing the old one.
  This brings over all the progress of the copy, not only the relics; whatever you did only on
  your normal save is replaced (that's why step 2).
- Online: your normal save goes online. The relics are combinations the game itself can roll,
  but I can't promise the anti-cheat or the servers won't flag anything. If you take a save
  used with the mod online, it's at your own risk.

UNINSTALL
Remove the mod from ME3 Manager, or delete RelicsReforged.dll and the RelicsReforged folder (and
the [[natives]] lines you added to your .me3), or the standalone folder. Delete
%APPDATA%\RelicsReforged too if you don't want the forge record and the save backups. If you
played on a save copy, first read KEEPING YOUR FORGED RELICS.

Source code (MIT license): https://github.com/IgnacioBianchi-cmd/relics-reforged
