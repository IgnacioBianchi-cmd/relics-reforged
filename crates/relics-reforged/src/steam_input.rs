//! Mando por Steam Input: el camino que usa el juego cuando Steam maneja el mando (probado el
//! 2026-11-06 con un mando de PlayStation configurado en Steam).
//!
//! El juego no ve el mando por XInput: cada frame le pide a `ISteamInput` (`SteamInput006`) el
//! estado de 14 acciones digitales y 4 analógicas, que son los botones y los sticks/gatillos de un
//! mando (contado con [`input_diag`](crate::input_diag)). Métodos de la vtable usados (orden de
//! `isteaminput.h`, confirmado por la cantidad de llamadas):
//!
//! | # | Método |
//! |---|---|
//! | 11 | `GetCurrentActionSet(mando)` |
//! | 17 | `GetDigitalActionData(mando, acción)` → `{bool bState, bool bActive}` |
//! | 18 | `GetDigitalActionOrigins(mando, set, acción, origins[8])` |
//! | 21 | `GetAnalogActionData(mando, acción)` → `{modo, x, y, bActive}` |
//! | 22 | `GetAnalogActionOrigins(mando, set, acción, origins[8])` |
//! | 42 | `GetActionOriginFromXboxOrigin(mando, EXboxOrigin)` |
//!
//! Se reemplazan el 17 y el 21. Cada resultado se guarda para la forja y, si
//! [`input_block`](crate::input_block) bloquea, el juego lo recibe suelto (`bState` falso, x e y
//! en cero). Para saber qué botón es cada acción, se comparan sus orígenes (el botón físico al que
//! está asignada) con el origen de cada botón de Xbox en ese mando (`EXboxOrigin`, un enum
//! chico y estable): así no hace falta conocer los nombres de las acciones del juego.
//!
//! MSVC devuelve las estructuras de los métodos virtuales por un puntero oculto (después de
//! `this`): por eso los ganchos reciben 4 enteros y, para tocar el resultado, verifican que lo
//! devuelto sea ese puntero.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{pad, win};

const GET_CURRENT_ACTION_SET: usize = 11;
const GET_DIGITAL_ACTION_DATA: usize = 17;
const GET_DIGITAL_ACTION_ORIGINS: usize = 18;
const GET_ANALOG_ACTION_DATA: usize = 21;
const GET_ANALOG_ACTION_ORIGINS: usize = 22;
const GET_INPUT_TYPE: usize = 37;
const GET_ORIGIN_FROM_XBOX: usize = 42;
const MAX_ORIGINS: usize = 8;

/// `EXboxOrigin` → bits de [`pad`] (botones; los ejes van aparte).
const XBOX_BUTTONS: [(i32, u16); 20] = [
    (0, pad::A),
    (1, pad::B),
    (2, pad::X),
    (3, pad::Y),
    (4, pad::LEFT_SHOULDER),
    (5, pad::RIGHT_SHOULDER),
    (6, pad::START),
    (7, pad::BACK),
    (9, pad::LEFT_TRIGGER),   // LeftTrigger_Click
    (11, pad::RIGHT_TRIGGER), // RightTrigger_Click
    (13, pad::LEFT_THUMB),
    (14, pad::DPAD_UP), // LeftStick_DPadNorth
    (15, pad::DPAD_DOWN),
    (16, pad::DPAD_LEFT),
    (17, pad::DPAD_RIGHT),
    (19, pad::RIGHT_THUMB),
    (24, pad::DPAD_UP), // DPad_North
    (25, pad::DPAD_DOWN),
    (26, pad::DPAD_LEFT),
    (27, pad::DPAD_RIGHT),
];

/// Qué hace una acción analógica para la forja.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Analog {
    LeftStick,
    LeftTrigger,
    RightTrigger,
    Other,
}

/// `EXboxOrigin` de los ejes: gatillos (Pull) y stick izquierdo (Move).
const XBOX_AXES: [(i32, Analog); 3] = [
    (8, Analog::LeftTrigger),
    (10, Analog::RightTrigger),
    (12, Analog::LeftStick),
];

static INPUT: AtomicUsize = AtomicUsize::new(0);
static ORIG_DIGITAL: AtomicUsize = AtomicUsize::new(0);
static ORIG_ANALOG: AtomicUsize = AtomicUsize::new(0);
/// Métodos que se llaman directamente (tomados de la vtable al instalar).
static FN_ACTION_SET: AtomicUsize = AtomicUsize::new(0);
static FN_DIGITAL_ORIGINS: AtomicUsize = AtomicUsize::new(0);
static FN_ANALOG_ORIGINS: AtomicUsize = AtomicUsize::new(0);
static FN_FROM_XBOX: AtomicUsize = AtomicUsize::new(0);
static FN_INPUT_TYPE: AtomicUsize = AtomicUsize::new(0);
static HELD: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct State {
    /// Por mando: origen de cada botón de Xbox en ese mando.
    xbox: HashMap<u64, Vec<(i32, u16)>>,
    xbox_axes: HashMap<u64, Vec<(i32, Analog)>>,
    /// (set, acción) → botones / eje.
    digital_map: HashMap<(u64, u64), u16>,
    analog_map: HashMap<(u64, u64), Analog>,
    /// Último set de acciones de cada mando (se consulta como mucho cada 250 ms).
    action_set: HashMap<u64, (u64, Instant)>,
    /// Estado real (sin bloquear) de cada acción: botones apretados por acción digital y valor
    /// por acción analógica.
    digital: HashMap<u64, u16>,
    analog: HashMap<u64, (Analog, f32, f32)>,
    at: Option<Instant>,
    logged: bool,
    /// Tipo de cada mando (`GetInputTypeForHandle`) y el del último que se leyó.
    kinds: HashMap<u64, pad::Kind>,
    kind: pad::Kind,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

type DigitalFn = unsafe extern "system" fn(usize, usize, u64, u64) -> usize;
type AnalogFn = unsafe extern "system" fn(usize, usize, u64, u64) -> usize;
type ActionSetFn = unsafe extern "system" fn(usize, u64) -> u64;
type OriginsFn = unsafe extern "system" fn(usize, u64, u64, u64, *mut i32) -> i32;
type FromXboxFn = unsafe extern "system" fn(usize, u64, i32) -> i32;
type InputTypeFn = unsafe extern "system" fn(usize, u64) -> i32;

fn load(f: &AtomicUsize) -> usize {
    f.load(Ordering::Relaxed)
}

/// Set de acciones actual del mando (cacheado 250 ms: el juego llama 18 veces por frame).
unsafe fn action_set(s: &mut State, this: usize, controller: u64) -> u64 {
    if let Some(&(set, at)) = s.action_set.get(&controller) {
        if at.elapsed() < Duration::from_millis(250) {
            return set;
        }
    }
    let f: ActionSetFn = std::mem::transmute(load(&FN_ACTION_SET));
    let set = f(this, controller);
    s.action_set.insert(controller, (set, Instant::now()));
    set
}

unsafe fn origins(f: &AtomicUsize, this: usize, controller: u64, set: u64, action: u64) -> Vec<i32> {
    let f: OriginsFn = std::mem::transmute(load(f));
    let mut out = [0i32; MAX_ORIGINS];
    let n = f(this, controller, set, action, out.as_mut_ptr()).clamp(0, MAX_ORIGINS as i32);
    out[..n as usize].to_vec()
}

unsafe fn xbox_tables(s: &mut State, this: usize, controller: u64) {
    if s.xbox.contains_key(&controller) {
        return;
    }
    let f: FromXboxFn = std::mem::transmute(load(&FN_FROM_XBOX));
    let buttons = XBOX_BUTTONS
        .iter()
        .map(|&(x, bit)| (f(this, controller, x), bit))
        .filter(|&(o, _)| o != 0)
        .collect();
    let axes = XBOX_AXES
        .iter()
        .map(|&(x, a)| (f(this, controller, x), a))
        .filter(|&(o, _)| o != 0)
        .collect();
    s.xbox.insert(controller, buttons);
    s.xbox_axes.insert(controller, axes);
}

/// Botones (bits de [`pad`]) que corresponden a una acción digital.
unsafe fn digital_bits(s: &mut State, this: usize, controller: u64, action: u64) -> u16 {
    let set = action_set(s, this, controller);
    if let Some(&bits) = s.digital_map.get(&(set, action)) {
        return bits;
    }
    xbox_tables(s, this, controller);
    let mine = origins(&FN_DIGITAL_ORIGINS, this, controller, set, action);
    let bits = s.xbox[&controller]
        .iter()
        .filter(|(o, _)| mine.contains(o))
        .fold(0, |acc, &(_, bit)| acc | bit);
    s.digital_map.insert((set, action), bits);
    if s.digital_map.len() + s.analog_map.len() <= 64 {
        crate::log_event(&format!(
            "Steam Input: acción digital {action:#X} (set {set:#X}, orígenes {mine:?}) = \
             botones {bits:#06X}"
        ));
    }
    bits
}

unsafe fn analog_kind(s: &mut State, this: usize, controller: u64, action: u64) -> Analog {
    let set = action_set(s, this, controller);
    if let Some(&a) = s.analog_map.get(&(set, action)) {
        return a;
    }
    xbox_tables(s, this, controller);
    let mine = origins(&FN_ANALOG_ORIGINS, this, controller, set, action);
    let kind = s.xbox_axes[&controller]
        .iter()
        .find(|(o, _)| mine.contains(o))
        .map_or(Analog::Other, |&(_, a)| a);
    s.analog_map.insert((set, action), kind);
    if s.digital_map.len() + s.analog_map.len() <= 64 {
        crate::log_event(&format!(
            "Steam Input: acción analógica {action:#X} (set {set:#X}, orígenes {mine:?}) = {kind:?}"
        ));
    }
    kind
}

fn update_held(s: &State) {
    let digital = s.digital.values().any(|&b| b != 0);
    let analog = s
        .analog
        .values()
        .any(|&(_, x, y)| x.abs() > 0.5 || y.abs() > 0.5);
    HELD.store(digital || analog, Ordering::Relaxed);
}

unsafe extern "system" fn digital_hook(this: usize, ret: usize, controller: u64, action: u64) -> usize {
    let orig: DigitalFn = std::mem::transmute(load(&ORIG_DIGITAL));
    let result = orig(this, ret, controller, action);
    // Con el puntero oculto, lo devuelto es ese puntero; si no, el resultado vino en rax y los
    // argumentos están corridos uno (mando en rdx, acción en r8).
    let by_pointer = result == ret && ret != 0;
    let (pressed, controller, action) = if by_pointer {
        (*(ret as *const u8) != 0, controller, action)
    } else {
        (result & 0xFF != 0, ret as u64, controller)
    };
    {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let s = guard.get_or_insert_with(State::default);
        let bits = digital_bits(s, this, controller, action);
        s.kind = *s.kinds.entry(controller).or_insert_with(|| {
            let f: InputTypeFn = std::mem::transmute(load(&FN_INPUT_TYPE));
            match f(this, controller) {
                // ESteamInputType: PS4 5, PS3 12, PS5 13.
                5 | 12 | 13 => pad::Kind::PlayStation,
                _ => pad::Kind::Xbox,
            }
        });
        s.digital
            .insert(action, if pressed { bits } else { 0 });
        s.at = Some(Instant::now());
        if !s.logged {
            s.logged = true;
            crate::log_event(&format!(
                "Steam Input: el juego lee el mando (resultado {})",
                if by_pointer { "por puntero" } else { "en rax" }
            ));
        }
        update_held(s);
    }
    if crate::input_block::blocked() {
        if by_pointer {
            *(ret as *mut u8) = 0;
        } else {
            return result & !0xFF;
        }
    }
    result
}

unsafe extern "system" fn analog_hook(this: usize, ret: usize, controller: u64, action: u64) -> usize {
    let orig: AnalogFn = std::mem::transmute(load(&ORIG_ANALOG));
    let result = orig(this, ret, controller, action);
    // InputAnalogActionData_t: modo (i32), x (f32), y (f32), bActive (bool). Siempre por
    // puntero (no entra en un registro).
    if result != ret || ret == 0 {
        return result;
    }
    let x = *((ret + 4) as *const f32);
    let y = *((ret + 8) as *const f32);
    {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let s = guard.get_or_insert_with(State::default);
        let kind = analog_kind(s, this, controller, action);
        s.analog.insert(action, (kind, x, y));
        s.at = Some(Instant::now());
        update_held(s);
    }
    if crate::input_block::blocked() {
        *((ret + 4) as *mut f32) = 0.0;
        *((ret + 8) as *mut f32) = 0.0;
    }
    result
}

/// Botones del mando según lo último que el juego le pidió a Steam Input (si fue hace poco), y
/// qué mando es.
pub fn state() -> Option<(u16, pad::Kind)> {
    let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = guard.as_ref()?;
    if !s.at.is_some_and(|t| t.elapsed() < Duration::from_millis(500)) {
        return None;
    }
    let mut buttons = s.digital.values().fold(0, |acc, &b| acc | b);
    for &(kind, x, y) in s.analog.values() {
        match kind {
            Analog::LeftTrigger if x > 0.5 => buttons |= pad::LEFT_TRIGGER,
            Analog::RightTrigger if x > 0.5 => buttons |= pad::RIGHT_TRIGGER,
            Analog::LeftStick => {
                // Steam Input: y positivo es arriba.
                if x < -0.5 {
                    buttons |= pad::DPAD_LEFT;
                }
                if x > 0.5 {
                    buttons |= pad::DPAD_RIGHT;
                }
                if y > 0.5 {
                    buttons |= pad::DPAD_UP;
                }
                if y < -0.5 {
                    buttons |= pad::DPAD_DOWN;
                }
            }
            _ => {}
        }
    }
    Some((buttons, s.kind))
}

pub fn released() -> bool {
    !HELD.load(Ordering::Relaxed)
}

/// La interfaz `SteamInput006` del juego (la de steamclient, compartida).
unsafe fn interface() -> Result<usize, String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    type GetUser = unsafe extern "C" fn() -> i32;
    type FindOrCreate = unsafe extern "C" fn(i32, *const u8) -> *mut c_void;
    let name: Vec<u16> = "steam_api64.dll".encode_utf16().chain([0]).collect();
    let api = GetModuleHandleW(name.as_ptr());
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
    Ok(input as usize)
}

/// Engancha `GetDigitalActionData` y `GetAnalogActionData`. Llamar con la partida cargada
/// (Steam ya inicializado).
pub fn install() -> Result<(), String> {
    // SAFETY: interfaz de Steam con la vtable de `isteaminput.h`; se verifica que cada entrada
    // usada apunte a código antes de tocar nada.
    unsafe {
        let input = interface()?;
        INPUT.store(input, Ordering::SeqCst);
        let vtable = *(input as *const usize);
        let entry = |i: usize| -> Result<usize, String> {
            let f: usize = win::read(vtable + i * 8).ok_or("no puedo leer la vtable de ISteamInput")?;
            if f == 0 || !win::is_executable(f) {
                return Err(format!("la entrada {i} de ISteamInput no apunta a código"));
            }
            Ok(f)
        };
        for i in [GET_ORIGIN_FROM_XBOX + 5, GET_ORIGIN_FROM_XBOX] {
            // Que la vtable sea al menos tan larga como la de SteamInput006 (48 métodos).
            entry(i)?;
        }
        FN_ACTION_SET.store(entry(GET_CURRENT_ACTION_SET)?, Ordering::SeqCst);
        FN_DIGITAL_ORIGINS.store(entry(GET_DIGITAL_ACTION_ORIGINS)?, Ordering::SeqCst);
        FN_ANALOG_ORIGINS.store(entry(GET_ANALOG_ACTION_ORIGINS)?, Ordering::SeqCst);
        FN_FROM_XBOX.store(entry(GET_ORIGIN_FROM_XBOX)?, Ordering::SeqCst);
        FN_INPUT_TYPE.store(entry(GET_INPUT_TYPE)?, Ordering::SeqCst);
        entry(GET_DIGITAL_ACTION_DATA)?;
        entry(GET_ANALOG_ACTION_DATA)?;
        crate::input_block::patch(
            vtable,
            GET_DIGITAL_ACTION_DATA,
            digital_hook as DigitalFn as usize,
            &ORIG_DIGITAL,
        )?;
        crate::input_block::patch(
            vtable,
            GET_ANALOG_ACTION_DATA,
            analog_hook as AnalogFn as usize,
            &ORIG_ANALOG,
        )?;
        Ok(())
    }
}
