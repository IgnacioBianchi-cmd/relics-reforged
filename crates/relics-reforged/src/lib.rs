//! # Relics Reforged (DLL)
//!
//! Versión de desarrollo. Al cargar la partida escribe en `relics_reforged.log`, junto a la
//! DLL, el murk y todas las reliquias leídas de la memoria del juego, validadas con el motor de
//! reglas. Solo modifica algo si se lo pide un comando de `relics_reforged_cmd.txt`
//! (ver [`commands`]).

mod commands;
mod forge;
mod game;
mod input_block;
mod mainthread;
mod msg;
mod profiler;
mod relic_menu;
mod search;
mod snap;
mod tracer;
mod ui;
mod win;

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use relic_rules::{relic_kind, validate, GameData, Relic, RelicKind};

use crate::game::{Game, LiveRelic, SUPPORTED_TIMESTAMP};

const DLL_PROCESS_ATTACH: u32 = 1;

/// Punto de entrada de la DLL. Solo lanza un hilo: hacer trabajo dentro de `DllMain`
/// (con el loader lock tomado) puede colgar el juego.
#[no_mangle]
#[allow(non_snake_case)]
// La firma la impone Windows; el puntero solo se le devuelve al sistema.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "system" fn DllMain(
    module: win::Handle,
    reason: u32,
    _reserved: *mut std::ffi::c_void,
) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        // SAFETY: `module` es el handle que nos pasa el loader.
        unsafe { win::DisableThreadLibraryCalls(module) };
        let module = module as usize;
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| run(module));
            if result.is_err() {
                if let Some(mut log) = Log::open(module) {
                    log.line("ERROR: el hilo del mod terminó por un panic");
                }
            }
        });
    }
    1
}

pub(crate) struct Log {
    file: File,
}

/// Momento en que arrancó el mod: los tiempos del log son relativos a esto.
static START: OnceLock<Instant> = OnceLock::new();
/// Handle de nuestra DLL, para que cualquier hilo pueda escribir en el log.
static MODULE: AtomicUsize = AtomicUsize::new(0);

impl Log {
    fn open(module: usize) -> Option<Log> {
        let path = win::module_path(module)?.with_file_name("relics_reforged.log");
        let file = File::options().create(true).append(true).open(path).ok()?;
        Some(Log { file })
    }

    pub(crate) fn line(&mut self, text: &str) {
        let t = START.get_or_init(Instant::now).elapsed().as_secs_f32();
        let _ = writeln!(self.file, "[{t:8.1}s] {text}");
        let _ = self.file.flush();
    }
}

/// Escribe una línea en el log desde cualquier hilo (por ejemplo, el del menú).
pub(crate) fn log_event(text: &str) {
    if let Some(mut log) = Log::open(MODULE.load(Ordering::Relaxed)) {
        log.line(text);
    }
}

fn run(module: usize) {
    MODULE.store(module, Ordering::Relaxed);
    START.get_or_init(Instant::now);
    let dll_path = win::module_path(module).unwrap_or_default();
    // Empezamos un log nuevo en cada arranque del juego.
    let _ = File::create(dll_path.with_file_name("relics_reforged.log"));
    let Some(mut log) = Log::open(module) else {
        return;
    };

    log.line(&format!(
        "Relics Reforged v{} (desarrollo: comandos en {})",
        env!("CARGO_PKG_VERSION"),
        commands::FILE_NAME
    ));
    log.line(&format!("DLL: {}", dll_path.display()));

    let base = win::main_module_base();
    let game = match Game::attach(base) {
        Ok(g) => g,
        Err(ts) => {
            log.line(&format!(
                "Versión del juego NO soportada (TimeDateStamp {ts:08X?}, se esperaba {SUPPORTED_TIMESTAMP:08X}). \
                 El mod queda inactivo."
            ));
            return;
        }
    };
    log.line(&format!("nightreign.exe en {base:#X}, versión soportada"));
    match input_block::install() {
        Ok(()) => log.line("Bloqueo de teclado y mouse del juego instalado (DirectInput 8)"),
        Err(e) => log.line(&format!(
            "ERROR: no pude instalar el bloqueo de teclado y mouse: {e}"
        )),
    }
    match relic_menu::install(base) {
        Ok(()) => log.line("Detección de Relic Rites instalada (vtable de AntiqueEquipDialog)"),
        Err(e) => log.line(&format!(
            "ERROR: no pude instalar la detección de Relic Rites: {e}"
        )),
    }

    let mut data = load_game_data(&dll_path, &mut log).map(Arc::new);
    let dll_dir = dll_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    // Por si quedó un archivo de comandos de una sesión anterior.
    let _ = std::fs::remove_file(dll_dir.join(commands::FILE_NAME));
    // El menú arranca cuando la partida está cargada, con los nombres ya leídos del juego.
    let mut menu_started = false;

    // Esperamos a que la partida esté cargada y estable (mismo estado dos lecturas seguidas).
    let mut waiting_logged = false;
    let mut previous: Option<(u32, Vec<LiveRelic>)> = None;
    let mut dumped: Option<(u32, Vec<LiveRelic>)> = None;
    loop {
        std::thread::sleep(Duration::from_secs(1));
        let mut ctx = commands::Context {
            game: &game,
            data: data.as_deref(),
            dll_dir: &dll_dir,
            dump_requested: false,
        };
        commands::poll(&mut ctx, &mut log);
        if ctx.dump_requested {
            dumped = None;
        }
        log_hits(&mut log, base);
        if tracer::is_active() {
            // Leer la página vigilada consumiría la guarda: no tocamos el inventario.
            continue;
        }
        let (Some(murk), relics) = (game.murk(), game.relics()) else {
            if !waiting_logged {
                log.line("Esperando a que cargue la partida...");
                waiting_logged = true;
            }
            continue;
        };
        if relics.is_empty() {
            continue;
        }
        let current = (murk, relics);
        if previous.as_ref() != Some(&current) {
            previous = Some(current);
            continue; // todavía cambiando
        }
        match &dumped {
            Some((old_murk, old_relics)) if *old_relics == current.1 => {
                if *old_murk != current.0 {
                    log.line(&format!("Murk: {old_murk} -> {}", current.0));
                    dumped = Some(current);
                }
            }
            _ => {
                dump(&mut log, data.as_deref(), current.0, &current.1);
                dumped = Some(current);
            }
        }
        // Con la partida cargada y estable, el gancho del hilo principal (lo usa el menú).
        if !mainthread::is_installed() {
            match mainthread::install(base) {
                Ok(()) => log.line("Gancho del hilo principal instalado"),
                Err(e) => log.line(&format!("ERROR: no pude instalar el gancho: {e}")),
            }
        }
        // Nombres desde los textos del juego, y después el menú.
        if !menu_started {
            if let Some(current) = &data {
                let (fmgs, array) = game
                    .player_game_data()
                    .map_or((Vec::new(), None), msg::loaded_fmgs);
                let mut named = (**current).clone();
                let summary = msg::apply_names(&mut named, &fmgs);
                log.line(&format!(
                    "Nombres del juego ({} FMG, array en {array:X?}): {summary}",
                    fmgs.len()
                ));
                // Idioma del menú = idioma del juego: "Strength +3" (7000302) en español es "Fuerza".
                let strength = named
                    .effects
                    .get(&7_000_302)
                    .map_or("", |e| e.name.as_str());
                let lang = if strength.starts_with("Fuerza") {
                    relic_rules::lang::Lang::Es
                } else {
                    relic_rules::lang::Lang::En
                };
                relic_rules::lang::set(lang);
                log.line(&format!(
                    "Idioma del menú: {lang:?} (7000302 = \"{strength}\")"
                ));
                let named = Arc::new(named);
                ui::start(module, game, named.clone(), dll_dir.clone());
                log.line(&format!("Menú: se abre con {}", ui::TOGGLE_KEY_NAME));
                data = Some(named);
                menu_started = true;
            }
        }
    }
}

/// Escribe en el log las escrituras que anotó el rastreador, con direcciones relativas al módulo.
fn log_hits(log: &mut Log, base: usize) {
    let size = win::pe_info(base).map_or(0, |(_, s)| s as usize);
    let rel = |a: usize| {
        if a >= base && a < base + size {
            format!("nr+{:X}", a - base)
        } else {
            format!("{a:#X}")
        }
    };
    for hit in tracer::take_hits() {
        let stack: Vec<String> = hit.stack[..hit.stack_len].iter().map(|&a| rel(a)).collect();
        log.line(&format!(
            "{} x{} en {:#X} desde {} (hilo {}) rcx={:#X} rdx={:#X} r8={:#X} r9={:#X} rbx={:#X} rsi={:#X} rdi={:#X}\n           pila: {}",
            if hit.write { "ESCRITURA" } else { "LECTURA" },
            hit.count,
            hit.addr,
            rel(hit.rip),
            hit.thread,
            hit.regs[0],
            hit.regs[1],
            hit.regs[2],
            hit.regs[3],
            hit.regs[4],
            hit.regs[5],
            hit.regs[6],
            stack.join(" ")
        ));
    }
}

/// Params desde el `regulation.bin` del juego (junto a `nightreign.exe`). Si en desarrollo
/// aparece `data/export` (subiendo desde la carpeta de la DLL), de ahí se toman solo los nombres.
fn load_game_data(dll_path: &Path, log: &mut Log) -> Option<GameData> {
    let exe = win::module_path(win::main_module_base()).unwrap_or_default();
    let regulation = exe.with_file_name("regulation.bin");
    let mut data = match std::fs::read(&regulation)
        .map_err(|e| e.to_string())
        .and_then(|raw| GameData::from_regulation(&raw).map_err(|e| e.to_string()))
    {
        Ok(d) => d,
        Err(e) => {
            log.line(&format!(
                "No pude leer los params de {}: {e}",
                regulation.display()
            ));
            return None;
        }
    };
    log.line(&format!(
        "Params cargados desde {}: {} efectos, {} tipos de reliquia, {} tablas",
        regulation.display(),
        data.effects.len(),
        data.relic_types.len(),
        data.tables.len()
    ));

    let csv_dir: Option<PathBuf> = dll_path
        .ancestors()
        .skip(1)
        .take(6)
        .map(|dir| dir.join("data").join("export"))
        .find(|dir| dir.join("AttachEffectParam.csv").exists());
    match csv_dir.map(|dir| (GameData::from_csv_dir(&dir), dir)) {
        Some((Ok(csv), dir)) => {
            for (id, e) in data.effects.iter_mut() {
                if let Some(c) = csv.effects.get(id) {
                    e.name = c.name.clone();
                }
            }
            for (id, t) in data.relic_types.iter_mut() {
                if let Some(c) = csv.relic_types.get(id) {
                    t.name = c.name.clone();
                }
            }
            log.line(&format!("Nombres tomados de {}", dir.display()));
        }
        Some((Err(e), _)) => log.line(&format!("No pude leer los nombres de los CSV: {e}")),
        None => log.line("Sin CSV de nombres: el menú muestra IDs"),
    }
    Some(data)
}

fn dump(log: &mut Log, data: Option<&GameData>, murk: u32, relics: &[LiveRelic]) {
    log.line(&format!(
        "===== Inventario: murk {murk}, {} reliquias =====",
        relics.len()
    ));
    let (mut legal, mut illegal, mut fixed, mut unknown) = (0, 0, 0, 0);
    for r in relics {
        let relic = Relic::from_raw(r.item_id, r.effects, r.curses);
        let mut text = format!("#{:<4} {:08X} item {:<8}", r.index, r.handle, r.item_id);
        if let Some(data) = data {
            let name = data
                .relic_types
                .get(&r.item_id)
                .map_or("?", |t| t.name.as_str());
            let status = match relic_kind(data, r.item_id) {
                RelicKind::Random { deep, size } => match validate(data, &relic) {
                    Ok(()) => {
                        legal += 1;
                        format!("{}{} OK", if deep { "Deep " } else { "" }, size)
                    }
                    Err(v) => {
                        illegal += 1;
                        format!("ILEGAL: {v}")
                    }
                },
                RelicKind::Fixed => {
                    fixed += 1;
                    "fija".to_string()
                }
                RelicKind::Unknown => {
                    unknown += 1;
                    "tipo desconocido".to_string()
                }
            };
            let _ = write!(text, " [{status}] {name}");
            for slot in relic.slots.iter() {
                if let Some(e) = slot.effect {
                    let ename = data.effects.get(&e).map_or("?", |i| i.name.as_str());
                    let _ = write!(text, "\n                 · {e} {ename}");
                    if let Some(c) = slot.curse {
                        let cname = data.effects.get(&c).map_or("?", |i| i.name.as_str());
                        let _ = write!(text, "  [maldición {c} {cname}]");
                    }
                }
            }
        } else {
            let _ = write!(
                text,
                " efectos {:08X?} maldiciones {:08X?}",
                r.effects, r.curses
            );
        }
        log.line(&text);
    }
    if data.is_some() {
        log.line(&format!(
            "Resumen: {legal} aleatorias legales, {illegal} ILEGALES, {fixed} fijas, {unknown} desconocidas"
        ));
    }
}
