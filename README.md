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
| Bloquear favoritas/equipadas, menú dentro del juego, Seamless Co-op | pendiente |

## Estructura

| Ruta | Contenido |
|---|---|
| `crates/relic-rules/` | Motor de reglas en Rust: legalidad de reliquias y transferencias. Sin acceso al juego |
| `crates/relics-reforged/` | La DLL que carga me3. Escribe `relics_reforged.log` y acepta comandos de prueba en `relics_reforged_cmd.txt`, ambos junto a la DLL |
| `crates/re-tools/` | `disasm`: desensamblador del volcado de `nightreign.exe` (comando `dumpimage` de la DLL) |
| `me3/relics-reforged-dev.me3` | Perfil de me3 para probar, con un save aparte (`NR_RelicsReforged.sl2`) |
| `docs/relevamiento.md` | Todo lo averiguado: formato del save, params, punteros, reglas |
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

Para probar en el juego:

```bash
cargo build --release -p relics-reforged
```

Después, doble clic en `me3/relics-reforged-dev.me3`. El log queda en `target/release/relics_reforged.log`.

## Reglas de balance (por defecto)

Están en `Rules` (`crates/relic-rules/src/transfer.rs`) y se pueden ajustar.

| Regla | Valor |
|---|---|
| Sacrificios por transferencia | 3 (chica = 1 punto, mediana = 2, grande = 3) |
| Puntos mínimos (receptora de 1 / 2 / 3 efectos) | 3 / 5 / 7 |
| Murk (receptora de 1 / 2 / 3 efectos) | 2.500 / 5.000 / 10.000, ×2 si es Deep |
| Efectos forjados por reliquia (1 / 2 / 3 efectos) | 1 / 1 / 2 |
