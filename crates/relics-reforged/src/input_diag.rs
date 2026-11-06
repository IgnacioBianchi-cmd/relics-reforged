//! Diagnóstico del mando: ¿por dónde lo lee el juego?
//!
//! Con un mando de PlayStation configurado en Steam, el juego no lo leyó por XInput, ni como
//! joystick de DirectInput, ni abriendo el HID con su `CreateFileW` (prueba del 2026-11-06). Esto
//! deja en el log lo necesario para encontrar el camino en una sola prueba:
//! - las DLL que el juego carga por su cuenta (`LoadLibrary*` de su tabla de importaciones);
//! - los módulos del proceso que tienen que ver con entrada (Steam, overlay, XInput, HID...);
//! - qué ve `XInputGetState` en cada DLL de XInput que esté cargada;
//! - cuántas veces llama el juego a cada método de `ISteamInput` (`SteamInput006`), con
//!   contadores en su vtable como los de [`relic_menu`](crate::relic_menu), cada 10 s durante
//!   3 minutos (solo los que cambiaron).

use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::win;

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> *mut c_void;
    fn Module32FirstW(snapshot: *mut c_void, entry: *mut ModuleEntry) -> i32;
    fn Module32NextW(snapshot: *mut c_void, entry: *mut ModuleEntry) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}

#[repr(C)]
struct ModuleEntry {
    size: u32,
    module_id: u32,
    process_id: u32,
    global_usage: u32,
    process_usage: u32,
    base: *mut u8,
    base_size: u32,
    handle: *mut c_void,
    name: [u16; 256],
    path: [u16; 260],
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn from_wide(s: &[u16]) -> String {
    let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..end])
}

// ---------------------------------------------------------------------------------------------
// LoadLibrary del juego

type LoadW = unsafe extern "system" fn(*const u16) -> *mut c_void;
type LoadExW = unsafe extern "system" fn(*const u16, *mut c_void, u32) -> *mut c_void;
type LoadA = unsafe extern "system" fn(*const u8) -> *mut c_void;
type LoadExA = unsafe extern "system" fn(*const u8, *mut c_void, u32) -> *mut c_void;

static ORIG_LOAD_W: AtomicUsize = AtomicUsize::new(0);
static ORIG_LOAD_EX_W: AtomicUsize = AtomicUsize::new(0);
static ORIG_LOAD_A: AtomicUsize = AtomicUsize::new(0);
static ORIG_LOAD_EX_A: AtomicUsize = AtomicUsize::new(0);
static LOADED: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn note_load(name: String, ok: bool) {
    let mut seen = LOADED.lock().unwrap_or_else(|e| e.into_inner());
    let key = name.to_ascii_lowercase();
    if seen.len() < 60 && !seen.contains(&key) {
        seen.push(key);
        crate::log_event(&format!(
            "Diagnóstico: el juego carga {name} ({})",
            if ok { "ok" } else { "falló" }
        ));
    }
}

unsafe fn str_w(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while n < 512 && *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

unsafe fn str_a(p: *const u8) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while n < 512 && *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned()
}

unsafe extern "system" fn load_w(name: *const u16) -> *mut c_void {
    let orig: LoadW = std::mem::transmute(ORIG_LOAD_W.load(Ordering::Relaxed));
    let m = orig(name);
    note_load(str_w(name), !m.is_null());
    m
}

unsafe extern "system" fn load_ex_w(name: *const u16, file: *mut c_void, flags: u32) -> *mut c_void {
    let orig: LoadExW = std::mem::transmute(ORIG_LOAD_EX_W.load(Ordering::Relaxed));
    let m = orig(name, file, flags);
    note_load(str_w(name), !m.is_null());
    m
}

unsafe extern "system" fn load_a(name: *const u8) -> *mut c_void {
    let orig: LoadA = std::mem::transmute(ORIG_LOAD_A.load(Ordering::Relaxed));
    let m = orig(name);
    note_load(str_a(name), !m.is_null());
    m
}

unsafe extern "system" fn load_ex_a(name: *const u8, file: *mut c_void, flags: u32) -> *mut c_void {
    let orig: LoadExA = std::mem::transmute(ORIG_LOAD_EX_A.load(Ordering::Relaxed));
    let m = orig(name, file, flags);
    note_load(str_a(name), !m.is_null());
    m
}

/// Reemplaza los `LoadLibrary*` de la tabla de importaciones del juego (solo anotan).
pub fn install(base: usize) {
    let hooks: [(&str, usize, &AtomicUsize); 4] = [
        ("LoadLibraryW", load_w as LoadW as usize, &ORIG_LOAD_W),
        ("LoadLibraryExW", load_ex_w as LoadExW as usize, &ORIG_LOAD_EX_W),
        ("LoadLibraryA", load_a as LoadA as usize, &ORIG_LOAD_A),
        ("LoadLibraryExA", load_ex_a as LoadExA as usize, &ORIG_LOAD_EX_A),
    ];
    for (func, hook, orig) in hooks {
        if let Some(slot) = crate::input_block::iat_slot_pub(base, "kernel32.dll", func) {
            // SAFETY: misma firma que la original, que se guarda antes de escribir.
            let _ = unsafe { crate::input_block::patch(slot, 0, hook, orig) };
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Con la partida cargada

/// Módulos, XInput y Steam Input. Llamar con la partida ya cargada.
pub fn run() {
    log_modules();
    log_xinput();
    match hook_steam_input() {
        Ok(n) => crate::log_event(&format!(
            "Diagnóstico: contadores en {n} métodos de ISteamInput (SteamInput006)"
        )),
        Err(e) => crate::log_event(&format!("Diagnóstico: Steam Input: {e}")),
    }
    std::thread::spawn(|| {
        let mut before = [0u64; MAX_METHODS];
        for _ in 0..18 {
            std::thread::sleep(std::time::Duration::from_secs(10));
            log_steam_input(&mut before);
        }
    });
}

fn log_modules() {
    const KEYS: [&str; 12] = [
        "input", "hid", "pad", "steam", "overlay", "dinput", "xinput", "gameinput", "sce",
        "rewasd", "ds4", "vigem",
    ];
    // SAFETY: toolhelp del propio proceso; `entry` se inicializa con su tamaño.
    unsafe {
        let snap = CreateToolhelp32Snapshot(0x8 | 0x10, 0); // MODULE | MODULE32
        if snap as isize == -1 {
            return;
        }
        let mut entry: ModuleEntry = std::mem::zeroed();
        entry.size = std::mem::size_of::<ModuleEntry>() as u32;
        let mut found = Vec::new();
        let mut ok = Module32FirstW(snap, &mut entry);
        while ok != 0 {
            let name = from_wide(&entry.name);
            let lower = name.to_ascii_lowercase();
            if KEYS.iter().any(|k| lower.contains(k)) {
                found.push(name);
            }
            ok = Module32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
        crate::log_event(&format!(
            "Diagnóstico: módulos de entrada cargados: {}",
            found.join(", ")
        ));
    }
}

fn log_xinput() {
    #[repr(C)]
    #[derive(Default)]
    struct State {
        packet: u32,
        buttons: u16,
        lt: u8,
        rt: u8,
        sticks: [i16; 4],
    }
    type GetState = unsafe extern "system" fn(u32, *mut State) -> u32;
    for dll in ["xinput1_4.dll", "xinput1_3.dll", "xinput9_1_0.dll", "xinputuap.dll"] {
        // Solo las que ya estén cargadas (no se carga ninguna nueva).
        // SAFETY: búsqueda de una función exportada conocida.
        let f = unsafe {
            let m = GetModuleHandleW(wide(dll).as_ptr());
            if m.is_null() {
                continue;
            }
            GetProcAddress(m, c"XInputGetState".as_ptr().cast())
        };
        if f.is_null() {
            continue;
        }
        // SAFETY: `f` es XInputGetState.
        let get: GetState = unsafe { std::mem::transmute(f) };
        let results: Vec<String> = (0..4)
            .map(|u| {
                let mut s = State::default();
                // SAFETY: `s` es un XINPUT_STATE.
                match unsafe { get(u, &mut s) } {
                    0 => format!("{u}: conectado"),
                    1167 => format!("{u}: no"),
                    e => format!("{u}: error {e}"),
                }
            })
            .collect();
        crate::log_event(&format!(
            "Diagnóstico: XInputGetState de {dll} (en {f:p}): {}",
            results.join(", ")
        ));
    }
}

const MAX_METHODS: usize = 64;
static CALLS: [AtomicU64; MAX_METHODS] = [const { AtomicU64::new(0) }; MAX_METHODS];
static METHODS: AtomicUsize = AtomicUsize::new(0);

/// Pone contadores en la vtable de `ISteamInput` (la misma interfaz que usa el juego).
fn hook_steam_input() -> Result<usize, String> {
    type GetUser = unsafe extern "C" fn() -> i32;
    type FindOrCreate = unsafe extern "C" fn(i32, *const u8) -> *mut c_void;
    // SAFETY: funciones exportadas de steam_api64.dll con sus firmas del SDK; se leen y se
    // escriben punteros de una vtable que se verificó que apunta a código.
    unsafe {
        let api = GetModuleHandleW(wide("steam_api64.dll").as_ptr());
        if api.is_null() {
            return Err("steam_api64.dll no está cargada".into());
        }
        let get_user = GetProcAddress(api, c"SteamAPI_GetHSteamUser".as_ptr().cast());
        let find = GetProcAddress(api, c"SteamInternal_FindOrCreateUserInterface".as_ptr().cast());
        if get_user.is_null() || find.is_null() {
            return Err("faltan funciones en steam_api64.dll".into());
        }
        let user = std::mem::transmute::<*mut c_void, GetUser>(get_user)();
        let input = std::mem::transmute::<*mut c_void, FindOrCreate>(find)(
            user,
            c"SteamInput006".as_ptr().cast(),
        );
        if input.is_null() {
            return Err(format!("no hay SteamInput006 (usuario {user})"));
        }
        let vtable = *(input as *const usize);
        // Se cuentan los métodos mientras apunten a código (después de la vtable suele venir
        // la información de RTTI de la siguiente).
        let mut originals = Vec::new();
        while originals.len() < MAX_METHODS {
            let Some(f) = win::read::<usize>(vtable + originals.len() * 8) else {
                break;
            };
            if f == 0 || !win::is_executable(f) {
                break;
            }
            originals.push(f);
        }
        if originals.is_empty() {
            return Err("la vtable de ISteamInput no apunta a código".into());
        }
        const THUNK: usize = 0x20;
        let code = win::alloc_executable_near(vtable, originals.len() * THUNK)
            .ok_or("no pude reservar memoria")?;
        for (i, &original) in originals.iter().enumerate() {
            let mut t = vec![0x48, 0xB8]; // mov rax, &CALLS[i]
            t.extend_from_slice(&(&CALLS[i] as *const AtomicU64 as u64).to_le_bytes());
            t.extend_from_slice(&[0xF0, 0x48, 0xFF, 0x00]); // lock inc qword ptr [rax]
            t.extend_from_slice(&[0x48, 0xB8]); // mov rax, método original
            t.extend_from_slice(&(original as u64).to_le_bytes());
            t.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
            std::ptr::copy_nonoverlapping(t.as_ptr(), (code + i * THUNK) as *mut u8, t.len());
        }
        let size = originals.len() * 8;
        let mut old = 0;
        if win::VirtualProtect(vtable as *mut _, size, 0x04, &mut old) == 0 {
            return Err("VirtualProtect falló en la vtable de ISteamInput".into());
        }
        for i in 0..originals.len() {
            std::ptr::write_volatile((vtable + i * 8) as *mut usize, code + i * THUNK);
        }
        win::VirtualProtect(vtable as *mut _, size, old, &mut old);
        METHODS.store(originals.len(), Ordering::SeqCst);
        Ok(originals.len())
    }
}

/// Anota los métodos de `ISteamInput` que el juego llamó desde la última vez.
fn log_steam_input(before: &mut [u64; MAX_METHODS]) {
    let n = METHODS.load(Ordering::Relaxed);
    if n == 0 {
        return;
    }
    let mut changed = Vec::new();
    for i in 0..n {
        let now = CALLS[i].load(Ordering::Relaxed);
        if now != before[i] {
            changed.push(format!("{i}:+{}", now - before[i]));
            before[i] = now;
        }
    }
    crate::log_event(&format!(
        "Diagnóstico: ISteamInput en los últimos 10 s: {}",
        if changed.is_empty() {
            "ninguna llamada".to_string()
        } else {
            changed.join(" ")
        }
    ));
}
