# Relics Reforged — Relevamiento (fase 0)

Fecha: 2026-11-03 · Juego: `nightreign.exe` del 2026-07-02 · regulation.bin del 2026-03-31

## Entorno

| Cosa | Estado |
|---|---|
| Juego | `B:\SteamLibrary\steamapps\common\ELDEN RING NIGHTREIGN\Game` |
| Save | `%APPDATA%\Nightreign\<steamid>\NR0000.sl2` (19,5 MB, 14 entradas BND4) |
| Seamless Co-op | No está instalado en la carpeta del juego |
| Cheat Engine | Instalado |
| Smithbox 2.2.6 | `herramientas\Smithbox-2.2.6` (hash verificado). Proyecto en `smithbox-project` |
| Python 3.13, git, Node, .NET, Java | Instalados |
| Rust 1.99.1 (MSVC) + Build Tools 2022, me3 v0.13.0 | Instalados (2026-11-03) |
| Ghidra, x64dbg, ReClass.NET | **No instalados** |

## Estructura de la carpeta

| Ruta | Contenido |
|---|---|
| `docs/` | Este documento |
| `tools/` | Scripts del relevamiento (ver abajo) |
| `data/export/` | CSV exportados de Smithbox: `AttachEffectParam`, `AttachEffectTableParam`, `EquipParamAntique` |
| `data/save/relics.json` | Las reliquias de tu save, decodificadas |
| `herramientas/`, `smithbox-project/` | Smithbox y su proyecto (no van a git) |

| Script | Para qué |
|---|---|
| `tools/decrypt_sl2.ps1` | Desencripta un `.sl2` usando solo .NET |
| `tools/relic_survey.py` | Extrae las reliquias de `USERDATA_00` → `relics.json` |
| `tools/validate_relics.py` | Valida reliquias contra las tablas de los params |
| `tools/find_sort_key.py` | Encontró la regla de orden de los efectos |
| `tools/pool_analysis.py` | Clasifica reliquias aleatorias y fijas y compara pools |
| `tools/order_check.py` | Análisis inicial del orden (superado por `find_sort_key.py`) |

## Formato del save (verificado)

- `.sl2` = contenedor BND4; cada entrada va cifrada con AES-128-CBC (IV = primeros 16 bytes).
- Las reliquias viven en `USERDATA_00`, en registros de 80 bytes intercalados con registros chicos de otros ítems.

| Offset | Tipo | Contenido |
|---|---|---|
| +0 | u32 | ID único de la reliquia (byte alto 0xC0) |
| +4, +8 | u32 | item_id \| 0x80000000 (= fila de `EquipParamAntique`; repetido, siempre igual) |
| +12 | u32 | 0xFFFFFFFF |
| +16 | 4 × u32 | **Efectos** (0xFFFFFFFF = vacío). El 4º nunca se usa |
| +32 | 24 bytes | Casi constante; en +36 hay un valor distinto en 2 reliquias (significado desconocido) |
| +56 | 4 × u32 | **Maldiciones, emparejadas por índice con su efecto** |
| +72 | 8 bytes | Siempre cero |

### Identidad de una reliquia entre sesiones

- **El ID `0xC08000NN` NO es estable**: al reescribirse el save, las 144 reliquias que quedaron cambiaron de ID (parece un índice de slot, no un identificador permanente).
- **El `sort_key` (u16 en el índice de orden, ver `relic_survey.py`) SÍ es estable**: se conservó en las 144.
  → La marca de "forjada" se indexa por `sort_key` (verificando también item y efectos).

### Murk en el save

- Es un u32 en `USERDATA_00`, rodeado por bytes fijos:
  `00 00 05 00 00 00 00 02 3D 01 00 00 00 00 00 00 [MURK u32] 00 00 00 00 00 00 00 00 FF FF 00 00`
- Su offset cambia (0x11998 → 0x10E18) porque el inventario de reliquias, que va antes, se compacta al vender.

### Sesión de Cheat Engine 1 (2026-11-03)

- Juego 1.3.3.0 lanzado con me3 (`nightreign-default`); me3 fuerza `Menu.IsEnableOnlineMode=false`.
- Murk 10140 → 10840 → 11190: 5 direcciones. Cuatro son copias que la UI reescribe seguido:
  - `mov [rdi+13D8],eax` (8142 escrituras)
  - `mov [rbx+2FC],edi` (6501)
  - `mov [rbx+2F8],eax` (44)
  - `mov [rcx],rax` (44, en la misma estructura que las dos anteriores, a +0x174)
- La 5ª estaba en otra zona de memoria (candidata a ser la real). **Poner un breakpoint de escritura ahí hizo crashear el juego**: `0xC0000005` en un módulo desconocido, consistente con el anti-debug del juego contra el VEH de Cheat Engine.
  → Seguir **sin debugger**: búsqueda por patrones de bytes y pointer scan.

### Sesión de Cheat Engine 2 (2026-11-03): datos vivos y punteros

- Después de cargar la partida, en memoria hay **3 copias** del murk y de cada reliquia:
  - **A = dato vivo.** El murk y las reliquias están en estructuras separadas y la reliquia siguiente no es contigua.
  - **B y C = buffers del save.** Mismo formato que el archivo (reliquias contiguas, distancia reliquia→murk ≈ la del archivo). Se liberan o reutilizan en algún momento.
- **El registro de la reliquia en A tiene el mismo layout que en el save**: efectos en +0x10, maldiciones en +0x38.
- Vender una reliquia cambia el murk A en el momento.
- Pointer scan con 2 sesiones comparadas (offset máx. 8192, nivel 5): **227 caminos**. Los más cortos:

| Camino al murk (u32) | Nota |
|---|---|
| `[nightreign.exe+3C078D0] + 0x530` | 1 nivel |
| `[[nightreign.exe+3C078D0] + 0x8] + 0xD0` | **Candidato principal** (¿GameDataMan → PlayerGameData?) |
| `[[nightreign.exe+3C24238] + 0x148] + 0xD0` | |
| `[[[nightreign.exe+3C2A768] + 0xAF0] + 0xA8] + 0xD0` | |

#### Tabla de ítems (equivalente al `CSGaItem` de Elden Ring), verificada

Pointer scan de la reliquia (2 sesiones, offset máx. 65536, nivel 4): 323 caminos. La mayoría sale de `nightreign.exe+3C131C8` y todos se reducen a:

```
objeto(h)   = [ [nightreign.exe+3C131C8] + 0x8 + (h & 0xFFFF) * 8 ]
registro(h) = objeto(h) + 0x8        ← mismo layout de 80 bytes que en el save
```

- `h` = ID de la reliquia (`0xC08000C8` → índice 0xC8).
- Verificado en vivo: índice 0xC8 → `0xC08000C8` e índice 0xC9 → `0xC08000C9`.
- `objeto + 0x0` = vtable dentro de `nightreign.exe` (`7FF61B0A21D0` en la sesión 2).
- Los objetos están contiguos, cada uno ocupa **0x100 bytes**.

#### Sesión 3: verificación tras reiniciar ✔

- Los 3 caminos del murk resuelven a la dirección nueva (`7FF49A03BF30`) con el valor correcto (11190).
- `[[nightreign.exe+3C131C8]+648]+8` → `0xC08000C8` y `+650` → `0xC08000C9`, en direcciones nuevas (`7FF4988C8208`, `…8308`).
- **Los IDs `0xC08000NN` se mantienen dentro de una sesión y al recargar el mismo save.** Cambian cuando el juego reescribe el save (compactación tras vender). La marca de "forjada" sigue yendo por `sort_key`.
- La vtable de los objetos de ítem no cambió entre sesiones (`7FF61B0A21D0`). Igual la DLL tiene que trabajar siempre relativa a la base del módulo.
- **Conclusión:** la DLL puede leer el murk y recorrer el inventario de reliquias con dos punteros estáticos. Fin de la ingeniería inversa necesaria para *leer*.

#### DLL v0 en el juego (2026-11-04) ✔

Primera prueba de `relics_reforged.dll` cargada con `me3/relics-reforged-dev.me3` (save aparte `NR_RelicsReforged.sl2`):

- Detecta 1.3.3.0 por `TimeDateStamp`, carga los params de `data/export` (2079 efectos, 1397 tipos, 1079 tablas) y espera unos 35 s a que la partida esté estable.
- Lee **murk 11190** y **147 reliquias**. Las 147 coinciden **por contenido** (item, efectos y maldiciones en orden) con el save: el `NR_RelicsReforged.sl2` desencriptado tiene exactamente 147 registros.
- Resumen del motor de reglas sobre los datos vivos: **58 aleatorias legales (51 normales + 7 Deep), 0 ilegales, 89 fijas, 0 de tipo desconocido**.
- Las reliquias ocupan los índices **84 a 230 de la tabla de ítems, contiguos**. Los índices 0–83 son otros ítems (no pasan el filtro de forma).

## Escritura (2026-11-04, todo probado en el juego con el save aparte)

Herramientas: los comandos de desarrollo de la DLL (`relics_reforged_cmd.txt`, ver `crates/relics-reforged/src/commands.rs`), el rastreador de escrituras (`tracer.rs`: `PAGE_GUARD` + manejador vectorizado, sin debugger; aguantó una hora con 2 M de lecturas sin problemas) y el desensamblador del volcado (`crates/re-tools`, binario `disasm`).

### Objeto de ítem de una reliquia (0x100 bytes)

| Offset | Contenido |
|---|---|
| +0x00 | vtable (`nr+2C321D0`) |
| +0x08 | registro de 80 bytes, igual al del save |
| +0x5C | lista de hasta 18 **SpEffects** (`passiveSpEffectId_1..3` de cada efecto en orden de slot, después los de cada maldición, sin quitar repetidos) |
| +0xA4 | cantidad de SpEffects de la lista |

- La lista de SpEffects **no está en el save**: el juego la arma al cargar. Es lo que se aplica en la expedición; la interfaz muestra los efectos del registro.
- Regla verificada contra 9 objetos (test `lista_de_sp_effects_coincide_con_el_juego`).

### Murk y efectos: escritura directa ✔

- Escribir el murk en `PlayerGameData + 0xD0` y los efectos/maldiciones/SpEffects en el objeto funciona: la interfaz lo muestra y **el save lo guarda** en el próximo guardado.
- Al recargar, el juego recalcula la lista de SpEffects desde el registro (verificado).

### Borrar una reliquia: `RemoveItem` del juego ✔

Rastreo de una venta: lo único que se escribe en el objeto es su destrucción (los ítems tienen **conteo de referencias**; el objeto se libera al soltar la última). La cadena que lo suelta es la del inventario:

| Función | Qué hace |
|---|---|
| `nr+7E91A0` | vender desde el menú (llama a la siguiente y después suma el murk) |
| `nr+7EAB30` | quitar N de la selección del menú (`[+0x50]` índice, `[+0x54]` handle) |
| **`nr+1F06B0`** | **`RemoveItem(EquipGameData*, índice, delta, flag)`**: la que usa el mod, con delta -1 y flag 0 |
| `nr+1F3E90` | resta la cantidad (`nr+1F4FF0`); si llega a 0 desequipa (`nr+1F1540`) y borra la entrada (`nr+1F6410`) |
| `nr+754050` → `nr+6C1E60` → `nr+69FAF0` | suelta la referencia del handle; en 0 destruye y libera el objeto |
| `nr+7EA470` | quitar N por **ID de ítem** (borra la primera entrada de ese tipo: **no sirve** para reliquias) |
| `nr+7EABC0` | devuelve EquipGameData = `[[nr+3C078D0]+8] + 0x1D8` |

Inventario = EquipGameData + 0x1C8 (= PlayerGameData + 0x3A0):

| Campo | Contenido |
|---|---|
| `[+0x1C]` | cantidad de índices que van al array de `[+0x40]`; el resto va al de `[+0x30]` (índice − `[+0x1C]`) |
| `[+0x70]` | último índice válido |
| entrada (20 bytes) | +0 handle (0 = vacía), +4 item\|0x80000000, +8 cantidad, +0xC número de orden, +0x10 flag |

Resultado: la reliquia desaparece de la tabla de ítems y del inventario, el menú muestra el contador correcto y, **después de un guardado real** (volver al título), el save tampoco la tiene. Un guardado que el juego hizo poco después del borrado todavía la tenía: el juego guarda una foto armada antes. Como la forja cambia todo en un mismo frame, cualquier guardado posterior la toma entera o no la toma: **no hace falta pedir un guardado**.

### Hilo principal ✔

Perfilador por muestreo (`profiler.rs`, comando `profile`): el hilo que arranca en el punto de entrada (`nr+26916BC`) es el principal, y es el que lee el inventario en cada frame.

| Dirección | Qué es |
|---|---|
| `nr+DCB340` | ciclo principal (WinMain) |
| `nr+DCB37F` | `call nr+F2F640`, una vez por frame: **acá va el gancho** |
| `nr+F2F640` | un frame entero (sin argumentos; incrementa el contador `nr+3C2C24C`) |

`mainthread.rs` cambia los 4 bytes del desplazamiento de esa llamada por un código propio, reservado a menos de 2 GB, que guarda rcx/rdx/r8/r9/xmm0-3, corre la cola de trabajos del mod y salta a `nr+F2F640`. Probado: 60 frames por segundo, estable.

### Guardado: lo que NO hay que llamar ⚠

Rastreo de lecturas (`watchr`) al volver al título: el save se arma en `nr+69F560` (un ítem: handle, ID y 0x48 bytes del registro) ← `nr+20D210` ← `nr+2014A0` (datos de partida desde PlayerGameData) ← `nr+6B97D0` (serializar en un buffer; GameMan = `[nr+3C13258]`).

**`nr+6AF930` NO es "guardar ya".** Es "foto en memoria / restaurar foto": si `GameMan+0xDA8` está vacío, serializa la partida ahí; si ya hay una foto, **la carga** (`nr+6B24A0` lee desde el buffer). La llamamos el 2026-11-04 y cargó un personaje por defecto (Wylder, tutoriales, cinemática del DLC, 9.999.999 de runas) encima del save de prueba, y el guardado siguiente dejó el resumen del perfil (USERDATA_10) en blanco. Se recuperó con un respaldo. No usar.

### Forja completa ✔ (2026-11-04)

`forge C08000B9 3 C08000C9 3 C08000E0 C08000AA C0800054`: la Grand Burning Scene pasó de Giants' Flame · Fire Attack +1 · Frost Resistance a Giants' Flame · **Cooldown Reduction +3** · Fire Attack +1 (el juego reordena solo); 145 → 141 reliquias; murk 11.840 → 1.840. Después de volver al título, el save tiene exactamente eso en la sección principal y el perfil intacto. El save conserva además una copia vieja del bloque de reliquias en otra zona de USERDATA_00, que el juego ignora al cargar.

### Reliquias bloqueadas (favoritas, equipadas, presets, recipientes) ✔

Encontrado con `snap` (diferencias de memoria antes y después de cada acción en el menú), `find` y `ptrs`.

| Qué | Dónde |
|---|---|
| Favorita | byte bajo de `+0x10` de la entrada de inventario (los bytes altos cambian entre sesiones: no usarlos) |
| Equipada ahora | `pgd+0x2F4`: 6 handles del recipiente equipado (`+0x374`: sus tipos de reliquia) |
| Contenedor de recipientes y presets | `[pgd+0x6F8]` → `+0x00` asignador (`nr+39B8008`), `+0x08` cabeza de un `std::map`, `+0x10` cantidad de recipientes; `+0x18` asignador, `+0x28` cantidad de presets, `+0x30…` presets de 0x50 bytes |
| Nodo del `std::map` (0x40) | izquierda, padre, derecha, color, esNulo (`+0x19`), ID del recipiente (`+0x1C`), 6 handles desde `+0x24` |
| Preset (0x50) | recipiente, 6 handles, FILETIME, marcas y nombre en UTF-16 |

- **Trampa:** en dos sesiones el contenedor quedó en memoria justo después de PlayerGameData (`pgd+0x168E0`) y parecía parte de él. No lo es: siempre se llega por `[pgd+0x6F8]`.
- **Trampa:** desde `pgd+0x8FC` hay una lista con **todas** las reliquias (registros de 20 bytes; ¿orden de adquisición o "nueva"?). No indica uso: no se mira.
- **ASLR:** la base de `nightreign.exe` cambió por primera vez (de `0x7FF618470000` a `0x7FF6E63A0000`). Todo se calcula relativo a la base, así que no afecta, pero las búsquedas manuales tienen que usar la base del log.
- Regla de `forge`: la donante y los sacrificios no pueden ser favoritos ni aparecer en esos lugares; la receptora sí. Si el contenedor no tiene la forma esperada o el árbol no tiene la cantidad de nodos que dice, la reliquia se da por bloqueada.
- Probado el 2026-11-04: 66 de 141 reliquias bloqueadas; rechazos por favorita, por preset + recipiente y por recipiente solo.
- En el save, recipientes y presets están en USERDATA_00 con otro formato (recipiente: ID + 6 handles, 0x1C bytes; desde ~`0x1c090`).

### Saves de prueba

- Respaldos en `data/save/backups/` (no van a git). Para restaurar: con el juego cerrado, copiar sobre `NR_RelicsReforged.sl2` y `.bak`, y **ponerles la fecha actual** (la carpeta tiene `steam_autocloud.vdf`).
- Un save puede tener las reliquias bien y el perfil roto: verificar las 14 secciones, no solo USERDATA_00.

Direcciones de referencia: sesión 1, murk A `7FF3B2F3BF30` y reliquia A `7FF3B17C8208`; sesión 2, murk A `7FF43F72F380` y reliquia A `7FF43E948208` (Deep Polished Tranquil Scene).
Archivos: `data/ce/sesion1.scandata`, `data/ce/murk_comparado.PTR`.

## Params (verificados con los CSV exportados)

| Param | Filas | Campos que usa el mod |
|---|---|---|
| `EquipParamAntique` | 1397 | `isDeepRelic`, `isSalable`, `relicColor`, `attachEffectTableId_1..3`, `attachEffectTableId_curse1..3` |
| `AttachEffectTableParam` | 22 088 entradas en 1081 tablas | `ID` (= tabla), `attachEffectId`, `chanceWeight`, `chanceWeight_dlc` |
| `AttachEffectParam` | 2079 | `compatibilityId`, `exclusivityId`, `overrideEffectId`, `allow<Personaje>`, `isDebuff` |

### Hallazgos confirmados

1. **Legalidad:** las 114 reliquias aleatorias de tu save tienen cada efecto en alguna tabla de slot de su tipo (100%).
2. **Orden de los efectos:** el juego tira un efecto por tabla y después los **ordena por (`overrideEffectId` ↑, ID ↑)**. Se cumple en las 153 reliquias con 2+ efectos, sin excepciones. La maldición se mueve con su efecto. Smithbox llama a ese campo "Override SpEffect ID", pero funciona como prioridad de orden (valores 1, 2, 3, 10, 100, 1000).
3. **Normales aleatorias:** los 3 slots tienen **el mismo pool** (290 o 340 efectos según la variante: tablas 300/200/100, 310/210/110, 100x00x). Ninguna normal tiene tablas de maldición.
4. **Fijas:** 49 de las tuyas son `Relic` pero fijas (Bazaar): una sola entrada por tabla y `isSalable = 0`. Además existen unas pocas Deep **vendibles pero fijas** (tablas 6040300, 707043600, etc.).
   → Criterio de elegibilidad: **todas las tablas de slot tienen más de un efecto** (no alcanza con `isSalable`).
5. **Deep, tres pools que no se pisan:**
   - `2000000`: 49 efectos potenciados, **siempre** con una maldición de `3000000` (24 maldiciones, no hay opción "sin maldición").
   - `2200000`: 283 efectos sin maldición.
   - `2100000`: otro pool sin maldición (sin reliquias en tu save; probablemente del DLC).
   - Combinaciones de slots que existen: desde `2000000 ×3` (3 maldiciones posibles) hasta `2200000 ×3` (ninguna), con todas las mezclas intermedias.

### Respuesta a "¿puede terminar con 3 negativos?"

Solo si el **tipo** de reliquia ya trae 3 slots potenciados (`2000000 ×3`), es decir, si el juego también podría haberla generado así. Un efecto potenciado solo entra en un slot `2000000` y siempre trae su maldición. Por eso **la cantidad de maldiciones nunca supera la cantidad de slots potenciados del tipo de reliquia**.

## Reglas de la transferencia (versión actual)

1. **Elegibles:** reliquias aleatorias (todas sus tablas de slot con más de un efecto). Nunca fijas ni `UniqueRelic`.
2. Normal con normal, Deep con Deep.
3. **El efecto viaja con su maldición** y pisa el efecto y la maldición del slot destino.
4. **Legalidad:** después de la transferencia tiene que existir una asignación 1 a 1 de efectos a las tablas de slot de la receptora, y cada efecto de `2000000` tiene que llevar una maldición de `3000000`.
5. **Exclusividad:** no puede haber dos efectos con el mismo `compatibilityId` (cubre duplicados de nivel, Attack Power, exclusivos de personaje, afinidad y skill).
6. **Orden:** se reescriben los efectos ordenados por (`overrideEffectId`, ID), cada maldición en el índice de su efecto.
7. Límite de slots forjados: 3 efectos → 2 · 2 efectos → 1 · 1 efecto → 1. Reforjar un slot ya forjado está permitido (se paga de nuevo). Como el orden cambia, el mod rastrea los efectos forjados por ID de efecto y no por posición.
8. Una reliquia forjada puede donar.
9. Costo: donante + 3 sacrificios + murk.
   - Puntos por sacrificio: chica 1, mediana 2, grande 3. Mínimo según destino: 1 ef → 3, 2 ef → 5, 3 ef → 7.
   - Si el destino es Deep, los sacrificios son Deep.
   - Murk: 2.500 / 5.000 / 10.000 según el tamaño del destino, ×2 si es Deep (a ajustar jugando).
10. No se pueden usar favoritas ni reliquias equipadas en un preset.
11. Solo en la Roundtable. La marca de "forjada" se guarda en un archivo propio del mod, indexado por el `sort_key` de la reliquia (el ID `0xC08000NN` no sirve porque cambia entre sesiones).

## Pendientes

- [x] Params identificados y exportados.
- [x] Regla de legalidad validada contra el save.
- [x] Regla de orden de los efectos.
- [ ] **Byte +36**: qué significa (¿favorita? ¿nueva?). Se prueba marcando o desmarcando una reliquia y comparando saves.
- [x] **Memoria**: murk y tabla de ítems encontrados con punteros estáticos, verificados en 3 sesiones.
- [x] **Escritura**: murk y efectos por escritura directa; borrado con `RemoveItem` del juego. Probado y guardado en el save.
- [x] **Hilo del juego**: gancho en `nr+DCB37F`; murk, set, remove y forge corren entre frames.
- [x] ~~Pedir un guardado~~: no hace falta (la forja es atómica respecto de los guardados). `nr+6AF930` no sirve: restaura una foto.
- [x] Comando `forge` (y `forge?` en seco) con `plan_transfer`, probado y guardado en el save.
- [x] **Favoritas, equipadas, presets y recipientes**: bloqueadas en `forge` (ver arriba).
- [x] Interfaz dentro del juego: menú F7 (`ui.rs`, Dear ImGui con hudhook 0.9.3 sobre DirectX 12). La fuente de ImGui cubre Latin-1: nada de "→" ni otros símbolos fuera de ese rango.
- [ ] **Solo en la Roundtable** (regla 11): falta detectar dónde está el jugador; hoy el menú forja en cualquier lado.
- [ ] **Params desde memoria**: hoy la DLL lee `data/export/*.csv`, que no se pueden repartir. Para pasarle el mod a otros hay que armar `GameData` desde los params en memoria.
- [ ] Seamless Co-op: probar que la forja funcione con el mod de co-op cargado.
- [ ] **Cuántas entradas tiene la tabla de ítems** y cómo se ve una entrada vacía. Por ahora la DLL recorre hasta 0x2000 entradas y filtra por la forma del registro; encuentra todas las reliquias, pero el largo real sigue sin confirmar.
- [x] Instalar Rust y me3. Falta Seamless Co-op.
- [x] Motor de reglas: `crates/relic-rules`, testeado contra `relics.json` y los CSV.
- [x] DLL v0 de solo lectura: murk e inventario desde memoria, validados en el juego.
- [ ] DLL: armar `GameData` desde los params en memoria, sin depender de los CSV.
