# Relics Reforged

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
| Bloqueo de favoritas, equipadas, presets y recipientes | ✔ probado en el juego |
| Menú dentro del juego (F7 en Relic Rites, ImGui con hudhook, a pantalla completa y con el estilo del juego) | ✔ probado: forja desde el menú guardada en el save |
| Íconos del juego (reliquias, efectos y teclas), leídos de los `data*.bdt` instalados | ✔ probado en el juego |
| Botón "F7 :Forge relics" en la barra de controles de Relic Rites | ✔ probado en inglés; posición en español medida con capturas, falta verla en el juego |
| Teclado y mouse del juego bloqueados con la forja abierta (DirectInput) | ✔ probado en el juego |
| Mouse del juego bloqueado (DirectInput del mouse y cursor por user32) | ✔ probado en el juego |
| Forja con teclado y mando (cursor propio; mando por Steam Input, XInput, DirectInput o HID, bloqueado para el juego con la forja abierta) | ✔ probado con teclado y con un mando de PlayStation por Steam Input |
| Reliquias únicas del Small Jar Bazaar fuera de la forja | ✔ probado en el juego |
| Sonidos de menú del propio juego (cursor, confirmar, cancelar, abrir y cerrar) | ✔ probado en el juego |
| Búsqueda, filtro como el del juego (color, tamaño, tipo, favoritas) y orden | ✔ probado, con filtro por efecto (hasta 3 condiciones, categorías del juego) |
| Params desde `regulation.bin` y nombres desde los textos del juego (sin CSV) | ✔ probado en el juego |
| Solo en la Roundtable (con Relic Rites abierto) | ✔ probado en el juego |
| Seamless Co-op (perfil `me3/relics-reforged-seamless.me3`, con `load_early`) | ✔ probado: forja guardada en el save de Seamless |

## Estructura

| Ruta | Contenido |
|---|---|
| `crates/relic-rules/` | Motor de reglas en Rust: legalidad de reliquias y transferencias. Sin acceso al juego |
| `crates/relics-reforged/` | La DLL que carga me3. Escribe `relics_reforged.log` y acepta comandos de prueba en `relics_reforged_cmd.txt`, ambos junto a la DLL |
| `crates/nr-assets/` | Lectura de archivos sueltos de los `data*.bhd/.bdt` del juego (RSA, AES, Oodle, TPF, BC7) para sacar los íconos del menú. `cargo run -p nr-assets --example icons` arma el atlas fuera del juego |
| `crates/re-tools/` | `disasm`: desensamblador del volcado de `nightreign.exe` (comando `dumpimage` de la DLL) |
| `me3/relics-reforged-dev.me3` | Perfil de me3 para probar, con un save aparte (`NR_RelicsReforged.sl2`) |
| `docs/relevamiento.md` | Todo lo averiguado: formato del save, params, punteros, reglas |
| `dist/` | Lo que va en la publicación: perfiles de me3 (`RelicsReforged.me3`, sin conexión y con una copia del save; `RelicsReforged-Seamless.me3`, con la carpeta `SeamlessCoop` del usuario) y el `README.txt` para el usuario |
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

Las dos quedan en `target/release/relics_reforged.dll` (la última que se compiló). Para publicar: `powershell -File tools/package.ps1` (compila la pública aparte, en `target/public`, y arma `target/dist/RelicsReforged-v<versión>.zip`). Para probar, doble clic en `me3/relics-reforged-dev.me3`. El log queda en `target/release/relics_reforged.log`.

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

Están en `Rules` (`crates/relic-rules/src/transfer.rs`) y se pueden ajustar.

| Regla | Valor |
|---|---|
| Sacrificios por transferencia | 3 (chica = 1 punto, mediana = 2, grande = 3) |
| Puntos mínimos (receptora de 1 / 2 / 3 efectos) | 3 / 5 / 7 |
| Murk (receptora de 1 / 2 / 3 efectos) | 2.500 / 5.000 / 10.000, ×2 si es Deep |
| Forjas por reliquia (1 / 2 / 3 efectos); cuenta también reforjar un efecto ya forjado | 1 / 1 / 2 |
