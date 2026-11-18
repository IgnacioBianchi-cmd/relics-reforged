RELICS REFORGED - ELDEN RING NIGHTREIGN
=======================================

Moves the one good effect from a bad relic onto a relic that will use it, in Relic Rites.
Full description: https://www.nexusmods.com/eldenringnightreign/mods/974

REQUIREMENTS
- Elden Ring Nightreign 1.3.3 (on other versions the mod stays off; the game runs normally).
- me3 0.13 or newer: https://me3.help

INSTALL
Keep RelicsReforged.dll and the RelicsReforged folder together.
- No mods yet: use the "Standalone" file. Unzip it anywhere outside the game folder and
  double-click RelicsReforged.me3. It plays offline on a copy of your save.
- ME3 Manager: drag the zip into it and launch the game from there.
- A .me3 file: put the DLL and the folder next to it and add at the end:
      [[natives]]
      path = 'RelicsReforged.dll'
  Delete its natives = [] line if it has one.
- With Seamless Co-op: Seamless has to load first. In ME3 Manager, add
  Game\SeamlessCoop\nrsc.dll as an external mod with load_early on (advanced options). In a
  .me3, add this block with the path to your own nrsc.dll:
      [[natives]]
      path = 'C:/Program Files (x86)/Steam/steamapps/common/ELDEN RING NIGHTREIGN/Game/SeamlessCoop/nrsc.dll'
      load_early = true

HOW TO OPEN IT
In Relic Rites, with the cursor on your equipped relic slots, press F7 or R2 / RT.

SETTINGS
RelicsReforged\relics_reforged.ini (created on the first start): key and button, sacrifices,
costs, forge limit, language and backups. Restart the game after editing it. Delete it to get
the defaults back.

SAVES
- Forging can't be undone. Before the first forge of every session your save is copied to
  %APPDATA%\RelicsReforged\backups. To restore one, close the game and copy it back to
  %APPDATA%\Nightreign\<your Steam ID>\ with the original name.
- With your own .me3 the mod works on your normal save (NR0000.sl2). To keep it apart, add
  savefile = "NR0000_RelicsReforged.sl2" above the first line that starts with [ (me3 copies
  your save to it the first time). With Seamless Co-op it's the Seamless save (NR0000.co2).
- The Standalone profile plays on NR0000_RelicsReforged.sl2, a copy of your save: forged relics
  stay there. To use them without the mod, back up NR0000.sl2 and replace it with a copy of
  NR0000_RelicsReforged.sl2 renamed to NR0000.sl2 (this brings over all the progress of the
  copy).
- Never use it on FromSoftware's official servers. Seamless Co-op and mods with their own
  servers don't use them.

BUGS
Attach RelicsReforged\relics_reforged.log (next to the DLL) to your report.

UNINSTALL
Remove it from ME3 Manager, or delete RelicsReforged.dll, the RelicsReforged folder and the
lines you added to your .me3. Delete %APPDATA%\RelicsReforged too if you don't want the forge
record and the backups. Forged relics stay in your save.

Source code (MIT license): https://github.com/IgnacioBianchi-cmd/relics-reforged
