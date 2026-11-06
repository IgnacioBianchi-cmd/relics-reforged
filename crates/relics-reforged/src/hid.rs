//! Mandos de PlayStation leídos por HID, como los lee el juego.
//!
//! Con un mando de PlayStation, el juego no usa XInput ni el joystick de DirectInput (0 lecturas
//! en la prueba del 2026-11-06): abre el dispositivo HID (`CreateFileW` de una ruta
//! `\\?\hid#vid_054c&pid_...`) y lee sus reportes con `ReadFile` y `GetOverlappedResult`. Esas
//! importaciones (y `CloseHandle`) se reemplazan en la tabla de `nightreign.exe`; solo se toca lo
//! que pasa por el handle de un mando de Sony:
//! - cada reporte leído se guarda para la forja ([`state`]);
//! - si [`input_block`](crate::input_block) bloquea, el reporte se deja en reposo (sticks al
//!   centro, gatillos sueltos, sin botones) y, en Bluetooth, se recalcula su CRC.
//!
//! Formatos (byte 0 = ID del reporte):
//! - DualSense USB `0x01` (64 bytes): sticks 1-4, L2 5, R2 6, botones 8-10.
//! - DualSense Bluetooth `0x31` (78): todo corrido uno (sticks 2-5), CRC32 en 74-77.
//! - DualShock 4 USB `0x01` (64) y el reporte corto de Bluetooth `0x01` (10, también del
//!   DualSense antes de pasar al completo): sticks 1-4, botones 5-7, L2 8, R2 9.
//! - DualShock 4 Bluetooth `0x11` (78): corrido dos (sticks 3-6), CRC32 en 74-77.
//!
//! Botones: el primer byte lleva el D-pad en los 4 bits bajos (0 arriba, en sentido horario, 8
//! suelto) y cuadrado, cruz, círculo, triángulo en los bits 4-7; el segundo L1, R1, L2, R2,
//! Share/Create, Options, L3, R3; el tercero PS (bit 0) y touchpad (bit 1).

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::pad;

type CreateFileWFn =
    unsafe extern "system" fn(*const u16, u32, u32, *mut c_void, u32, u32, *mut c_void) -> usize;
type ReadFileFn = unsafe extern "system" fn(usize, *mut u8, u32, *mut u32, *mut c_void) -> i32;
type OverlappedResultFn = unsafe extern "system" fn(usize, *mut c_void, *mut u32, i32) -> i32;
type CloseHandleFn = unsafe extern "system" fn(usize) -> i32;

static ORIG_CREATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_READ: AtomicUsize = AtomicUsize::new(0);
static ORIG_OVERLAPPED: AtomicUsize = AtomicUsize::new(0);
static ORIG_CLOSE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Model {
    DualSense,
    DualShock4,
}

/// Handles abiertos de mandos de Sony (0 = libre) y su modelo.
const MAX_HANDLES: usize = 8;
static HANDLES: [AtomicUsize; MAX_HANDLES] = [const { AtomicUsize::new(0) }; MAX_HANDLES];
static MODELS: Mutex<[Option<Model>; MAX_HANDLES]> = Mutex::new([None; MAX_HANDLES]);
/// Hay algún handle de mando abierto (para salir rápido en los ganchos).
static ANY_HANDLE: AtomicBool = AtomicBool::new(false);

/// Lecturas pendientes (con OVERLAPPED) de un mando: (overlapped, buffer, largo, modelo).
static PENDING: Mutex<Vec<(usize, usize, u32, Model)>> = Mutex::new(Vec::new());

/// Reportes leídos (para el log).
static REPORTS: AtomicU32 = AtomicU32::new(0);
/// El juego tiene algo apretado en el mando.
static HELD: AtomicBool = AtomicBool::new(false);

struct Last {
    buttons: u16,
    at: Option<Instant>,
    logged: bool,
}

static LAST: Mutex<Last> = Mutex::new(Last {
    buttons: 0,
    at: None,
    logged: false,
});

fn find(handle: usize) -> Option<usize> {
    if !ANY_HANDLE.load(Ordering::Relaxed) || handle == 0 || handle == usize::MAX {
        return None;
    }
    HANDLES
        .iter()
        .position(|h| h.load(Ordering::Relaxed) == handle)
}

fn model_of(slot: usize) -> Option<Model> {
    MODELS.lock().unwrap_or_else(|e| e.into_inner())[slot]
}

/// Modelo de un mando de Sony por su ruta (`...vid_054c&pid_0ce6...`), o `None` si no es uno.
fn model_from_path(path: &str) -> Option<Model> {
    let p = path.to_ascii_lowercase();
    if !p.contains("vid_054c") {
        return None;
    }
    let pid = p.split("pid_").nth(1).map(|s| &s[..s.len().min(4)]);
    Some(match pid {
        Some("0ce6") | Some("0df2") => Model::DualSense,
        _ => Model::DualShock4,
    })
}

unsafe fn wide_string(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0;
    while len < 1024 && *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

unsafe extern "system" fn create_file_hook(
    name: *const u16,
    access: u32,
    share: u32,
    security: *mut c_void,
    disposition: u32,
    flags: u32,
    template: *mut c_void,
) -> usize {
    // SAFETY: guardado al instalar.
    let orig: CreateFileWFn = std::mem::transmute(ORIG_CREATE.load(Ordering::Relaxed));
    let handle = orig(name, access, share, security, disposition, flags, template);
    // Solo las rutas de dispositivos ("\\?\...") interesan; los archivos del juego, no.
    if name.is_null() || *name != u16::from(b'\\') {
        return handle;
    }
    let path = wide_string(name);
    if !path.starts_with("\\\\?\\") {
        return handle;
    }
    let lower = path.to_ascii_lowercase();
    if lower.contains("hid") {
        crate::log_event(&format!(
            "HID: el juego abrió {path} ({})",
            if handle == usize::MAX { "falló" } else { "ok" }
        ));
    }
    if handle == 0 || handle == usize::MAX {
        return handle;
    }
    if let Some(model) = model_from_path(&path) {
        let mut models = MODELS.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = HANDLES.iter().position(|h| h.load(Ordering::Relaxed) == 0) {
            models[slot] = Some(model);
            HANDLES[slot].store(handle, Ordering::Relaxed);
            ANY_HANDLE.store(true, Ordering::Relaxed);
            crate::log_event(&format!("HID: mando de Sony ({model:?})"));
        }
    }
    handle
}

unsafe extern "system" fn close_handle_hook(handle: usize) -> i32 {
    if let Some(slot) = find(handle) {
        HANDLES[slot].store(0, Ordering::Relaxed);
        MODELS.lock().unwrap_or_else(|e| e.into_inner())[slot] = None;
        ANY_HANDLE.store(
            HANDLES.iter().any(|h| h.load(Ordering::Relaxed) != 0),
            Ordering::Relaxed,
        );
    }
    // SAFETY: guardado al instalar.
    let orig: CloseHandleFn = std::mem::transmute(ORIG_CLOSE.load(Ordering::Relaxed));
    orig(handle)
}

unsafe extern "system" fn read_file_hook(
    handle: usize,
    buffer: *mut u8,
    len: u32,
    read: *mut u32,
    overlapped: *mut c_void,
) -> i32 {
    // SAFETY: guardado al instalar.
    let orig: ReadFileFn = std::mem::transmute(ORIG_READ.load(Ordering::Relaxed));
    let slot = find(handle);
    let model = slot.and_then(model_of);
    if let (Some(model), false) = (model, overlapped.is_null()) {
        // Se anota antes de leer: la lectura puede terminar en otro hilo.
        let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
        pending.retain(|p| p.0 != overlapped as usize);
        pending.push((overlapped as usize, buffer as usize, len, model));
    }
    let ok = orig(handle, buffer, len, read, overlapped);
    if let Some(model) = model {
        if ok != 0 {
            // Terminó en el momento: con OVERLAPPED, la cantidad está en el propio OVERLAPPED
            // (InternalHigh, +8); sin él, en `read`.
            let n = if !read.is_null() {
                *read
            } else if !overlapped.is_null() {
                *((overlapped as *const u8).add(8) as *const usize) as u32
            } else {
                0
            };
            if !overlapped.is_null() {
                PENDING
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .retain(|p| p.0 != overlapped as usize);
            }
            report(buffer, n.min(len), model);
        }
    }
    ok
}

unsafe extern "system" fn overlapped_result_hook(
    handle: usize,
    overlapped: *mut c_void,
    transferred: *mut u32,
    wait: i32,
) -> i32 {
    // SAFETY: guardado al instalar.
    let orig: OverlappedResultFn = std::mem::transmute(ORIG_OVERLAPPED.load(Ordering::Relaxed));
    let ok = orig(handle, overlapped, transferred, wait);
    if ok != 0 && find(handle).is_some() && !transferred.is_null() {
        let entry = {
            let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
            let i = pending.iter().position(|p| p.0 == overlapped as usize);
            i.map(|i| pending.swap_remove(i))
        };
        if let Some((_, buffer, len, model)) = entry {
            report(buffer as *mut u8, (*transferred).min(len), model);
        }
    }
    ok
}

/// Dónde está cada cosa en un reporte.
struct Layout {
    sticks: usize,
    buttons: [usize; 3],
    triggers: [usize; 2],
    crc: bool,
}

fn layout(model: Model, data: &[u8]) -> Option<Layout> {
    let n = data.len();
    match (data.first()?, model) {
        (0x01, Model::DualSense) if n >= 64 => Some(Layout {
            sticks: 1,
            buttons: [8, 9, 10],
            triggers: [5, 6],
            crc: false,
        }),
        (0x31, _) if n >= 78 => Some(Layout {
            sticks: 2,
            buttons: [9, 10, 11],
            triggers: [6, 7],
            crc: true,
        }),
        (0x01, _) if n >= 10 => Some(Layout {
            sticks: 1,
            buttons: [5, 6, 7],
            triggers: [8, 9],
            crc: false,
        }),
        (0x11, _) if n >= 78 => Some(Layout {
            sticks: 3,
            buttons: [7, 8, 9],
            triggers: [10, 11],
            crc: true,
        }),
        _ => None,
    }
}

/// CRC32 de los reportes de Bluetooth: sobre 0xA1 y los primeros 74 bytes.
fn bt_crc(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in std::iter::once(&0xA1u8).chain(&data[..74]) {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Botones (bits de XInput) de un reporte, con el stick izquierdo sumado al D-pad.
fn buttons(data: &[u8], l: &Layout) -> u16 {
    let [b0, b1, b2] = l.buttons.map(|i| data[i]);
    let mut out = 0;
    let hat = b0 & 0x0F;
    if matches!(hat, 7 | 0 | 1) {
        out |= pad::DPAD_UP;
    }
    if matches!(hat, 1..=3) {
        out |= pad::DPAD_RIGHT;
    }
    if matches!(hat, 3..=5) {
        out |= pad::DPAD_DOWN;
    }
    if matches!(hat, 5..=7) {
        out |= pad::DPAD_LEFT;
    }
    for (bit, button) in [(4, pad::X), (5, pad::A), (6, pad::B), (7, pad::Y)] {
        if b0 & (1 << bit) != 0 {
            out |= button;
        }
    }
    for (bit, button) in [
        (0, pad::LEFT_SHOULDER),
        (1, pad::RIGHT_SHOULDER),
        (2, pad::LEFT_TRIGGER),
        (3, pad::RIGHT_TRIGGER),
        (5, pad::START),
        (6, pad::LEFT_THUMB),
        (7, pad::RIGHT_THUMB),
    ] {
        if b1 & (1 << bit) != 0 {
            out |= button;
        }
    }
    if b2 & 0x02 != 0 {
        out |= pad::BACK;
    }
    let (x, y) = (data[l.sticks], data[l.sticks + 1]);
    if x < 64 {
        out |= pad::DPAD_LEFT;
    }
    if x > 192 {
        out |= pad::DPAD_RIGHT;
    }
    if y < 64 {
        out |= pad::DPAD_UP;
    }
    if y > 192 {
        out |= pad::DPAD_DOWN;
    }
    out
}

/// Un reporte que el juego acaba de leer: se guarda y, si hay que bloquear, se deja en reposo.
unsafe fn report(buffer: *mut u8, n: u32, model: Model) {
    if buffer.is_null() || n == 0 {
        return;
    }
    let data = std::slice::from_raw_parts_mut(buffer, n as usize);
    REPORTS.fetch_add(1, Ordering::Relaxed);
    let Some(l) = layout(model, data) else {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if !last.logged {
            last.logged = true;
            crate::log_event(&format!(
                "HID: reporte desconocido del mando (ID {:#04X}, {n} bytes)",
                data[0]
            ));
        }
        return;
    };
    let pressed = buttons(data, &l);
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if !last.logged {
            last.logged = true;
            crate::log_event(&format!(
                "HID: el juego lee el mando ({model:?}, reporte {:#04X} de {n} bytes)",
                data[0]
            ));
        }
        last.buttons = pressed;
        last.at = Some(Instant::now());
    }
    let triggers_down = l.triggers.iter().any(|&i| data[i] > 60);
    HELD.store(pressed != 0 || triggers_down, Ordering::Relaxed);
    if !crate::input_block::blocked() {
        return;
    }
    data[l.sticks..l.sticks + 4].fill(0x80);
    for i in l.triggers {
        data[i] = 0;
    }
    let [b0, b1, b2] = l.buttons;
    data[b0] = 0x08;
    data[b1] = 0;
    // En el DualShock 4, los 6 bits altos del tercero son un contador: se dejan.
    data[b2] &= 0xFC;
    if l.crc {
        let crc = bt_crc(data);
        data[74..78].copy_from_slice(&crc.to_le_bytes());
    }
}

/// Botones del mando según el último reporte, si llegó uno hace poco.
pub fn state() -> Option<u16> {
    let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    last.at
        .filter(|t| t.elapsed() < Duration::from_millis(500))
        .map(|_| last.buttons)
}

pub fn released() -> bool {
    !HELD.load(Ordering::Relaxed)
}

/// Reportes leídos desde la última vez (para el log).
pub fn take_reports() -> u32 {
    REPORTS.swap(0, Ordering::Relaxed)
}

/// Reemplaza `CreateFileW`, `ReadFile`, `GetOverlappedResult` y `CloseHandle` en la tabla de
/// importaciones del juego. Devuelve las que no pudo reemplazar.
pub fn install(base: usize) -> Vec<&'static str> {
    let hooks: [(&'static str, usize, &AtomicUsize); 4] = [
        (
            "CreateFileW",
            create_file_hook as CreateFileWFn as usize,
            &ORIG_CREATE,
        ),
        ("ReadFile", read_file_hook as ReadFileFn as usize, &ORIG_READ),
        (
            "GetOverlappedResult",
            overlapped_result_hook as OverlappedResultFn as usize,
            &ORIG_OVERLAPPED,
        ),
        (
            "CloseHandle",
            close_handle_hook as CloseHandleFn as usize,
            &ORIG_CLOSE,
        ),
    ];
    let mut missing = Vec::new();
    for (func, hook, orig) in hooks {
        let Some(slot) = crate::input_block::iat_slot_pub(base, "kernel32.dll", func) else {
            missing.push(func);
            continue;
        };
        // SAFETY: la entrada es un puntero a la función, con la misma firma que el gancho; se
        // guarda la original antes de escribir.
        if unsafe { crate::input_block::patch(slot, 0, hook, orig) }.is_err() {
            missing.push(func);
        }
    }
    missing
}
