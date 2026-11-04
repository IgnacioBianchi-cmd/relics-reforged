//! Comandos de desarrollo: la DLL lee `relics_reforged_cmd.txt` (junto a la DLL) una vez por
//! segundo, ejecuta cada línea, escribe el resultado en el log y borra el archivo.
//!
//! | Comando | Qué hace |
//! |---|---|
//! | `dump` | vuelve a listar el inventario |
//! | `obj <handle>` | muestra en hex los 0x100 bytes del objeto de una reliquia |
//! | `murk <actual> <nuevo>` | cambia el murk, solo si el valor actual coincide |
//! | `set <handle> <ef>[/<maldición>] ...` | reescribe los efectos (hasta 3, `-` = vacío) y la lista de SpEffects; tiene que quedar legal |
//! | `set! ...` | igual, pero sin exigir que la lista de SpEffects actual sea la esperada (para reparar) |
//! | `watch <handle>` / `unwatch` | rastrea qué código escribe en esa reliquia |
//! | `watchr <handle>` | igual, pero también anota las lecturas (agrupadas por instrucción) |
//! | `profile <segundos>` | muestrea todos los hilos y escribe `profile.txt` (ver `disasm ... profile`) |
//! | `inv <handle>` | muestra la entrada del inventario de ese handle |
//! | `remove <handle>` | quita la reliquia con `RemoveItem` del juego, como la venta (no da murk) |
//! | `hook` | instala el gancho del hilo principal (hace falta para `murk`, `set`, `remove`, `save` y `forge`) |
//! | `save` | pide un guardado al juego |
//! | `forge <donante> <slot> <receptora> <slot> <s1> <s2> <s3>` | la transferencia completa (ver [`crate::forge`]) |
//! | `forge? ...` | igual, pero solo valida y muestra el resultado |
//! | `dumpimage` | copia la imagen de `nightreign.exe` a `nightreign_dump.bin` para analizarla |
//!
//! Los handles van en hex (`C08000E6`), los efectos en decimal, como en el log.

use std::fmt::Write as _;
use std::path::Path;

use relic_rules::{normalize, relic_kind, validate, GameData, Relic, RelicKind, Slot, EMPTY};

use crate::game::Game;
use crate::mainthread;
use crate::tracer;
use crate::win;
use crate::Log;

pub const FILE_NAME: &str = "relics_reforged_cmd.txt";
/// Tiempo máximo para que el hilo principal ejecute un trabajo (en una pantalla de carga no corre).
const MAIN_THREAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const OBJECT_SIZE: usize = 0x100;

pub struct Context<'a> {
    pub game: &'a Game,
    pub data: Option<&'a GameData>,
    pub dll_dir: &'a Path,
    /// Lo pone en `true` el comando `dump`.
    pub dump_requested: bool,
}

/// Lee y ejecuta el archivo de comandos, si existe.
pub fn poll(ctx: &mut Context, log: &mut Log) {
    let path = ctx.dll_dir.join(FILE_NAME);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let _ = std::fs::remove_file(&path);
    let text = text.trim_start_matches('\u{feff}'); // BOM de algunos editores
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        log.line(&format!("> {line}"));
        let result = run(ctx, line);
        match result {
            Ok(msg) => log.line(&msg),
            Err(msg) => log.line(&format!("ERROR: {msg}")),
        }
    }
}

fn run(ctx: &mut Context, line: &str) -> Result<String, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        ["dump"] => {
            ctx.dump_requested = true;
            Ok("ok".into())
        }
        ["obj", h] => obj(ctx, parse_handle(h)?),
        ["murk", old, new] => murk(ctx, parse_u32(old)?, parse_u32(new)?),
        ["set", h, slots @ ..] => set(ctx, parse_handle(h)?, slots, false),
        ["set!", h, slots @ ..] => set(ctx, parse_handle(h)?, slots, true),
        ["watch", h] => watch(ctx, parse_handle(h)?, false),
        ["watchr", h] => watch(ctx, parse_handle(h)?, true),
        ["profile", secs] => profile(ctx, parse_u32(secs)?),
        ["unwatch"] => {
            tracer::unwatch();
            let (reads, outside, dropped) = tracer::counters();
            Ok(format!(
                "vigilancia terminada: {reads} lecturas del objeto, {outside} accesos al resto de la página, \
                 {dropped} escrituras sin anotar"
            ))
        }
        ["dumpimage"] => dump_image(ctx),
        ["hook"] => hook(ctx),
        ["save"] => {
            let game = *ctx.game;
            // SAFETY: corre en el hilo principal.
            mainthread::run(move || unsafe { game.save_now() }, MAIN_THREAD_TIMEOUT)??;
            Ok("se pidió un guardado".into())
        }
        [cmd @ ("forge" | "forge?"), d, ds, t, ts, s1, s2, s3] => {
            not_while_watching()?;
            let data = ctx.data.ok_or("sin params no puedo validar")?;
            let order = crate::forge::Order {
                donor: parse_handle(d)?,
                donor_slot: parse_u32(ds)? as usize,
                target: parse_handle(t)?,
                target_slot: parse_u32(ts)? as usize,
                sacrifices: [parse_handle(s1)?, parse_handle(s2)?, parse_handle(s3)?],
                dry_run: *cmd == "forge?",
            };
            crate::forge::forge(*ctx.game, data, ctx.dll_dir, order)
        }
        ["remove", h] => remove(ctx, parse_handle(h)?),
        ["inv", h] => inv(ctx, parse_handle(h)?),
        _ => Err("comando desconocido".into()),
    }
}

fn parse_handle(s: &str) -> Result<u32, String> {
    u32::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|_| format!("handle inválido: {s}"))
}

fn parse_u32(s: &str) -> Result<u32, String> {
    s.parse().map_err(|_| format!("número inválido: {s}"))
}

fn not_while_watching() -> Result<(), String> {
    if tracer::is_active() {
        Err("hay una vigilancia activa; mandá `unwatch` primero".into())
    } else {
        Ok(())
    }
}

fn obj(ctx: &Context, handle: u32) -> Result<String, String> {
    not_while_watching()?;
    let (object, _) = ctx
        .game
        .relic_by_handle(handle)
        .ok_or("no hay una reliquia con ese handle")?;
    let (bytes, _) = win::read_bytes(object, OBJECT_SIZE);
    let mut out = format!("objeto {handle:08X} en {object:#X}:");
    for (i, row) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "\n  +{:02X} ", i * 16);
        for word in row.chunks(4) {
            let v = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
            let _ = write!(out, " {v:08X}");
        }
    }
    Ok(out)
}

fn murk(ctx: &Context, old: u32, new: u32) -> Result<String, String> {
    let game = *ctx.game;
    mainthread::run(
        move || {
            let addr = game.murk_addr().ok_or("no encuentro el murk")?;
            let current: u32 = win::read(addr).ok_or("no puedo leer el murk")?;
            if current != old {
                return Err(format!(
                    "el murk actual es {current}, no {old}; no cambio nada"
                ));
            }
            if !win::write(addr, new) {
                return Err("no pude escribir el murk".to_string());
            }
            let after: u32 = win::read(addr).unwrap_or(0);
            Ok(format!("murk {current} -> {after} (en el hilo principal)"))
        },
        MAIN_THREAD_TIMEOUT,
    )?
}

fn hook(ctx: &Context) -> Result<String, String> {
    mainthread::install(ctx.game.base())?;
    let before = mainthread::frames();
    std::thread::sleep(std::time::Duration::from_secs(1));
    let after = mainthread::frames();
    let tid = mainthread::run(|| unsafe { win::GetCurrentThreadId() }, MAIN_THREAD_TIMEOUT)?;
    Ok(format!(
        "gancho instalado: {} frames en 1 s, hilo principal {tid}",
        after - before
    ))
}

fn parse_slot(s: &str) -> Result<Slot, String> {
    if s == "-" {
        return Ok(Slot::EMPTY);
    }
    let (effect, curse) = match s.split_once('/') {
        Some((e, c)) => (parse_u32(e)?, Some(parse_u32(c)?)),
        None => (parse_u32(s)?, None),
    };
    Ok(Slot::new(effect, curse))
}

fn set(ctx: &Context, handle: u32, slots: &[&str], force: bool) -> Result<String, String> {
    not_while_watching()?;
    let data = ctx
        .data
        .ok_or("sin params no puedo validar; no escribo nada")?;
    if slots.is_empty() || slots.len() > 3 {
        return Err("hacen falta entre 1 y 3 efectos".into());
    }
    let (_, live) = ctx
        .game
        .relic_by_handle(handle)
        .ok_or("no hay una reliquia con ese handle")?;
    if !matches!(relic_kind(data, live.item_id), RelicKind::Random { .. }) {
        return Err("la reliquia no es aleatoria".into());
    }
    let mut relic = Relic::from_raw(live.item_id, live.effects, live.curses);
    // Control de que entendemos el objeto: la lista de SpEffects del juego tiene que ser la que
    // calculamos para la reliquia actual. `set!` lo saltea (para reparar una reliquia ya tocada).
    let current_sp = ctx
        .game
        .sp_effects(handle)
        .ok_or("no puedo leer los SpEffects")?;
    if !force && relic.sp_effects(data).as_deref() != Some(&current_sp[..]) {
        return Err(format!(
            "los SpEffects en memoria {current_sp:?} no son los esperados; no escribo nada"
        ));
    }
    for (i, s) in slots.iter().enumerate() {
        relic.slots[i] = parse_slot(s)?;
    }
    for slot in relic.slots.iter_mut().skip(slots.len()) {
        *slot = Slot::EMPTY;
    }
    normalize(data, &mut relic);
    validate(data, &relic).map_err(|v| format!("el resultado no sería legal: {v}"))?;
    let sp = relic
        .sp_effects(data)
        .ok_or("un efecto no está en los params")?;
    let (effects, curses) = relic.to_raw();
    let game = *ctx.game;
    let written = mainthread::run(
        move || game.write_slots(handle, effects, curses, &sp),
        MAIN_THREAD_TIMEOUT,
    )?;
    if !written {
        return Err("no pude escribir".into());
    }
    let (_, after) = ctx
        .game
        .relic_by_handle(handle)
        .ok_or("la reliquia desapareció después de escribir")?;
    let after_sp = ctx.game.sp_effects(handle).unwrap_or_default();
    let fmt = |v: [u32; 4]| {
        v.iter()
            .map(|&x| {
                if x == EMPTY {
                    "-".to_string()
                } else {
                    x.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    Ok(format!(
        "reliquia {handle:08X} reescrita: efectos [{}] maldiciones [{}] SpEffects {after_sp:?}",
        fmt(after.effects),
        fmt(after.curses)
    ))
}

fn profile(ctx: &Context, seconds: u32) -> Result<String, String> {
    if !(1..=60).contains(&seconds) {
        return Err("entre 1 y 60 segundos".into());
    }
    let base = ctx.game.base();
    let size = win::pe_info(base).map_or(0, |(_, s)| s as usize);
    let (text, summary) = crate::profiler::run(seconds as u64, 5, (base, base + size))?;
    let path = ctx.dll_dir.join("profile.txt");
    std::fs::write(&path, text).map_err(|e| format!("no pude escribir {}: {e}", path.display()))?;
    Ok(format!("perfil: {summary}, en {}", path.display()))
}

fn watch(ctx: &Context, handle: u32, reads: bool) -> Result<String, String> {
    let (object, live) = ctx
        .game
        .relic_by_handle(handle)
        .ok_or("no hay una reliquia con ese handle")?;
    let base = ctx.game.base();
    let size = win::pe_info(base).map_or(0, |(_, s)| s as usize);
    tracer::watch(object, OBJECT_SIZE, (base, base + size), reads)?;
    Ok(format!(
        "vigilando {handle:08X} (item {}) en {object:#X}. Mientras dure, el inventario no se relee.",
        live.item_id
    ))
}

fn remove(ctx: &Context, handle: u32) -> Result<String, String> {
    not_while_watching()?;
    let (_, live) = ctx
        .game
        .relic_by_handle(handle)
        .ok_or("no hay una reliquia con ese handle")?;
    let murk_before = ctx.game.murk();
    let entry = ctx.game.inventory_entry(handle);
    // Control de que entendemos el inventario: la entrada tiene que tener el mismo ítem y cantidad 1.
    match entry {
        Some((_, e)) if e[1] == live.item_id | 0x8000_0000 && e[2] == 1 => {}
        other => {
            return Err(format!(
                "la entrada del inventario no es la esperada: {other:X?}; no borro nada"
            ))
        }
    }
    let game = *ctx.game;
    // SAFETY: corre en el hilo principal, entre dos frames.
    let result = mainthread::run(
        move || unsafe { game.remove_item(handle) },
        MAIN_THREAD_TIMEOUT,
    )??;
    let still_there = ctx.game.relic_by_handle(handle).is_some();
    Ok(format!(
        "remove {handle:08X} (item {}, entrada {entry:X?}): RemoveItem devolvió {result}; en la tabla {}; \
         en el inventario {}; murk {murk_before:?} -> {:?}",
        live.item_id,
        if still_there { "SIGUE" } else { "ya no está" },
        if ctx.game.inventory_entry(handle).is_some() { "SIGUE" } else { "ya no está" },
        ctx.game.murk()
    ))
}

fn inv(ctx: &Context, handle: u32) -> Result<String, String> {
    let (index, entry) = ctx
        .game
        .inventory_entry(handle)
        .ok_or("el handle no está en el inventario")?;
    Ok(format!("inventario[{index}] = {entry:08X?}"))
}

fn dump_image(ctx: &Context) -> Result<String, String> {
    let base = ctx.game.base();
    let (_, size) = win::pe_info(base).ok_or("no puedo leer la cabecera PE")?;
    let (bytes, failed) = win::read_bytes(base, size as usize);
    let path = ctx.dll_dir.join("nightreign_dump.bin");
    std::fs::write(&path, &bytes)
        .map_err(|e| format!("no pude escribir {}: {e}", path.display()))?;
    Ok(format!(
        "imagen de {:#X} bytes copiada a {} ({failed} páginas ilegibles)",
        size,
        path.display()
    ))
}
