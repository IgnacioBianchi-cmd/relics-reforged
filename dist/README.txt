RELICS REFORGED - ELDEN RING NIGHTREIGN
=======================================

(Español más abajo)

Got the effect you wanted on a relic with two useless ones? Take that one good effect off the
bad relic (the donor) and move it onto a relic that will actually use it. The donor and
3 sacrifices are destroyed and you pay murk. The result is always a relic the game itself could
have rolled: the mod never creates impossible relics, it just cuts down the RNG.

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
Other DLL loaders (like Elden Mod Loader) should work too, but haven't been tested.

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
Tested offline with "More Map Variations and Weapons Mod Merge" 2.1.8.3; it hasn't been tested
on that mod's own online server.

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
  but we can't promise the anti-cheat or the servers won't flag anything. If you take a save
  used with the mod online, it's at your own risk.

UNINSTALL
Remove the mod from ME3 Manager, or delete RelicsReforged.dll and the RelicsReforged folder (and
the [[natives]] lines you added to your .me3), or the standalone folder. Delete
%APPDATA%\RelicsReforged too if you don't want the forge record and the save backups. If you
played on a save copy, first read KEEPING YOUR FORGED RELICS.

Source code (MIT license): https://github.com/IgnacioBianchi-cmd/relics-reforged


-----------------------------------------------------------------------------------------------
ESPAÑOL
-----------------------------------------------------------------------------------------------

¿Te salió el efecto que buscabas en una reliquia con otros dos que no sirven? Sacale ese efecto
bueno a la reliquia mala (la donante) y pasáselo a una que sí lo aproveche. La donante y 3
sacrificios se destruyen y pagás murk. El resultado siempre es una reliquia que el juego podría
haber generado: el mod no crea reliquias imposibles, solo reduce cuánto azar hace falta.

REQUISITOS
- Elden Ring Nightreign, versión 1.3.3 (en otra versión el mod se desactiva solo).
- me3 0.13 o más nuevo: https://me3.help (se instala con su instalador). ME3 Manager, la forma
  habitual de manejar mods de me3, es opcional.
- Opcional: Seamless Co-op para Nightreign, para jugar con amigos.

INSTALACIÓN
Relics Reforged es un mod DLL: RelicsReforged.dll y, al lado, la carpeta RelicsReforged (este
readme; ahí se crean la configuración y el registro). Dejalos juntos. Elegí una opción:

A. ME3 Manager (recomendado): arrastrá el zip descargado a ME3 Manager, o instalalo desde Nexus
   dentro de ME3 Manager, y arrancá el juego desde ahí.
   Con Seamless Co-op: dejá Seamless en la carpeta del juego y agregá
   Game\SeamlessCoop\nrsc.dll a ME3 Manager como mod externo, con load_early activado en sus
   opciones avanzadas (es lo que recomienda ME3 Manager para Seamless).

B. Tu propio perfil de me3: descomprimí RelicsReforged.dll y la carpeta RelicsReforged en tu
   carpeta de mods y agregá la DLL a tu .me3. Para el perfil que crea me3
   (%LOCALAPPDATA%\garyttierney\me3\config\profiles\nightreign-default.me3), descomprimí en la
   carpeta nightreign-mods que está al lado, abrí nightreign-default.me3 con un editor de texto
   y reemplazá la línea natives = [] por estas (si dejás natives = [], me3 rechaza el perfil
   con "duplicate key"):
       savefile = "NR0000_RelicsReforged.sl2"

       [[natives]]
       path = 'nightreign-mods/RelicsReforged.dll'
   La línea savefile hace que el mod no trabaje sobre tu save normal (ver SAVES). Con Seamless
   Co-op podés no ponerla (Seamless usa su propio save) y agregar otro bloque (con la ruta de tu
   juego):
       [[natives]]
       path = 'C:/Program Files (x86)/Steam/steamapps/common/ELDEN RING NIGHTREIGN/Game/SeamlessCoop/nrsc.dll'
       load_early = true
   En cualquier otro .me3: agregá un bloque [[natives]] como el de arriba y borrá su línea
   natives = [] si la tiene.

C. Sin otros mods: descargá el archivo opcional "Standalone", descomprimí su carpeta donde
   quieras (no dentro de la carpeta del juego) y hacé doble clic en RelicsReforged.me3. Juega
   sin conexión sobre una copia de tu save.

Cada jugador que quiera forjar necesita el mod; los demás no.
Otros loaders de DLLs (como Elden Mod Loader) deberían funcionar, pero no se probaron.

CÓMO SE USA
En la Mesa Redonda, abrí Ritos de reliquias. Con el cursor en los espacios de reliquias
equipadas, apretá F7 (teclado) o R2/RT (mando), o hacé clic en "F7: Forjar reliquias". Elegí la
reliquia a mejorar y el efecto a reemplazar, la donante y su efecto, y 3 sacrificios (todo
esto se puede cambiar, ver CONFIGURACIÓN); revisá la vista previa y forjá. Funciona con
mouse, teclado (flechas + Enter) y mando.
La donante puede ser favorita o estar en uso (equipada, en un recipiente o en un preajuste): la
forja avisa antes de confirmar y deja ese espacio vacío. Los sacrificios no pueden ser favoritos
ni estar en uso.
Después de una forja, al salir de la forja se cierra Ritos de reliquias: el juego solo
actualiza su lista de reliquias al volver a abrir el menú.

CONFIGURACIÓN
La primera vez que arranca el juego con el mod, se crea RelicsReforged\relics_reforged.ini
(junto a la DLL). Editalo con cualquier editor de texto y reiniciá el juego para cambiar:
- la tecla (F1-F12, Insert, Delete, Home, End, PageUp, PageDown) y el botón del mando (R2 o L2)
  que abren la forja;
- la cantidad de sacrificios (0-9), los puntos mínimos de sacrificio, el costo en murk, el
  multiplicador de murk de las Deep y el límite de forjas por reliquia (0 = sin límite);
- el idioma del menú (auto, en, es) y cuántas copias del save se guardan (0 = ninguna).
Actualizar el mod no lo pisa. Borralo para volver a los valores por defecto. Si un valor está
mal, se usa el de por defecto y relics_reforged.log lo avisa.

SAVES
El mod trabaja sobre el save que cargue el juego:
- Perfil standalone (RelicsReforged.me3): una copia de tu save (NR0000_RelicsReforged.sl2). Tu
  save normal no se toca.
- Con Seamless Co-op: el save de Seamless (NR0000.co2 por defecto).
- Tu propio perfil de me3 sin Seamless: tu save normal (NR0000.sl2), salvo que el perfil tenga
  una línea savefile. Para separar el juego con mods, agregá esto a tu .me3, antes de la primera
  línea que empieza con [ (la primera vez, me3 copia tu save a ese archivo):
      savefile = "NR0000_RelicsReforged.sl2"
- La forja no se puede deshacer. Antes de la primera forja de cada sesión, el mod copia tu save
  a %APPDATA%\RelicsReforged\backups (guarda las últimas 10). Para restaurar una, cerrá el juego
  y copiala a %APPDATA%\Nightreign\<tu ID de Steam>\ con el nombre original.
- me3 mantiene el juego sin conexión. No uses este mod online.

OTROS MODS (MORE MAP VARIATIONS Y PARECIDOS)
Relics Reforged se carga junto con otros mods en el mismo perfil de me3. La forja usa las reglas
de reliquias que cargó el juego, así que funciona con mods que cambian las reliquias con su
propio regulation.bin: los efectos que agregan a los pools aleatorios también se pueden forjar.
Si un mod trae su propio .me3, agregá Relics Reforged a ese archivo: copiá RelicsReforged.dll y
la carpeta RelicsReforged junto a él y agregá al final:
    [[natives]]
    path = 'RelicsReforged.dll'
Probado sin conexión con "More Map Variations and Weapons Mod Merge" 2.1.8.3; no se probó en el
servidor online propio de ese mod.

ARCHIVOS
- RelicsReforged\relics_reforged.log (junto a la DLL): se escribe cada vez que arranca el juego.
  Adjuntalo si reportás un error.
- %APPDATA%\RelicsReforged\relics_reforged_forjas.txt: qué reliquias se forjaron (cada reliquia
  tiene un límite de forjas). Queda ahí aunque actualices o borres el mod.
- Si el juego se actualiza y el mod todavía no es compatible con la versión nueva, aparece un
  aviso durante el primer minuto y el mod queda apagado; el juego funciona normal.

CONSERVAR LAS RELIQUIAS FORJADAS (ACTUALIZACIONES Y DESINSTALAR)
Las reliquias forjadas quedan en el save como cualquier otra reliquia: no necesitan el mod para
existir. Lo que importa es en qué save están (ver SAVES).
- El juego se actualizó y el mod todavía no es compatible: seguí jugando igual. El mod queda
  apagado (no se puede forjar), pero el juego carga el mismo save con todas tus reliquias
  forjadas.
- Seamless: tus reliquias forjadas están en el save de Seamless (NR0000.co2). Jugá Seamless sin
  el mod y siguen ahí.
- En una copia del save (el perfil standalone, o una línea savefile en tu propio perfil): tus
  reliquias forjadas están solo en NR0000_RelicsReforged.sl2, que avanzó por separado de tu save
  normal (NR0000.sl2). Para seguir jugando sin el mod con todo lo que hiciste en la copia:
    1. Cerrá el juego y abrí %APPDATA%\Nightreign\<tu ID de Steam>\.
    2. Respaldá NR0000.sl2 (copialo a otro lado).
    3. Copiá NR0000_RelicsReforged.sl2 y renombrá la copia a NR0000.sl2, reemplazando el viejo.
  Así pasa todo el progreso de la copia, no solo las reliquias; lo que hayas hecho solo en tu
  save normal se reemplaza (por eso el paso 2).
- Online: tu save normal es el que va online. Las reliquias son combinaciones que el juego
  puede generar, pero no podemos asegurar que el anticheat o los servidores no detecten nada.
  Llevar online un save usado con el mod corre por tu cuenta.

DESINSTALAR
Quitá el mod de ME3 Manager, o borrá RelicsReforged.dll y la carpeta RelicsReforged (y las líneas
[[natives]] que agregaste a tu .me3), o la carpeta del standalone. Borrá también
%APPDATA%\RelicsReforged si no querés el registro de forjas ni las copias del save. Si jugaste
sobre una copia del save, leé antes CONSERVAR LAS RELIQUIAS FORJADAS.

Código fuente (licencia MIT): https://github.com/IgnacioBianchi-cmd/relics-reforged
