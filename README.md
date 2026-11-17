# Relics Reforged

**English:** an Elden Ring Nightreign mod that takes the one good effect off a bad relic and moves
it onto a relic that will use it, inside Relic Rites, always producing a relic the game itself
could have rolled. Download, install
instructions and the full description in English are on Nexus Mods
([`docs/nexus/descripcion.bbcode`](docs/nexus/descripcion.bbcode) has the same text); the end-user
readme is [`dist/README.txt`](dist/README.txt). The source code and its docs are in Spanish.
Released under the [MIT license](LICENSE).

---

Mod para **Elden Ring Nightreign** que permite transferir un efecto de una reliquia a otra,
para que un buen roll sirva aunque la reliquia entera no lo sea.

- Se elige un efecto de una reliquia **donante** y se sobrescribe un efecto de una **receptora**.
- La donante y **3 reliquias de sacrificio** se destruyen, y se paga **murk**.
- La reliquia resultante siempre es una combinación que **el juego podría haber generado**:
  el mod no crea reliquias imposibles, solo reduce cuánto RNG hace falta para llegar a una buena.

Pensado para jugar offline o con Seamless Co-op. **No usar online.**

## Estado

| Parte | Estado |
|---|---|
| Relevamiento del formato del save, params y memoria | ✔ ([docs/relevamiento.md](docs/relevamiento.md)) |
| Motor de reglas (`crates/relic-rules`) | ✔ con tests contra un save real |
| DLL in-game (`crates/relics-reforged`, me3) | Lee y valida el inventario. Gancho en el hilo principal del juego |
| Forja completa (`forge` desde el archivo de comandos) | ✔ probada en el juego y guardada en el save |
| Bloqueo de favoritas, equipadas, presets y recipientes como sacrificios | ✔ probado en el juego |
| Donante favorita o en uso: aviso al elegirla y al confirmar; al forjar se la saca de los slots equipados, presets y recipientes (quedan vacíos) | Formato de presets y slots vacíos relevado en memoria (2026-11-14); falta probar la forja en el juego |
| Menú dentro del juego (F7 en Relic Rites, ImGui con hudhook, a pantalla completa y con el estilo del juego) | ✔ probado: forja desde el menú guardada en el save |
| Íconos del juego (reliquias, efectos y teclas), leídos de los `data*.bdt` instalados | ✔ probado en el juego |
| Botón "F7 :Forge relics" en la barra de controles de Relic Rites | ✔ probado en inglés; posición en español medida con capturas, falta verla en el juego |
| Teclado y mouse del juego bloqueados con la forja abierta (DirectInput) | ✔ probado en el juego |
| Mouse del juego bloqueado (DirectInput del mouse y cursor por user32) | ✔ probado en el juego |
| Forja con teclado y mando (cursor propio; mando por Steam Input, XInput, DirectInput o HID, bloqueado para el juego con la forja abierta) | ✔ probado con teclado y con un mando de PlayStation por Steam Input |
| Reliquias de expediciones (tipos `100xxxx`, grupo 250) dentro de la forja | ✔ se venden en el juego (2026-11-11); la forja las acepta (tests) |
| Sonidos de menú del propio juego (cursor, confirmar, cancelar, abrir y cerrar) | ✔ probado en el juego |
| Búsqueda, filtro como el del juego (color, tamaño, tipo, favoritas) y orden | ✔ probado, con filtro por efecto (hasta 3 condiciones, categorías del juego) |
| Params desde `regulation.bin` y nombres desde los textos del juego (sin CSV) | ✔ probado en el juego |
| Solo en la Roundtable (con Relic Rites abierto) | ✔ probado en el juego |
| Seamless Co-op (perfil `me3/relics-reforged-seamless.me3`, con `load_early`) | ✔ probado: forja guardada en el save de Seamless. Sesión con un amigo en otra PC, los dos con el mod y con versiones distintas del mod, sin errores (2026-11-17). También con un jugador sin el mod: el que lo tenía forjó sin problemas |
| Datos del mod en `%APPDATA%\RelicsReforged`: registro de forjas (se mueve solo desde la carpeta de la DLL) y copia del save antes de la primera forja de cada sesión (últimas 10) | ✔ probado: registro movido, una copia por sesión |
| Aviso en pantalla si la versión del juego no es compatible, en un solo idioma: el de la configuración; si no, el del manifiesto de Steam (`appmanifest_2622380.acf`, con el que está instalado el juego); si no, el de Windows (`crates/relics-reforged/src/notice.rs`). En desarrollo se fuerza con `probar_aviso_version.txt` junto a la DLL | ✔ probado (forzado): sale en inglés según Steam (2026-11-17) |
| Configuración del jugador en `RelicsReforged\relics_reforged.ini` junto a la DLL (se crea con los valores por defecto si falta; el zip no lo trae, así una actualización no lo pisa): tecla y botón de apertura, reglas de balance, idioma y copias del save (`crates/relics-reforged/src/config.rs`) | ✔ probado en el juego (2026-11-17): multiplicador de murk de las Deep y sacrificios en 0, la forja respeta los dos |
| Publicación (`tools/package.ps1`) como los demás mods DLL de Nightreign (relevado en Nexus, me3 y ME3 Manager el 2026-11-17): zip principal con `RelicsReforged.dll` + `RelicsReforged\` en la raíz y sin `.me3` (ME3 Manager importa como perfil cualquier zip con uno), para ME3 Manager o el perfil propio del jugador (Seamless queda en `Game\SeamlessCoop` como mod externo con `load_early`); zip opcional standalone con `RelicsReforged.me3` (sin conexión, copia del save) | ✔ probado en ME3 Manager 1.5.2 con Seamless 1.1.3 (2026-11-17): el zip arrastrado se instala como mod DLL en su carpeta (`RelicsReforged-v0.1.0\`) y queda en el perfil; reinstalarlo pide sobrescribir y no rompe nada. Seamless como mod externo con `load_early`. La DLL crea ahí `RelicsReforged\` con el `.ini` y el log, y la forja se guardó en el save de Seamless con su respaldo (los demás saves intactos). También probados el standalone (doble clic en su `.me3`: forja sobre `NR0000_RelicsReforged.sl2`) y el zip en `nightreign-default.me3` con el acceso directo "NIGHTREIGN (me3)" (DLL en `nightreign-mods`, `RelicsReforged\` al lado). Ahí hay que **reemplazar** `natives = []` por el bloque `[[natives]]`: si se agrega sin borrarla, me3 rechaza el perfil con "duplicate key" |

## Estructura

| Ruta | Contenido |
|---|---|
| `crates/relic-rules/` | Motor de reglas en Rust: legalidad de reliquias y transferencias. Sin acceso al juego |
| `crates/relics-reforged/` | La DLL que carga me3. Escribe `relics_reforged.log` y `relics_reforged.ini` en la carpeta `RelicsReforged\` junto a la DLL, y en desarrollo acepta comandos de prueba en `relics_reforged_cmd.txt`, junto a la DLL |
| `crates/nr-assets/` | Lectura de archivos sueltos de los `data*.bhd/.bdt` del juego (RSA, AES, Oodle, TPF, BC7) para sacar los íconos del menú. `cargo run -p nr-assets --example icons` arma el atlas fuera del juego |
| `crates/re-tools/` | `disasm`: desensamblador del volcado de `nightreign.exe` (comando `dumpimage` de la DLL) |
| `me3/relics-reforged-dev.me3` | Perfil de me3 para probar, con un save aparte (`NR_RelicsReforged.sl2`). `relics-reforged-seamless.me3`: lo mismo con Seamless |
| `docs/relevamiento.md` | Todo lo averiguado: formato del save, params, punteros, reglas |
| `docs/nexus/` | Página de Nexus Mods: descripción en BBCode (`descripcion.bbcode`) y ficha con resumen, archivo, changelog, capturas y lo que falta antes de publicar (`ficha.md`) |
| `dist/` | Lo que va en la publicación además de la DLL: el `README.txt` para el usuario y, en `standalone/`, el perfil `RelicsReforged.me3` del archivo opcional (sin conexión y con una copia del save) |
| `tools/package.ps1` | Arma la publicación: compila la DLL pública en `target/public` y deja la carpeta y el zip en `target/dist` |
| `tools/` | Scripts de análisis (Python/PowerShell) usados en el relevamiento |
| `data/` | Datos locales, **no van a git**: CSV del juego, save del jugador, archivos de Cheat Engine |

## Desarrollo

Requisitos: Rust (toolchain `stable-x86_64-pc-windows-msvc`) y MSVC Build Tools.

```bash
cargo test -p relic-rules
```

Los tests de `tests/real_data.rs` usan datos reales si existen:
- `data/export/`: `AttachEffectParam.csv`, `AttachEffectTableParam.csv` y `EquipParamAntique.csv`, exportados con Smithbox (todas las filas y campos).
- `data/save/relics.json`: generado con `tools/decrypt_sl2.ps1` + `tools/relic_survey.py` desde un save.

Si faltan, esos tests se saltean y pasan.

Hay dos versiones de la DLL:

- **Pública** (la que se publica): solo la forja, el menú y su registro.

  ```bash
  cargo build --release -p relics-reforged
  ```

- **Desarrollo** (feature `dev`): suma el archivo de comandos `relics_reforged_cmd.txt` (lee y escribe memoria del juego), el rastreador, el perfilador, la búsqueda en memoria, el diagnóstico de entrada, la lectura de mandos por HID (sin confirmar), los nombres desde `data/export` y el inventario completo en el log. **No se publica.**

  ```bash
  cargo build --release -p relics-reforged --features dev
  ```

Las dos quedan en `target/release/relics_reforged.dll` (la última que se compiló). Para publicar: `powershell -File tools/package.ps1` (compila la pública aparte, en `target/public`, y arma en `target/dist` el zip principal `RelicsReforged-v<versión>.zip` y el opcional `RelicsReforged-Standalone-v<versión>.zip`; en la publicación la DLL se llama `RelicsReforged.dll`). Para probar, doble clic en `me3/relics-reforged-dev.me3`. El log queda en `target/release/RelicsReforged/relics_reforged.log`.

El menú y sus mensajes salen en inglés o en español según el idioma del juego (por defecto, inglés; ver `relic_rules::lang`). En el juego, con **Relic Rites** abierto en la Roundtable, **F7** (o un clic en "F7 :Forge relics") abre el menú a pantalla completa: a la izquierda receptora y efecto a reemplazar, donante y efecto a transferir, 3 sacrificios, vista previa y forja (con doble confirmación); a la derecha, la lista de reliquias del paso actual, con la imagen de cada reliquia y el ícono de cada efecto. Q (o F7, o Esc) vuelve a Relic Rites; R refresca y C empieza de nuevo. Arriba de la lista: búsqueda, 4 abre el filtro (en el panel izquierdo, con hasta 3 condiciones de efecto) y 5 cambia el orden. Con la forja abierta, el juego no recibe teclado, mouse ni mando. En la versión de desarrollo, para pruebas sin menú está el archivo de comandos (`crates/relics-reforged/src/commands.rs`).

Con teclado o mando, un cursor recorre botones y filas: arriba/abajo dentro de un panel, izquierda/derecha cambia de panel, Enter/Espacio o A elige. En el selector de efectos, A o derecha abren un grupo. Los controles muestran las teclas o los botones según lo último que se usó.

| Acción | Teclado | PlayStation | Xbox |
|---|---|---|---|
| Abrir / cerrar la forja (en Relic Rites) | F7 | R2 | RT |
| Elegir | Enter / Espacio | Cruz | A |
| Volver | Q o Esc | Círculo | B |
| Refrescar / Empezar de nuevo | R / C | Options / Triángulo | Menu / Y |
| Filtro / Orden | 4 / 5 | L3 / R3 | LS / RS |
| En el filtro: Limpiar / Quitar condiciones | 1 / 2 | Cuadrado / Triángulo | X / Y |
| Borrar la búsqueda | 2 | Cuadrado | X |

## Reglas de balance (por defecto)

Están en `Rules` (`crates/relic-rules/src/transfer.rs`). El jugador las cambia en `RelicsReforged\relics_reforged.ini` (sección `[Rules]`; ver `crates/relics-reforged/src/config.rs`, que también tiene el archivo por defecto).

| Regla | Valor |
|---|---|
| Sacrificios por transferencia | 3 (chica = 1 punto, mediana = 2, grande = 3) |
| Puntos mínimos (receptora de 1 / 2 / 3 efectos) | 3 / 5 / 7 |
| Murk (receptora de 1 / 2 / 3 efectos) | 2.000 / 4.000 / 7.000, ×1,5 si es Deep (3.000 / 6.000 / 10.500); el multiplicador acepta decimales |
| Forjas por reliquia (1 / 2 / 3 efectos); cuenta también reforjar un efecto ya forjado. 0 = sin límite | 1 / 1 / 2 |
