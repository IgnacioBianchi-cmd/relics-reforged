//! Con la forja abierta, el teclado y el mouse no le tienen que llegar al juego.
//!
//! El juego lee el teclado y el mouse con DirectInput 8, no con los mensajes de la ventana, así
//! que el filtro de mensajes de hudhook no alcanza: sin esto, el "4" abre el filtro de Relic Rites
//! detrás de la forja, el "2" marca favoritas y el "3" guarda un preset.
//!
//! Se crean un teclado y un mouse propios para conseguir las vtables de los dispositivos de
//! `dinput8.dll` (cada tipo tiene la suya, compartida con los del juego) y se reemplazan
//! `GetDeviceState` (9) y `GetDeviceData` (10): con la forja abierta llaman al original (así
//! el dispositivo sigue al día) y después devuelven "nada apretado" y "sin eventos". Se hace para
//! la versión ANSI y la Unicode de la interfaz. Los clics llegan por el mouse de DirectInput: con
//! solo la vtable del teclado, un clic en la forja equipaba reliquias en Relic Rites y el clic
//! derecho lo cerraba.
//!
//! Además, el juego lee la posición del cursor con `GetCursorPos` y el teclado también con
//! `GetKeyboardState` (y puede usar `GetKeyState`). Esas tres se reemplazan en la tabla de
//! importaciones de `nightreign.exe`, así que solo afectan las llamadas del juego (hudhook e
//! ImGui usan las suyas): con la forja abierta, teclas sueltas y el cursor quieto donde estaba al
//! abrirla (sin esto, pasar el mouse por un slot de Relic Rites lo seleccionaba). El juego no saca
//! mensajes con `PeekMessageW` de su tabla de importaciones: los de la ventana los frena el
//! filtro de hudhook.
//!
//! El mando (XInput) se bloquea en [`pad`](crate::pad), con el mismo criterio.
//!
//! Al cerrar la forja se sigue bloqueando hasta que se suelten todas las teclas y botones, también
//! los del mando (o 1,5 s), para que la tecla o el clic con que se cerró no le llegue al juego.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::win;

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

const IID_IDIRECTINPUT8A: Guid = Guid(
    0xBF79_8030,
    0x483A,
    0x4DA2,
    [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00],
);
const IID_IDIRECTINPUT8W: Guid = Guid(
    0xBF79_8031,
    0x483A,
    0x4DA2,
    [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00],
);
const GUID_SYS_KEYBOARD: Guid = Guid(
    0x6F1D_2B61,
    0xD5A0,
    0x11CF,
    [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00],
);
const GUID_SYS_MOUSE: Guid = Guid(
    0x6F1D_2B60,
    0xD5A0,
    0x11CF,
    [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00],
);
const DIRECTINPUT_VERSION: u32 = 0x0800;
const RELEASE_TIMEOUT: Duration = Duration::from_millis(1500);

type Create =
    unsafe extern "system" fn(*mut c_void, u32, *const Guid, *mut *mut c_void, *mut c_void) -> i32;
type CreateDevice =
    unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void, *mut c_void) -> i32;
type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type GetState = unsafe extern "system" fn(*mut c_void, u32, *mut u8) -> i32;
type GetData = unsafe extern "system" fn(*mut c_void, u32, *mut c_void, *mut u32, u32) -> i32;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

/// La forja está abierta.
static MENU_OPEN: AtomicBool = AtomicBool::new(false);
/// Se cerró la forja pero todavía hay teclas apretadas: (desde cuándo).
static RELEASING: Mutex<Option<Instant>> = Mutex::new(None);

// Originales, por vtable de dispositivo reemplazada (teclado, mouse, joystick... y variante
// ANSI o Unicode). Si dos comparten vtable, se reemplaza una sola vez.
const SLOTS: usize = 8;
static ORIG_STATE: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
static ORIG_DATA: [AtomicUsize; SLOTS] = [const { AtomicUsize::new(0) }; SLOTS];
/// Vtables de dispositivo ya reemplazadas (la posición es el slot).
static PATCHED: Mutex<Vec<usize>> = Mutex::new(Vec::new());
/// `IDirectInput8::CreateDevice` original, de la interfaz ANSI y de la Unicode.
static ORIG_CREATE: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];

/// Con la forja recién cerrada, el teclado de DirectInput ya no tiene teclas apretadas.
static KEYS_UP: AtomicBool = AtomicBool::new(false);

/// Llamar en cada frame con el estado de la forja.
pub fn set_menu_open(open: bool) {
    let was = MENU_OPEN.swap(open, Ordering::Relaxed);
    if !was && open {
        freeze_cursor();
    }
    if was && !open {
        KEYS_UP.store(false, Ordering::Relaxed);
        *RELEASING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
        log_stats();
    }
}

/// Si el juego no tiene que ver teclado, mouse ni mando: con la forja abierta, y al cerrarla
/// hasta que se suelte todo.
pub(crate) fn blocked() -> bool {
    if MENU_OPEN.load(Ordering::Relaxed) {
        return true;
    }
    let mut releasing = RELEASING.lock().unwrap_or_else(|e| e.into_inner());
    match *releasing {
        Some(at) if at.elapsed() < RELEASE_TIMEOUT => {
            if KEYS_UP.load(Ordering::Relaxed) && mouse_released() && crate::pad::released() {
                *releasing = None;
                false
            } else {
                true
            }
        }
        Some(_) => {
            *releasing = None;
            false
        }
        None => false,
    }
}

/// Para el log: la primera vez que el juego lee un dispositivo de DirectInput de cada forma
/// (vtable enganchada, función y tamaño del dato).
fn log_new_read(slot: usize, func: &'static str, cb: u32) {
    static SEEN: Mutex<Vec<(usize, &'static str, u32)>> = Mutex::new(Vec::new());
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if seen.len() < 32 && !seen.contains(&(slot, func, cb)) {
        seen.push((slot, func, cb));
        crate::log_event(&format!(
            "DirectInput: el juego lee con {func} ({cb} bytes, vtable {slot})"
        ));
    }
}

/// Con la forja recién cerrada: anota si el teclado ya no tiene teclas apretadas.
fn keyboard_released(cb: u32, data: *const u8) {
    if cb != 256 || MENU_OPEN.load(Ordering::Relaxed) {
        return;
    }
    // SAFETY: el original acaba de llenar `cb` bytes en `data`.
    let keys = unsafe { std::slice::from_raw_parts(data, 256) };
    if keys.iter().all(|k| k & 0x80 == 0) {
        KEYS_UP.store(true, Ordering::Relaxed);
    }
}

unsafe fn state_hook(slot: usize, this: *mut c_void, cb: u32, data: *mut u8) -> i32 {
    // SAFETY: guardado al instalar, antes de reemplazar la entrada de la vtable.
    let orig: GetState = std::mem::transmute(ORIG_STATE[slot].load(Ordering::Relaxed));
    let result = orig(this, cb, data);
    if result >= 0 {
        log_new_read(slot, "GetDeviceState", cb);
    }
    // Joystick (DIJOYSTATE o DIJOYSTATE2): un mando que no es de XInput, como los de PlayStation.
    // No se lo pone en cero: los ejes en 0 son un stick apretado a fondo.
    if result >= 0 && !data.is_null() && (cb == 80 || cb == 272) {
        crate::pad::joystick_read(this, cb, data, blocked());
        return result;
    }
    if result >= 0 && !data.is_null() && blocked() {
        keyboard_released(cb, data);
        match cb {
            256 => count(STAT_DI_KEYBOARD),
            // DIMOUSESTATE y DIMOUSESTATE2.
            16 | 20 => count(STAT_DI_MOUSE),
            _ => {}
        }
        std::ptr::write_bytes(data, 0, cb as usize);
    }
    result
}

unsafe fn data_hook(
    slot: usize,
    this: *mut c_void,
    cb: u32,
    items: *mut c_void,
    count_ptr: *mut u32,
    flags: u32,
) -> i32 {
    // SAFETY: guardado al instalar, antes de reemplazar la entrada de la vtable.
    let orig: GetData = std::mem::transmute(ORIG_DATA[slot].load(Ordering::Relaxed));
    let result = orig(this, cb, items, count_ptr, flags);
    if result >= 0 && !count_ptr.is_null() && *count_ptr > 0 {
        log_new_read(slot, "GetDeviceData", cb);
    }
    if result >= 0 && !count_ptr.is_null() && blocked() {
        STATS[STAT_DI_EVENTS].fetch_add(*count_ptr, Ordering::Relaxed);
        *count_ptr = 0;
    }
    result
}

macro_rules! di_hooks {
    ($($slot:literal => $state:ident, $data:ident;)*) => {
        $(
            unsafe extern "system" fn $state(this: *mut c_void, cb: u32, data: *mut u8) -> i32 {
                state_hook($slot, this, cb, data)
            }
            unsafe extern "system" fn $data(
                this: *mut c_void,
                cb: u32,
                items: *mut c_void,
                count: *mut u32,
                flags: u32,
            ) -> i32 {
                data_hook($slot, this, cb, items, count, flags)
            }
        )*
        fn di_hooks() -> [(usize, usize); SLOTS] {
            [$(($state as GetState as usize, $data as GetData as usize)),*]
        }
    };
}

di_hooks! {
    0 => state_0, data_0;
    1 => state_1, data_1;
    2 => state_2, data_2;
    3 => state_3, data_3;
    4 => state_4, data_4;
    5 => state_5, data_5;
    6 => state_6, data_6;
    7 => state_7, data_7;
}

/// Reemplaza `GetDeviceState` (9) y `GetDeviceData` (10) en la vtable de un dispositivo, si no
/// se hizo antes. `true` si era nueva.
unsafe fn hook_device_vtable(vtable: usize) -> Result<bool, String> {
    let mut patched = PATCHED.lock().unwrap_or_else(|e| e.into_inner());
    if patched.contains(&vtable) {
        return Ok(false);
    }
    let slot = patched.len();
    if slot >= SLOTS {
        return Err("no quedan ganchos libres para otra vtable de DirectInput".into());
    }
    let (state, data) = di_hooks()[slot];
    patch(vtable, 9, state, &ORIG_STATE[slot])?;
    patch(vtable, 10, data, &ORIG_DATA[slot])?;
    patched.push(vtable);
    Ok(true)
}

/// `CreateDevice` del juego: llama al original y engancha la vtable del dispositivo nuevo (así
/// se cubren el joystick y los mandos que se conectan con el juego abierto).
unsafe fn create_device_hook(
    variant: usize,
    this: *mut c_void,
    guid: *const Guid,
    out: *mut *mut c_void,
    outer: *mut c_void,
) -> i32 {
    // SAFETY: guardado al instalar, antes de reemplazar la entrada.
    let orig: CreateDevice = std::mem::transmute(ORIG_CREATE[variant].load(Ordering::Relaxed));
    let hr = orig(this, guid, out, outer);
    if hr >= 0 && !out.is_null() && !(*out).is_null() {
        match hook_device_vtable(*(*out as *const usize)) {
            Ok(true) => crate::log_event("DirectInput: el juego creó un dispositivo de otro tipo; enganchado"),
            Ok(false) => {}
            Err(e) => crate::log_event(&format!("ERROR: DirectInput: {e}")),
        }
    }
    hr
}

unsafe extern "system" fn create_device_a(
    this: *mut c_void,
    guid: *const Guid,
    out: *mut *mut c_void,
    outer: *mut c_void,
) -> i32 {
    create_device_hook(0, this, guid, out, outer)
}

unsafe extern "system" fn create_device_w(
    this: *mut c_void,
    guid: *const Guid,
    out: *mut *mut c_void,
    outer: *mut c_void,
) -> i32 {
    create_device_hook(1, this, guid, out, outer)
}

type EnumDevices = unsafe extern "system" fn(
    *mut c_void,
    u32,
    unsafe extern "system" fn(*const u8, *mut c_void) -> i32,
    *mut c_void,
    u32,
) -> i32;

/// `EnumDevices`: guarda el `guidInstance` del primer mando en `found` y corta.
unsafe extern "system" fn first_device(instance: *const u8, found: *mut c_void) -> i32 {
    // DIDEVICEINSTANCE: dwSize y después guidInstance.
    *(found as *mut Option<Guid>) = Some(std::ptr::read_unaligned(instance.add(4) as *const Guid));
    0 // DIENUM_STOP
}

/// Reemplaza la entrada `index` de `vtable`, guardando antes la original en `orig` (el gancho
/// puede llamarse apenas se escribe la entrada).
pub(crate) unsafe fn patch(
    vtable: usize,
    index: usize,
    hook: usize,
    orig: &AtomicUsize,
) -> Result<(), String> {
    let slot = vtable + index * 8;
    let original: usize = win::read(slot).ok_or("no puedo leer la vtable de DirectInput")?;
    orig.store(original, Ordering::SeqCst);
    let mut old = 0;
    if win::VirtualProtect(slot as *mut _, 8, 0x04, &mut old) == 0 {
        return Err("VirtualProtect falló en la vtable de DirectInput".into());
    }
    std::ptr::write_volatile(slot as *mut usize, hook);
    win::VirtualProtect(slot as *mut _, 8, old, &mut old);
    Ok(())
}

/// Vtable de un dispositivo de DirectInput creado con la interfaz `di` (lo crea y lo suelta).
unsafe fn device_vtable(di: *mut c_void, device_guid: &Guid) -> Result<usize, String> {
    let di_vtable = *(di as *const *const usize);
    let create_device: CreateDevice = std::mem::transmute(*di_vtable.add(3));
    let mut device: *mut c_void = std::ptr::null_mut();
    let hr = create_device(di, device_guid, &mut device, std::ptr::null_mut());
    if hr < 0 || device.is_null() {
        return Err(format!("CreateDevice falló ({hr:#X})"));
    }
    let vtable = *(device as *const usize);
    let release_device: Release = std::mem::transmute(*(vtable as *const usize).add(2));
    release_device(device);
    Ok(vtable)
}

/// Crea una interfaz de DirectInput 8 (`iid`: ANSI o Unicode).
unsafe fn direct_input(create: Create, iid: &Guid) -> Result<*mut c_void, String> {
    let mut di: *mut c_void = std::ptr::null_mut();
    let hinst = GetModuleHandleW(std::ptr::null());
    let hr = create(hinst, DIRECTINPUT_VERSION, iid, &mut di, std::ptr::null_mut());
    if hr < 0 || di.is_null() {
        return Err(format!("DirectInput8Create falló ({hr:#X})"));
    }
    Ok(di)
}

/// Reemplaza `GetDeviceState` y `GetDeviceData` de los dispositivos de DirectInput: teclado,
/// mouse y el primer mando conectado (si hay), y `CreateDevice` para los que el juego cree
/// después. Devuelve cuántas vtables de dispositivo reemplazó.
pub fn install() -> Result<usize, String> {
    let name: Vec<u16> = "dinput8.dll".encode_utf16().chain([0]).collect();
    // SAFETY: carga de una DLL del sistema que el juego ya usa; se buscan funciones conocidas y
    // se reemplazan dos entradas de vtable por funciones con la misma firma.
    unsafe {
        let module = LoadLibraryW(name.as_ptr());
        if module.is_null() {
            return Err("no pude cargar dinput8.dll".into());
        }
        let f = GetProcAddress(module, c"DirectInput8Create".as_ptr().cast());
        if f.is_null() {
            return Err("dinput8.dll sin DirectInput8Create".into());
        }
        let create: Create = std::mem::transmute(f);
        let creates: [unsafe extern "system" fn(_, _, _, _) -> i32; 2] =
            [create_device_a, create_device_w];
        for (variant, iid) in [&IID_IDIRECTINPUT8A, &IID_IDIRECTINPUT8W]
            .into_iter()
            .enumerate()
        {
            let di = direct_input(create, iid)?;
            let di_vtable = *(di as *const *const usize);
            let release_di: Release = std::mem::transmute(*di_vtable.add(2));
            let mut devices = vec![GUID_SYS_KEYBOARD, GUID_SYS_MOUSE];
            // El primer mando conectado (DI8DEVCLASS_GAMECTRL, DIEDFL_ATTACHEDONLY).
            let enum_devices: EnumDevices = std::mem::transmute(*di_vtable.add(4));
            let mut pad: Option<Guid> = None;
            enum_devices(di, 4, first_device, (&mut pad as *mut Option<Guid>).cast(), 1);
            devices.extend(pad);
            let result = devices.iter().try_for_each(|guid| {
                hook_device_vtable(device_vtable(di, guid)?).map(|_| ())
            });
            // CreateDevice de esta variante, para los dispositivos que cree el juego.
            let result = result.and_then(|()| {
                patch(
                    di_vtable as usize,
                    3,
                    creates[variant] as usize,
                    &ORIG_CREATE[variant],
                )
            });
            release_di(di);
            result?;
        }
        Ok(PATCHED.lock().unwrap_or_else(|e| e.into_inner()).len())
    }
}

// ---------------------------------------------------------------------------------------------
// Win32: lo que el juego lee sin DirectInput.

/// VK_LBUTTON, VK_RBUTTON, VK_MBUTTON, VK_XBUTTON1, VK_XBUTTON2.
const MOUSE_BUTTONS: [i32; 5] = [0x01, 0x02, 0x04, 0x05, 0x06];

#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vk: i32) -> i16;
    #[link_name = "GetCursorPos"]
    fn RealGetCursorPos(point: *mut [i32; 2]) -> i32;
}

type GetKeyStateFn = unsafe extern "system" fn(i32) -> i16;
type GetKeyboardStateFn = unsafe extern "system" fn(*mut u8) -> i32;
type GetCursorPosFn = unsafe extern "system" fn(*mut [i32; 2]) -> i32;

static ORIG_KEY_STATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_KEYBOARD_STATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_CURSOR_POS: AtomicUsize = AtomicUsize::new(0);

/// Posición del cursor al abrir la forja (x en los 32 bits altos, y en los bajos).
static FROZEN_CURSOR: AtomicI64 = AtomicI64::new(0);

/// Cuántas veces se bloqueó cada cosa desde que se abrió la forja (para el log).
static STATS: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];
const STAT_KEY_STATE: usize = 0;
const STAT_KEYBOARD_STATE: usize = 1;
const STAT_CURSOR: usize = 2;
const STAT_DI_KEYBOARD: usize = 3;
const STAT_DI_MOUSE: usize = 4;
const STAT_DI_EVENTS: usize = 5;

fn count(stat: usize) {
    STATS[stat].fetch_add(1, Ordering::Relaxed);
}

fn log_stats() {
    let n: Vec<u32> = STATS.iter().map(|s| s.swap(0, Ordering::Relaxed)).collect();
    let (xinput, joystick) = crate::pad::take_stats();
    let hid = crate::hid::take_reports();
    crate::log_event(&format!(
        "Forja cerrada. Con la forja abierta se le ocultaron al juego: DirectInput {} lecturas \
         de teclado, {} de mouse y {} eventos; {} GetKeyState, {} GetKeyboardState, \
         {} GetCursorPos. Lecturas de mando desde la última vez: {xinput} XInput (conectado), \
         {joystick} joystick de DirectInput, {hid} reportes HID",
        n[STAT_DI_KEYBOARD],
        n[STAT_DI_MOUSE],
        n[STAT_DI_EVENTS],
        n[STAT_KEY_STATE],
        n[STAT_KEYBOARD_STATE],
        n[STAT_CURSOR]
    ));
}

fn mouse_released() -> bool {
    // SAFETY: función de user32 sin efectos.
    MOUSE_BUTTONS
        .iter()
        .all(|&vk| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 == 0)
}

fn freeze_cursor() {
    let mut p = [0i32; 2];
    // SAFETY: escribe en nuestro buffer.
    if unsafe { RealGetCursorPos(&mut p) } != 0 {
        FROZEN_CURSOR.store(
            ((p[0] as i64) << 32) | (p[1] as u32 as i64),
            Ordering::Relaxed,
        );
    }
}

unsafe extern "system" fn key_state_hook(vk: i32) -> i16 {
    // SAFETY: guardado al instalar.
    let orig: GetKeyStateFn = std::mem::transmute(ORIG_KEY_STATE.load(Ordering::Relaxed));
    let state = orig(vk);
    if blocked() {
        count(STAT_KEY_STATE);
        return 0;
    }
    state
}

unsafe extern "system" fn keyboard_state_hook(keys: *mut u8) -> i32 {
    // SAFETY: guardado al instalar.
    let orig: GetKeyboardStateFn = std::mem::transmute(ORIG_KEYBOARD_STATE.load(Ordering::Relaxed));
    let ok = orig(keys);
    if ok != 0 && !keys.is_null() && blocked() {
        std::ptr::write_bytes(keys, 0, 256);
        count(STAT_KEYBOARD_STATE);
    }
    ok
}

unsafe extern "system" fn cursor_pos_hook(point: *mut [i32; 2]) -> i32 {
    // SAFETY: guardado al instalar.
    let orig: GetCursorPosFn = std::mem::transmute(ORIG_CURSOR_POS.load(Ordering::Relaxed));
    let ok = orig(point);
    if ok != 0 && !point.is_null() && blocked() {
        let p = FROZEN_CURSOR.load(Ordering::Relaxed);
        *point = [(p >> 32) as i32, p as i32];
        count(STAT_CURSOR);
    }
    ok
}

fn c_string(addr: usize) -> Option<String> {
    let (bytes, bad) = win::read_bytes(addr, 64);
    if bad > 0 {
        return None;
    }
    let end = bytes.iter().position(|&b| b == 0)?;
    Some(String::from_utf8_lossy(&bytes[..end]).into_owned())
}

/// Una función importada: por nombre o por ordinal.
#[derive(Clone, Copy)]
enum Import<'a> {
    Name(&'a str),
    Ordinal(u16),
}

/// Dirección de la entrada de la tabla de importaciones de `module` para `dll!func`.
fn iat_slot(module: usize, dll: &str, func: &str) -> Option<usize> {
    iat_slot_of(module, dll, Import::Name(func))
}

pub(crate) fn iat_slot_pub(module: usize, dll: &str, func: &str) -> Option<usize> {
    iat_slot(module, dll, func)
}

/// Dirección de la entrada de la tabla de importaciones de `module` para `dll!#ordinal`.
pub(crate) fn iat_slot_ordinal(module: usize, dll: &str, ordinal: u16) -> Option<usize> {
    iat_slot_of(module, dll, Import::Ordinal(ordinal))
}

fn iat_slot_of(module: usize, dll: &str, func: Import) -> Option<usize> {
    let pe = module + win::read::<u32>(module + 0x3C)? as usize;
    // Directorio de importaciones: entrada 1 de los data directories de un PE32+.
    let imports = win::read::<u32>(pe + 24 + 112 + 8)? as usize;
    let mut desc = module + imports;
    loop {
        let names = win::read::<u32>(desc)? as usize;
        let name = win::read::<u32>(desc + 12)? as usize;
        let thunks = win::read::<u32>(desc + 16)? as usize;
        if name == 0 {
            return None;
        }
        if names != 0 && c_string(module + name)?.eq_ignore_ascii_case(dll) {
            for k in 0.. {
                let entry = win::read::<u64>(module + names + 8 * k)?;
                if entry == 0 {
                    break;
                }
                let found = match func {
                    Import::Ordinal(n) => entry >> 63 == 1 && entry as u16 == n,
                    Import::Name(name) => {
                        entry >> 63 == 0
                            && c_string(module + (entry as usize & 0x7FFF_FFFF) + 2)? == name
                    }
                };
                if found {
                    return Some(module + thunks + 8 * k);
                }
            }
        }
        desc += 20;
    }
}

/// Reemplaza las importaciones de user32 del juego. Devuelve las que no pudo reemplazar.
pub fn install_win32(base: usize) -> Vec<&'static str> {
    let hooks: [(&'static str, usize, &AtomicUsize); 3] = [
        (
            "GetKeyState",
            key_state_hook as GetKeyStateFn as usize,
            &ORIG_KEY_STATE,
        ),
        (
            "GetKeyboardState",
            keyboard_state_hook as GetKeyboardStateFn as usize,
            &ORIG_KEYBOARD_STATE,
        ),
        (
            "GetCursorPos",
            cursor_pos_hook as GetCursorPosFn as usize,
            &ORIG_CURSOR_POS,
        ),
    ];
    let mut missing = Vec::new();
    for (func, hook, orig) in hooks {
        let Some(slot) = iat_slot(base, "user32.dll", func) else {
            missing.push(func);
            continue;
        };
        // SAFETY: la entrada es un puntero a función con la misma firma que el gancho; se guarda
        // la original antes de escribir.
        if unsafe { patch(slot, 0, hook, orig) }.is_err() {
            missing.push(func);
        }
    }
    missing
}
