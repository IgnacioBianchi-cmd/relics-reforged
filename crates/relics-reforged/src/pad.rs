//! Mando: el juego no lo tiene que ver con la forja abierta, y la forja lo usa para navegar.
//!
//! El juego lee los mandos por dos caminos:
//! - **XInput** (mandos de Xbox, o cualquiera que Steam Input presente como uno): `XInputGetState`
//!   de `XINPUT1_4.dll`, importada por ordinal (2). Esa entrada de la tabla de importaciones se
//!   reemplaza: cuando [`input_block`](crate::input_block) bloquea, el juego recibe el mando
//!   suelto (mismo número de paquete, sin botones ni sticks).
//! - **DirectInput** (los demás, como los de PlayStation sin Steam Input): un joystick con
//!   `GetDeviceState` de `DIJOYSTATE` (80 bytes) o `DIJOYSTATE2` (272). El gancho de DirectInput
//!   de `input_block` ya pasa por ahí (la vtable es la misma que la del teclado y el mouse) y le
//!   deja cada lectura a [`joystick_read`]: guarda una copia para la forja y, si hay que bloquear,
//!   le devuelve al juego el mando en reposo.
//!
//! La forja lee XInput con la función original (no pasa por la tabla del juego) y el joystick de
//! la última lectura del juego. Un mando de XInput desconectado tarda en responder, así que solo
//! se vuelve a buscar cada 2 s.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// Botones con los bits de XInput. Los gatillos no son botones en XInput: usan bits libres.
pub const DPAD_UP: u16 = 0x0001;
pub const DPAD_DOWN: u16 = 0x0002;
pub const DPAD_LEFT: u16 = 0x0004;
pub const DPAD_RIGHT: u16 = 0x0008;
pub const START: u16 = 0x0010;
pub const BACK: u16 = 0x0020;
pub const LEFT_THUMB: u16 = 0x0040;
pub const RIGHT_THUMB: u16 = 0x0080;
pub const LEFT_SHOULDER: u16 = 0x0100;
pub const RIGHT_SHOULDER: u16 = 0x0200;
pub const LEFT_TRIGGER: u16 = 0x0400;
pub const RIGHT_TRIGGER: u16 = 0x0800;
pub const A: u16 = 0x1000;
pub const B: u16 = 0x2000;
pub const X: u16 = 0x4000;
pub const Y: u16 = 0x8000;

/// Botón que abre la forja desde Relic Rites: R2 (RT). Relic Rites no usa los gatillos.
pub const OPEN_BUTTON: u16 = RIGHT_TRIGGER;

/// Qué mando es, para mostrar sus botones.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Kind {
    #[default]
    Xbox,
    PlayStation,
}

/// Glifo del atlas del juego y nombre (si falta el glifo) de un botón.
pub fn glyph(button: u16, kind: Kind) -> (&'static str, &'static str) {
    let ps = kind == Kind::PlayStation;
    let pick = |xbox: (&'static str, &'static str), play: (&'static str, &'static str)| {
        if ps {
            play
        } else {
            xbox
        }
    };
    match button {
        A => pick(("KG_R_D", "A"), ("KG_PS5_R_D", "Cross")),
        B => pick(("KG_R_R", "B"), ("KG_PS5_R_R", "Circle")),
        X => pick(("KG_R_L", "X"), ("KG_PS5_R_L", "Square")),
        Y => pick(("KG_R_U", "Y"), ("KG_PS5_R_U", "Triangle")),
        LEFT_SHOULDER => pick(("KG_L1", "LB"), ("KG_PS5_L1", "L1")),
        RIGHT_SHOULDER => pick(("KG_R1", "RB"), ("KG_PS5_R1", "R1")),
        LEFT_TRIGGER => pick(("KG_L2", "LT"), ("KG_PS5_L2", "L2")),
        RIGHT_TRIGGER => pick(("KG_R2", "RT"), ("KG_PS5_R2", "R2")),
        LEFT_THUMB => pick(("KG_L3", "LS"), ("KG_PS5_L3", "L3")),
        RIGHT_THUMB => pick(("KG_R3", "RS"), ("KG_PS5_R3", "R3")),
        START => pick(("KG_Start", "Menu"), ("KG_PS5_Start", "Options")),
        BACK => pick(("KG_Back", "View"), ("KG_PS5_TP", "Touchpad")),
        _ => ("", "?"),
    }
}

/// Sticks y gatillos de XInput por encima de esto cuentan como apretados.
const STICK_THRESHOLD: i16 = 16_000;
const TRIGGER_THRESHOLD: u8 = 60;

// ---------------------------------------------------------------------------------------------
// XInput

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
/// Mandos de XInput (bit por índice 0-3) con algo apretado, según la última lectura del juego.
static XINPUT_HELD: AtomicU8 = AtomicU8::new(0);
/// Lecturas de XInput del juego con un mando conectado (para el log).
static XINPUT_READS: AtomicU32 = AtomicU32::new(0);

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
    let connected = result == 0 && !state.is_null();
    if user < 4 {
        let bit = 1u8 << user;
        if connected && pressed_anything(&*state) {
            XINPUT_HELD.fetch_or(bit, Ordering::Relaxed);
        } else {
            XINPUT_HELD.fetch_and(!bit, Ordering::Relaxed);
        }
    }
    if connected {
        XINPUT_READS.fetch_add(1, Ordering::Relaxed);
        if crate::input_block::blocked() {
            let packet = (*state).packet;
            *state = XInputState {
                packet,
                ..Default::default()
            };
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

// ---------------------------------------------------------------------------------------------
// DirectInput (joystick)

/// Primeros campos de `DIJOYSTATE` / `DIJOYSTATE2`: ejes, sliders, POV y botones.
const JOY_STATE: usize = 80;
const JOY_POV: usize = 32;
const JOY_BUTTONS: usize = 48;
/// POV centrado.
const POV_CENTER: u32 = 0xFFFF_FFFF;

struct Joystick {
    /// Última lectura real (sin bloquear) y cuándo.
    last: [u8; JOY_STATE],
    at: Option<Instant>,
    /// Última lectura sin nada apretado (lo que el juego recibe mientras se bloquea).
    rest: Option<Vec<u8>>,
    /// Rango de los ejes (`DIPROP_RANGE` del eje X), si se pudo leer.
    range: Option<(i32, i32)>,
    logged: bool,
}

static JOYSTICK: Mutex<Joystick> = Mutex::new(Joystick {
    last: [0; JOY_STATE],
    at: None,
    rest: None,
    range: None,
    logged: false,
});
/// El juego tiene algo apretado en el joystick.
static JOY_HELD: AtomicBool = AtomicBool::new(false);
/// Lecturas del joystick del juego (para el log).
static JOY_READS: AtomicU32 = AtomicU32::new(0);

fn i32_at(s: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([s[off], s[off + 1], s[off + 2], s[off + 3]])
}

fn joy_pressed(s: &[u8]) -> bool {
    // Algunos drivers marcan el POV centrado solo en la palabra baja.
    i32_at(s, JOY_POV) as u32 & 0xFFFF != 0xFFFF
        || s[JOY_BUTTONS..JOY_BUTTONS + 32].iter().any(|b| b & 0x80 != 0)
}

#[repr(C)]
struct DiPropRange {
    size: u32,
    header_size: u32,
    obj: u32,
    how: u32,
    min: i32,
    max: i32,
}

type GetPropertyFn = unsafe extern "system" fn(*mut c_void, usize, *mut DiPropRange) -> i32;

/// Rango del eje X del joystick (`IDirectInputDevice8::GetProperty(DIPROP_RANGE)`).
unsafe fn axis_range(device: *mut c_void) -> Option<(i32, i32)> {
    let vtable = *(device as *const *const usize);
    let get_property: GetPropertyFn = std::mem::transmute(*vtable.add(5));
    let mut range = DiPropRange {
        size: std::mem::size_of::<DiPropRange>() as u32,
        header_size: 16,
        obj: 0,  // DIJOFS_X
        how: 1,  // DIPH_BYOFFSET
        min: 0,
        max: 0,
    };
    // DIPROP_RANGE = MAKEDIPROP(4).
    (get_property(device, 4, &mut range) >= 0 && range.max > range.min)
        .then_some((range.min, range.max))
}

/// Una lectura de joystick del juego (`GetDeviceState` con `cb` = 80 o 272), ya hecha por el
/// original. Guarda una copia y, si `blocked`, deja el mando en reposo.
///
/// # Safety
/// `data` apunta a `cb` bytes que el original acaba de llenar y `device` es el dispositivo.
pub(crate) unsafe fn joystick_read(device: *mut c_void, cb: u32, data: *mut u8, blocked: bool) {
    let state = std::slice::from_raw_parts_mut(data, cb as usize);
    JOY_READS.fetch_add(1, Ordering::Relaxed);
    let pressed = joy_pressed(state);
    JOY_HELD.store(pressed, Ordering::Relaxed);
    let mut j = JOYSTICK.lock().unwrap_or_else(|e| e.into_inner());
    if !j.logged {
        j.logged = true;
        j.range = axis_range(device);
        crate::log_event(&format!(
            "Mando por DirectInput: el juego lee un joystick ({cb} bytes, rango de ejes {:?})",
            j.range
        ));
    }
    j.last.copy_from_slice(&state[..JOY_STATE]);
    j.at = Some(Instant::now());
    if !blocked {
        if !pressed {
            j.rest = Some(state.to_vec());
        }
        return;
    }
    // En reposo: la última lectura sin nada apretado (ejes y gatillos donde estaban); si no hay,
    // la actual. Sin botones y con el POV centrado.
    if let Some(rest) = j.rest.as_ref().filter(|r| r.len() == state.len()) {
        state.copy_from_slice(rest);
    }
    for pov in 0..4 {
        state[JOY_POV + pov * 4..JOY_POV + pov * 4 + 4].copy_from_slice(&POV_CENTER.to_le_bytes());
    }
    state[JOY_BUTTONS..JOY_BUTTONS + 32].fill(0);
}

/// Botones (bits de XInput) de un mando de PlayStation por DirectInput. Orden de los botones
/// de DualShock 4 y DualSense: cuadrado, cruz, círculo, triángulo, L1, R1, L2, R2, Share/Create,
/// Options, L3, R3, PS, touchpad.
fn joystick_buttons(s: &[u8], range: Option<(i32, i32)>) -> u16 {
    const MAP: [u16; 14] = [
        X,
        A,
        B,
        Y,
        LEFT_SHOULDER,
        RIGHT_SHOULDER,
        LEFT_TRIGGER,
        RIGHT_TRIGGER,
        0,
        START,
        LEFT_THUMB,
        RIGHT_THUMB,
        0,
        BACK,
    ];
    let mut buttons = 0;
    for (i, &bit) in MAP.iter().enumerate() {
        if s[JOY_BUTTONS + i] & 0x80 != 0 {
            buttons |= bit;
        }
    }
    let pov = i32_at(s, JOY_POV) as u32;
    if pov & 0xFFFF != 0xFFFF {
        // Centésimas de grado, 0 = arriba, en el sentido de las agujas del reloj.
        let deg = pov / 100 % 360;
        if !(68..=292).contains(&deg) {
            buttons |= DPAD_UP;
        }
        if (23..=157).contains(&deg) {
            buttons |= DPAD_RIGHT;
        }
        if (113..=247).contains(&deg) {
            buttons |= DPAD_DOWN;
        }
        if (203..=337).contains(&deg) {
            buttons |= DPAD_LEFT;
        }
    }
    // Stick izquierdo (X e Y crecen hacia la derecha y hacia abajo).
    if let Some((min, max)) = range {
        let half = (max - min) as f32 / 2.0;
        let norm = |v: i32| (v - min) as f32 / half - 1.0;
        let (x, y) = (norm(i32_at(s, 0)), norm(i32_at(s, 4)));
        if x < -0.5 {
            buttons |= DPAD_LEFT;
        }
        if x > 0.5 {
            buttons |= DPAD_RIGHT;
        }
        if y < -0.5 {
            buttons |= DPAD_UP;
        }
        if y > 0.5 {
            buttons |= DPAD_DOWN;
        }
    }
    buttons
}

// ---------------------------------------------------------------------------------------------

/// Con la forja recién cerrada se sigue bloqueando hasta que el juego vea el mando suelto.
pub fn released() -> bool {
    XINPUT_HELD.load(Ordering::Relaxed) == 0
        && !JOY_HELD.load(Ordering::Relaxed)
        && crate::hid::released()
        && crate::steam_input::released()
}

/// Cuántas lecturas de mando del juego hubo desde la última vez (XInput, DirectInput).
pub fn take_stats() -> (u32, u32) {
    (
        XINPUT_READS.swap(0, Ordering::Relaxed),
        JOY_READS.swap(0, Ordering::Relaxed),
    )
}

/// Lo que se lee del mando en un frame, con el stick izquierdo sumado al D-pad.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Pad {
    /// Botones con los bits de XInput.
    pub buttons: u16,
    /// Se tocó algo desde la lectura anterior.
    pub touched: bool,
    pub kind: Kind,
}

struct Reader {
    user: Option<u32>,
    next_scan: Option<Instant>,
    last: XInputState,
    last_joy: u16,
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
    last_joy: 0,
});

/// Lee el mando: el primero de XInput o, si no hay, el de PlayStation que lee el juego (por HID
/// o como joystick de DirectInput).
pub fn read() -> Option<Pad> {
    let mut r = READER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(pad) = read_xinput(&mut r) {
        return Some(pad);
    }
    if let Some((buttons, kind)) = crate::steam_input::state() {
        let touched = buttons != r.last_joy;
        r.last_joy = buttons;
        return Some(Pad {
            buttons,
            touched,
            kind,
        });
    }
    if let Some(buttons) = crate::hid::state() {
        let touched = buttons != r.last_joy;
        r.last_joy = buttons;
        return Some(Pad {
            buttons,
            touched,
            kind: Kind::PlayStation,
        });
    }
    let j = JOYSTICK.lock().unwrap_or_else(|e| e.into_inner());
    if !j.at.is_some_and(|t| t.elapsed() < Duration::from_millis(500)) {
        return None;
    }
    let buttons = joystick_buttons(&j.last, j.range);
    let touched = buttons != r.last_joy;
    r.last_joy = buttons;
    Some(Pad {
        buttons,
        touched,
        kind: Kind::PlayStation,
    })
}

fn read_xinput(r: &mut Reader) -> Option<Pad> {
    let get = real_get_state()?;
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
    let mut buttons = state.buttons & !(LEFT_TRIGGER | RIGHT_TRIGGER);
    if state.left_trigger > TRIGGER_THRESHOLD {
        buttons |= LEFT_TRIGGER;
    }
    if state.right_trigger > TRIGGER_THRESHOLD {
        buttons |= RIGHT_TRIGGER;
    }
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
    Some(Pad {
        buttons,
        touched,
        kind: Kind::Xbox,
    })
}
