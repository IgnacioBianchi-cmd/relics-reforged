//! Mando (XInput): el juego no lo tiene que ver con la forja abierta, y la forja lo usa para
//! navegar.
//!
//! El juego lee el mando con `XInputGetState` de `XINPUT1_4.dll`, importada por ordinal (2) en
//! `nightreign.exe`. Esa entrada de la tabla de importaciones se reemplaza: cuando
//! [`input_block`](crate::input_block) bloquea, el juego recibe el mando suelto (mismo número de
//! paquete, sin botones ni sticks). Mientras la forja se ofrece en Relic Rites, el juego tampoco
//! ve el botón que la abre ([`OPEN_BUTTON`]).
//!
//! La forja lee el mando con la función original (no pasa por la tabla del juego), en el hilo de
//! render. Un mando desconectado tarda en responder, así que solo se vuelve a buscar cada 2 s.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const DPAD_UP: u16 = 0x0001;
pub const DPAD_DOWN: u16 = 0x0002;
pub const DPAD_LEFT: u16 = 0x0004;
pub const DPAD_RIGHT: u16 = 0x0008;
pub const START: u16 = 0x0010;
pub const BACK: u16 = 0x0020;
pub const RIGHT_THUMB: u16 = 0x0080;
pub const RIGHT_SHOULDER: u16 = 0x0200;
pub const A: u16 = 0x1000;
pub const B: u16 = 0x2000;
pub const X: u16 = 0x4000;
pub const Y: u16 = 0x8000;

/// Botón que abre la forja desde Relic Rites (click del stick derecho).
pub const OPEN_BUTTON: u16 = RIGHT_THUMB;

/// Sticks y gatillos por encima de esto cuentan como apretados.
const STICK_THRESHOLD: i16 = 16_000;
const TRIGGER_THRESHOLD: u8 = 60;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct XInputState {
    packet: u32,
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    lx: i16,
    ly: i16,
    rx: i16,
    ry: i16,
}

type GetStateFn = unsafe extern "system" fn(u32, *mut XInputState) -> u32;

#[link(name = "kernel32")]
extern "system" {
    fn LoadLibraryW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
}

/// `XInputGetState` original (el de la tabla del juego antes del gancho).
static ORIG_GET_STATE: AtomicUsize = AtomicUsize::new(0);
/// La forja se ofrece en Relic Rites: el juego no ve [`OPEN_BUTTON`].
static OFFERED: AtomicBool = AtomicBool::new(false);
/// Mandos (bit por índice 0-3) con algo apretado, según la última lectura del juego.
static HELD: AtomicU8 = AtomicU8::new(0);

/// Lo que el juego tiene apretado en el mando: con la forja recién cerrada se lo sigue
/// bloqueando hasta que se suelte todo.
pub fn released() -> bool {
    HELD.load(Ordering::Relaxed) == 0
}

pub fn set_offered(offered: bool) {
    OFFERED.store(offered, Ordering::Relaxed);
}

fn pressed_anything(g: &XInputState) -> bool {
    g.buttons != 0
        || g.left_trigger > TRIGGER_THRESHOLD
        || g.right_trigger > TRIGGER_THRESHOLD
        || [g.lx, g.ly, g.rx, g.ry]
            .iter()
            .any(|v| v.unsigned_abs() > STICK_THRESHOLD as u16)
}

unsafe extern "system" fn get_state_hook(user: u32, state: *mut XInputState) -> u32 {
    // SAFETY: guardado al instalar, antes de escribir la entrada.
    let orig: GetStateFn = std::mem::transmute(ORIG_GET_STATE.load(Ordering::Relaxed));
    let result = orig(user, state);
    if user < 4 {
        let bit = 1u8 << user;
        if result == 0 && !state.is_null() && pressed_anything(&*state) {
            HELD.fetch_or(bit, Ordering::Relaxed);
        } else {
            HELD.fetch_and(!bit, Ordering::Relaxed);
        }
    }
    if result == 0 && !state.is_null() {
        if crate::input_block::blocked() {
            let packet = (*state).packet;
            *state = XInputState {
                packet,
                ..Default::default()
            };
        } else if OFFERED.load(Ordering::Relaxed) {
            (*state).buttons &= !OPEN_BUTTON;
        }
    }
    result
}

/// Reemplaza `XInputGetState` en la tabla de importaciones del juego.
pub fn install(base: usize) -> Result<(), String> {
    let slot = crate::input_block::iat_slot_ordinal(base, "xinput1_4.dll", 2)
        .ok_or("nightreign.exe no importa XInputGetState (xinput1_4.dll #2)")?;
    // SAFETY: la entrada es un puntero a `XInputGetState`, con la misma firma que el gancho; se
    // guarda la original antes de escribir.
    unsafe {
        crate::input_block::patch(
            slot,
            0,
            get_state_hook as GetStateFn as usize,
            &ORIG_GET_STATE,
        )
    }
}

/// La función original para las lecturas propias: la de la tabla del juego si ya se instaló el
/// gancho, o la de la DLL.
fn real_get_state() -> Option<GetStateFn> {
    let mut f = ORIG_GET_STATE.load(Ordering::Relaxed);
    if f == 0 {
        let name: Vec<u16> = "xinput1_4.dll".encode_utf16().chain([0]).collect();
        // SAFETY: carga una DLL del sistema que el juego ya tiene cargada.
        unsafe {
            let module = LoadLibraryW(name.as_ptr());
            if module.is_null() {
                return None;
            }
            f = GetProcAddress(module, c"XInputGetState".as_ptr().cast()) as usize;
        }
    }
    // SAFETY: `f` es `XInputGetState`.
    (f != 0).then(|| unsafe { std::mem::transmute::<usize, GetStateFn>(f) })
}

/// Lo que se lee del mando en un frame, con los sticks pasados a direcciones.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Pad {
    /// Botones de XInput; el stick izquierdo se suma a las direcciones del D-pad.
    pub buttons: u16,
    /// Se movió algo (botón, stick o gatillo) desde la lectura anterior.
    pub touched: bool,
}

struct Reader {
    user: Option<u32>,
    next_scan: Option<Instant>,
    last: XInputState,
}

static READER: Mutex<Reader> = Mutex::new(Reader {
    user: None,
    next_scan: None,
    last: XInputState {
        packet: 0,
        buttons: 0,
        left_trigger: 0,
        right_trigger: 0,
        lx: 0,
        ly: 0,
        rx: 0,
        ry: 0,
    },
});

/// Lee el primer mando conectado. `None` si no hay ninguno.
pub fn read() -> Option<Pad> {
    let get = real_get_state()?;
    let mut r = READER.lock().unwrap_or_else(|e| e.into_inner());
    let mut state = XInputState::default();
    // SAFETY: `state` es un XINPUT_STATE válido.
    let ok = |user: u32, state: &mut XInputState| unsafe { get(user, state) } == 0;
    if !r.user.is_some_and(|u| ok(u, &mut state)) {
        r.user = None;
        if r.next_scan.is_some_and(|t| Instant::now() < t) {
            return None;
        }
        r.next_scan = Some(Instant::now() + Duration::from_secs(2));
        r.user = Some((0..4).find(|&u| ok(u, &mut state))?);
    }
    let mut buttons = state.buttons;
    if state.lx < -STICK_THRESHOLD {
        buttons |= DPAD_LEFT;
    }
    if state.lx > STICK_THRESHOLD {
        buttons |= DPAD_RIGHT;
    }
    if state.ly > STICK_THRESHOLD {
        buttons |= DPAD_UP;
    }
    if state.ly < -STICK_THRESHOLD {
        buttons |= DPAD_DOWN;
    }
    let before = r.last;
    r.last = state;
    let moved = |a: i16, b: i16| (a as i32 - b as i32).abs() > 4_000;
    let touched = state.buttons != before.buttons
        || moved(state.lx, before.lx)
        || moved(state.ly, before.ly)
        || moved(state.rx, before.rx)
        || moved(state.ry, before.ry)
        || state.left_trigger.abs_diff(before.left_trigger) > 40
        || state.right_trigger.abs_diff(before.right_trigger) > 40;
    Some(Pad { buttons, touched })
}
