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
//! | `hook` | instala el gancho del hilo principal (hace falta para `murk`, `set`, `remove` y `forge`) |
//! | `forge <donante> <slot> <receptora> <slot> <s1> <s2> <s3>` | la transferencia completa (ver [`crate::forge`]) |
//! | `forge? ...` | igual, pero solo valida y muestra el resultado |
//! | `snap <nombre>` | foto de PlayerGameData e inventario; lista los cambios respecto de la foto anterior |
//! | `locks` | reliquias que no se pueden donar ni sacrificar, y por qué |
//! | `findtext <texto>` | busca un texto en UTF-16 en toda la memoria |
//! | `fmg <dirección>` | cabecera del FMG que contiene ese texto, y su ID |
//! | `find <u32> ...` | busca esa secuencia de valores (hex, `*` = cualquiera) en toda la memoria |
//! | `ptrs <dirección> [pgd\|gdm\|nr\|dir] [largo]` | punteros hacia esa dirección (o un objeto que la contiene) |
//! | `mem <dirección> [largo]` | muestra memoria en hex; las direcciones aceptan `pgd+`, `gdm+`, `nr+` |
//! | `dumpdata <nombre>` | guarda las variables globales del ejecutable en `data_<nombre>.bin` |
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
        // Sonido de menú del juego: se <id> [prioridad]
        ["se", id, rest @ ..] => {
            let id: i32 = id.parse().map_err(|_| "ID inválido")?;
            let priority: i32 = match rest.first() {
                Some(p) => p.parse().map_err(|_| "prioridad inválida")?,
                None => 10,
            };
            crate::sound::play(id, priority);
            Ok(format!("sonido {id} pedido (prioridad {priority})"))
        }
        // Ejecución de código: watchx <rva> [largo en hex]
        ["watchx", rva, rest @ ..] => {
            let base = ctx.game.base();
            let size = win::pe_info(base).map_or(0, |(_, s)| s as usize);
            let rva = usize::from_str_radix(rva.trim_start_matches("nr+"), 16)
                .map_err(|_| "RVA inválida")?;
            let len = match rest.first() {
                Some(l) => usize::from_str_radix(l, 16).map_err(|_| "largo inválido")?,
                None => 1,
            };
            tracer::watch_exec(base + rva, len, (base, base + size))?;
            Ok(format!(
                "vigilando la ejecución de nr+{rva:X} ({len:#X} bytes)"
            ))
        }
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
        ["dumpdata", name] => dump_data(ctx, name),
        ["hook"] => hook(ctx),
        ["snap", name] => crate::snap::take(ctx.game, name),
        ["locks"] => locks(ctx),
        ["find", words @ ..] if !words.is_empty() => crate::search::find(ctx.game, words),
        ["findtext", words @ ..] if !words.is_empty() => crate::search::find_text(&words.join(" ")),
        ["fmg", addr] => crate::search::fmg_of(crate::search::parse_addr(ctx.game, addr)?),
        ["ptrs", target, rest @ ..] => {
            let target = crate::search::parse_addr(ctx.game, target)?;
            let base = rest.first().copied().unwrap_or("pgd");
            let len = match rest.get(1) {
                Some(l) => Some(
                    usize::from_str_radix(l.trim_start_matches("0x"), 16)
                        .map_err(|_| format!("largo inválido: {l}"))?,
                ),
                None => None,
            };
            crate::search::ptrs(ctx.game, target, base, len)
        }
        ["mem", addr, rest @ ..] => {
            let addr = crate::search::parse_addr(ctx.game, addr)?;
            let len = match rest.first() {
                Some(l) => usize::from_str_radix(l.trim_start_matches("0x"), 16)
                    .map_err(|_| format!("largo inválido: {l}"))?,
                None => 0x100,
            };
            Ok(crate::search::mem(addr, len))
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
            crate::forge::forge(*ctx.game, data, &crate::userdata::dir(), order)
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

/// Lista las reliquias bloqueadas (favoritas o mencionadas en los datos del jugador) y dónde.
fn locks(ctx: &Context) -> Result<String, String> {
    use std::collections::BTreeMap;
    let mut by_handle: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (h, place) in ctx.game.relic_references()? {
        by_handle.entry(h).or_default().push(place);
    }
    for (_, entry) in ctx.game.inventory_entries() {
        if entry[0] >> 16 == 0xC080 && crate::game::is_favorite(&entry) {
            by_handle
                .entry(entry[0])
                .or_default()
                .push("favorita".into());
        }
    }
    let names: BTreeMap<u32, String> = ctx
        .game
        .relics()
        .into_iter()
        .map(|r| {
            let name = ctx
                .data
                .and_then(|d| d.relic_types.get(&r.item_id))
                .map_or(String::from("?"), |t| t.name.clone());
            (r.handle, name)
        })
        .collect();
    let mut out = format!("{} reliquias bloqueadas:", by_handle.len());
    for (h, places) in &by_handle {
        let name = names
            .get(h)
            .map_or("(no es una reliquia viva)", |n| n.as_str());
        let _ = write!(out, "\n  {h:08X} {name}: {}", places.join(", "));
    }
    Ok(out)
}

fn inv(ctx: &Context, handle: u32) -> Result<String, String> {
    let (index, entry) = ctx
        .game
        .inventory_entry(handle)
        .ok_or("el handle no está en el inventario")?;
    Ok(format!("inventario[{index}] = {entry:08X?}"))
}

/// Guarda las secciones escribibles del ejecutable (variables globales) en `data_<nombre>.bin`,
/// para comparar fotos tomadas en distintos momentos. Cada sección va precedida de su RVA y su
/// tamaño (u32 + u32).
fn dump_data(ctx: &Context, name: &str) -> Result<String, String> {
    let base = ctx.game.base();
    let mut out = Vec::new();
    let mut total = 0;
    for (rva, size) in win::writable_sections(base) {
        let (bytes, _) = win::read_bytes(base + rva, size);
        out.extend_from_slice(&(rva as u32).to_le_bytes());
        out.extend_from_slice(&(size as u32).to_le_bytes());
        out.extend_from_slice(&bytes);
        total += size;
    }
    let path = ctx.dll_dir.join(format!("data_{name}.bin"));
    std::fs::write(&path, &out).map_err(|e| format!("no pude escribir {}: {e}", path.display()))?;
    Ok(format!(
        "{total:#X} bytes de globales en {}",
        path.display()
    ))
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
