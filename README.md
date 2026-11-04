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
| DLL in-game (me3) | pendiente |

## Estructura

| Ruta | Contenido |
|---|---|
| `crates/relic-rules/` | Motor de reglas en Rust: legalidad de reliquias y transferencias. Sin acceso al juego |
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

## Reglas de balance (por defecto)

Están en `Rules` (`crates/relic-rules/src/transfer.rs`) y se pueden ajustar.

| Regla | Valor |
|---|---|
| Sacrificios por transferencia | 3 (chica = 1 punto, mediana = 2, grande = 3) |
| Puntos mínimos (receptora de 1 / 2 / 3 efectos) | 3 / 5 / 7 |
| Murk (receptora de 1 / 2 / 3 efectos) | 2.500 / 5.000 / 10.000, ×2 si es Deep |
| Efectos forjados por reliquia (1 / 2 / 3 efectos) | 1 / 1 / 2 |
