RELICS REFORGED - ELDEN RING NIGHTREIGN
=======================================

(Español más abajo)

Move one effect from a relic you don't need onto a relic you want to keep. The donor relic and
3 sacrifices are destroyed and you pay murk. The result is always a relic the game itself could
have rolled: the mod never creates impossible relics, it just cuts down the RNG.

REQUIREMENTS
- Elden Ring Nightreign, game version 1.3.3 (the mod turns itself off on any other version).
- me3 0.13 or newer: https://me3.help (install it with its installer).
- Optional: Seamless Co-op for Nightreign, to play with friends.

INSTALL
1. Unzip the "RelicsReforged" folder anywhere (not inside the game folder).
2. Without Seamless: double-click RelicsReforged.me3.
   With Seamless: copy the "SeamlessCoop" folder from your game folder (Game\SeamlessCoop, the one
   with nrsc.dll and nrsc_settings.ini) into the RelicsReforged folder, then double-click
   RelicsReforged-Seamless.me3. Every player who wants to forge needs the mod; the others don't.
   If the SeamlessCoop folder is missing, the game starts without Seamless (on the mod's save
   copy, never on your normal save).

HOW TO USE
At the Roundtable Hold, open Relic Rites. With the cursor on the equipped relic slots, press F7
(keyboard) or R2/RT (controller), or click "F7: Forge relics". Pick the relic to improve and the
effect to replace, the donor and its effect, and 3 sacrifices (all of this can be changed, see
SETTINGS); check the preview and forge.
Works with mouse, keyboard (arrows + Enter) and controller.
After a forge, leaving the forge closes Relic Rites: the game only updates its relic list when
the menu is opened again.

SETTINGS
The first time the game starts with the mod, it creates relics_reforged.ini in the mod folder.
Edit it with any text editor and restart the game to change:
- the key (F1-F12, Insert, Delete, Home, End, PageUp, PageDown) and controller button (R2 or L2)
  that open the forge;
- the number of sacrifices (0-9), the minimum sacrifice points, the murk cost, the Deep murk
  multiplier and the forge limit per relic (0 = no limit);
- the menu language (auto, en, es) and how many save backups to keep (0 = none).
Updating the mod doesn't overwrite it. Delete it to get the defaults back. If a value is wrong,
the default is used and relics_reforged.log says so.

SAVES
- RelicsReforged.me3 works on a copy of your save (NR0000_RelicsReforged.sl2). Your normal save
  is never touched.
- With Seamless, the mod works on the Seamless save (NR0000.co2 by default).
- Forging cannot be undone. Before the first forge of every game session, the mod copies your
  save to %APPDATA%\RelicsReforged\backups (the last 10 are kept). To restore one, close the
  game and copy it back to %APPDATA%\Nightreign\<your Steam ID>\ with the original name.
- me3 keeps the game offline. Do not use this mod online.

OTHER MODS (MORE MAP VARIATIONS AND SIMILAR)
The forge uses the relic rules the game loaded, so mods that change relics through their own
regulation.bin work: the effects they add to the random pools can be forged too. me3 runs one
profile at a time, so to play both, copy relics_reforged.dll next to the other mod's .me3 file
and add these lines at the end of that .me3:
    [[natives]]
    path = 'relics_reforged.dll'
The mod then works on whatever save that profile uses. Tested offline with "More Map Variations
and Weapons Mod Merge" 2.1.8.3; it hasn't been tested on that mod's own online server.

FILES
- %APPDATA%\RelicsReforged\relics_reforged_forjas.txt: which relics were forged (each relic has
  a forge limit). It stays there when you update or delete the mod.
- relics_reforged.log (in the mod folder): written every time the game starts. Attach it to any
  bug report.
- If the game updates and the mod doesn't support the new version yet, a notice shows up for the
  first minute and the mod stays off; the game runs normally.

UNINSTALL
Delete the RelicsReforged folder (and %APPDATA%\RelicsReforged if you don't want the forge
record and the save backups).

Source code (MIT license): https://github.com/IgnacioBianchi-cmd/relics-reforged


-----------------------------------------------------------------------------------------------
ESPAÑOL
-----------------------------------------------------------------------------------------------

Pasá un efecto de una reliquia que no te sirve a una que querés conservar. La donante y 3
sacrificios se destruyen y pagás murk. El resultado siempre es una reliquia que el juego podría
haber generado: el mod no crea reliquias imposibles, solo reduce cuánto azar hace falta.

REQUISITOS
- Elden Ring Nightreign, versión 1.3.3 (en otra versión el mod se desactiva solo).
- me3 0.13 o más nuevo: https://me3.help (se instala con su instalador).
- Opcional: Seamless Co-op para Nightreign, para jugar con amigos.

INSTALACIÓN
1. Descomprimí la carpeta "RelicsReforged" donde quieras (no dentro de la carpeta del juego).
2. Sin Seamless: doble clic en RelicsReforged.me3.
   Con Seamless: copiá la carpeta "SeamlessCoop" de la carpeta del juego (Game\SeamlessCoop, la
   que tiene nrsc.dll y nrsc_settings.ini) dentro de la carpeta RelicsReforged y hacé doble clic
   en RelicsReforged-Seamless.me3. Cada jugador que quiera forjar necesita el mod; los demás no.
   Si falta la carpeta SeamlessCoop, el juego arranca sin Seamless (con la copia del save del
   mod, nunca con tu save normal).

CÓMO SE USA
En la Mesa Redonda, abrí Ritos de reliquias. Con el cursor en los espacios de reliquias
equipadas, apretá F7 (teclado) o R2/RT (mando), o hacé clic en "F7: Forjar reliquias". Elegí la
reliquia a mejorar y el efecto a reemplazar, la donante y su efecto, y 3 sacrificios (todo
esto se puede cambiar, ver CONFIGURACIÓN); revisá la vista previa y forjá. Funciona con
mouse, teclado (flechas + Enter) y mando.
Después de una forja, al salir de la forja se cierra Ritos de reliquias: el juego solo
actualiza su lista de reliquias al volver a abrir el menú.

CONFIGURACIÓN
La primera vez que arranca el juego con el mod, se crea relics_reforged.ini en la carpeta del
mod. Editalo con cualquier editor de texto y reiniciá el juego para cambiar:
- la tecla (F1-F12, Insert, Delete, Home, End, PageUp, PageDown) y el botón del mando (R2 o L2)
  que abren la forja;
- la cantidad de sacrificios (0-9), los puntos mínimos de sacrificio, el costo en murk, el
  multiplicador de murk de las Deep y el límite de forjas por reliquia (0 = sin límite);
- el idioma del menú (auto, en, es) y cuántas copias del save se guardan (0 = ninguna).
Actualizar el mod no lo pisa. Borralo para volver a los valores por defecto. Si un valor está
mal, se usa el de por defecto y relics_reforged.log lo avisa.

SAVES
- RelicsReforged.me3 trabaja sobre una copia de tu save (NR0000_RelicsReforged.sl2). Tu save
  normal no se toca.
- Con Seamless, el mod trabaja sobre el save de Seamless (NR0000.co2 por defecto).
- La forja no se puede deshacer. Antes de la primera forja de cada sesión, el mod copia tu save
  a %APPDATA%\RelicsReforged\backups (guarda las últimas 10). Para restaurar una, cerrá el juego
  y copiala a %APPDATA%\Nightreign\<tu ID de Steam>\ con el nombre original.
- me3 mantiene el juego sin conexión. No uses este mod online.

OTROS MODS (MORE MAP VARIATIONS Y PARECIDOS)
La forja usa las reglas de reliquias que cargó el juego, así que funciona con mods que cambian
las reliquias con su propio regulation.bin: los efectos que agregan a los pools aleatorios
también se pueden forjar. me3 corre un perfil por vez; para jugar con los dos, copiá
relics_reforged.dll junto al .me3 del otro mod y agregá al final de ese .me3:
    [[natives]]
    path = 'relics_reforged.dll'
El mod trabaja entonces sobre el save que use ese perfil. Probado sin conexión con "More Map
Variations and Weapons Mod Merge" 2.1.8.3; no se probó en el servidor online propio de ese mod.

ARCHIVOS
- %APPDATA%\RelicsReforged\relics_reforged_forjas.txt: qué reliquias se forjaron (cada reliquia
  tiene un límite de forjas). Queda ahí aunque actualices o borres el mod.
- relics_reforged.log (en la carpeta del mod): se escribe cada vez que arranca el juego.
  Adjuntalo si reportás un error.
- Si el juego se actualiza y el mod todavía no es compatible con la versión nueva, aparece un
  aviso durante el primer minuto y el mod queda apagado; el juego funciona normal.

DESINSTALAR
Borrá la carpeta RelicsReforged (y %APPDATA%\RelicsReforged si no querés el registro de forjas
ni las copias del save).

Código fuente (licencia MIT): https://github.com/IgnacioBianchi-cmd/relics-reforged
