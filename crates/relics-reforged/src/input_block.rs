//! Con la forja abierta, el teclado y el mouse no le tienen que llegar al juego.
//!
//! El juego lee el teclado y el mouse con DirectInput 8, no con los mensajes de la ventana, así
//! que el filtro de mensajes de hudhook no alcanza: sin esto, el "4" abre el filtro de Relic Rites
//! detrás de la forja, el "2" marca favoritas y el "3" guarda un preset.
//!
//! Se crea un dispositivo propio para conseguir la vtable de los dispositivos de `dinput8.dll`
//! (la comparten todos los del mismo tipo, también los del juego) y se reemplazan
//! `GetDeviceState` (9) y `GetDeviceData` (10): con la forja abierta llaman al original (así
//! el dispositivo sigue al día) y después devuelven "nada apretado" y "sin eventos". Se hace para
//! la versión ANSI y la Unicode de la interfaz.
//!
//! Al cerrar la forja se sigue bloqueando hasta que se suelten todas las teclas (o 1,5 s), para
//! que la tecla con la que se cerró no le llegue al juego.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

// Originales, por variante de la interfaz (0 = ANSI, 1 = Unicode).
static ORIG_STATE: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];
static ORIG_DATA: [AtomicUsize; 2] = [AtomicUsize::new(0), AtomicUsize::new(0)];

/// Llamar en cada frame con el estado de la forja.
pub fn set_menu_open(open: bool) {
    let was = MENU_OPEN.swap(open, Ordering::Relaxed);
    if was && !open {
        *RELEASING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    }
}

fn blocked() -> bool {
    if MENU_OPEN.load(Ordering::Relaxed) {
        return true;
    }
    let mut releasing = RELEASING.lock().unwrap_or_else(|e| e.into_inner());
    match *releasing {
        Some(at) if at.elapsed() < RELEASE_TIMEOUT => true,
        Some(_) => {
            *releasing = None;
            false
        }
        None => false,
    }
}

/// Con la forja recién cerrada: si el teclado ya no tiene teclas apretadas, se deja de bloquear.
fn keyboard_released(cb: u32, data: *const u8) {
    if cb != 256 || MENU_OPEN.load(Ordering::Relaxed) {
        return;
    }
    // SAFETY: el original acaba de llenar `cb` bytes en `data`.
    let keys = unsafe { std::slice::from_raw_parts(data, 256) };
    if keys.iter().all(|k| k & 0x80 == 0) {
        *RELEASING.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

unsafe fn state_hook(variant: usize, this: *mut c_void, cb: u32, data: *mut u8) -> i32 {
    // SAFETY: guardado al instalar, antes de reemplazar la entrada de la vtable.
    let orig: GetState = std::mem::transmute(ORIG_STATE[variant].load(Ordering::Relaxed));
    let result = orig(this, cb, data);
    if result >= 0 && !data.is_null() && blocked() {
        keyboard_released(cb, data);
        std::ptr::write_bytes(data, 0, cb as usize);
    }
    result
}

unsafe fn data_hook(
    variant: usize,
    this: *mut c_void,
    cb: u32,
    items: *mut c_void,
    count: *mut u32,
    flags: u32,
) -> i32 {
    // SAFETY: guardado al instalar, antes de reemplazar la entrada de la vtable.
    let orig: GetData = std::mem::transmute(ORIG_DATA[variant].load(Ordering::Relaxed));
    let result = orig(this, cb, items, count, flags);
    if result >= 0 && !count.is_null() && blocked() {
        *count = 0;
    }
    result
}

unsafe extern "system" fn state_a(this: *mut c_void, cb: u32, data: *mut u8) -> i32 {
    state_hook(0, this, cb, data)
}
unsafe extern "system" fn state_w(this: *mut c_void, cb: u32, data: *mut u8) -> i32 {
    state_hook(1, this, cb, data)
}
unsafe extern "system" fn data_a(
    this: *mut c_void,
    cb: u32,
    items: *mut c_void,
    count: *mut u32,
    flags: u32,
) -> i32 {
    data_hook(0, this, cb, items, count, flags)
}
unsafe extern "system" fn data_w(
    this: *mut c_void,
    cb: u32,
    items: *mut c_void,
    count: *mut u32,
    flags: u32,
) -> i32 {
    data_hook(1, this, cb, items, count, flags)
}

/// Reemplaza la entrada `index` de `vtable`, guardando antes la original en `orig` (el gancho
/// puede llamarse apenas se escribe la entrada).
unsafe fn patch(
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

/// Vtable de los dispositivos de una variante (crea y suelta un teclado propio).
unsafe fn device_vtable(create: Create, iid: &Guid) -> Result<usize, String> {
    let mut di: *mut c_void = std::ptr::null_mut();
    let hinst = GetModuleHandleW(std::ptr::null());
    let hr = create(
        hinst,
        DIRECTINPUT_VERSION,
        iid,
        &mut di,
        std::ptr::null_mut(),
    );
    if hr < 0 || di.is_null() {
        return Err(format!("DirectInput8Create falló ({hr:#X})"));
    }
    let di_vtable = *(di as *const *const usize);
    let create_device: CreateDevice = std::mem::transmute(*di_vtable.add(3));
    let release_di: Release = std::mem::transmute(*di_vtable.add(2));
    let mut device: *mut c_void = std::ptr::null_mut();
    let hr = create_device(di, &GUID_SYS_KEYBOARD, &mut device, std::ptr::null_mut());
    if hr < 0 || device.is_null() {
        release_di(di);
        return Err(format!("CreateDevice falló ({hr:#X})"));
    }
    let vtable = *(device as *const usize);
    let release_device: Release = std::mem::transmute(*(vtable as *const usize).add(2));
    release_device(device);
    release_di(di);
    Ok(vtable)
}

pub fn install() -> Result<(), String> {
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
        let hooks: [(&Guid, usize, usize); 2] = [
            (
                &IID_IDIRECTINPUT8A,
                state_a as GetState as usize,
                data_a as GetData as usize,
            ),
            (
                &IID_IDIRECTINPUT8W,
                state_w as GetState as usize,
                data_w as GetData as usize,
            ),
        ];
        let mut done: Vec<usize> = Vec::new();
        for (variant, (iid, state, data)) in hooks.into_iter().enumerate() {
            let vtable = device_vtable(create, iid)?;
            if done.contains(&vtable) {
                continue;
            }
            patch(vtable, 9, state, &ORIG_STATE[variant])?;
            patch(vtable, 10, data, &ORIG_DATA[variant])?;
            done.push(vtable);
        }
    }
    Ok(())
}
