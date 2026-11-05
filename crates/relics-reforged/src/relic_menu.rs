//! ¿Está abierto el menú de Relic Rites, y dónde está el cursor?
//!
//! La ventana de Relic Rites es la clase `CS::AntiqueEquipDialog` ("Antique" = reliquia en el
//! código del juego), con su vtable en `nr+2C4B078` (50 métodos). Se reemplaza cada puntero de la
//! vtable por un código propio que suma 1 al contador de ese método y salta al original. Mientras
//! la ventana existe, el juego llama a sus métodos en cada frame: si algún contador cambió hace
//! menos de 300 ms, Relic Rites está abierto.
//!
//! Encontrado el 2026-11-05 vigilando las lecturas de una reliquia con el menú abierto
//! (`watchr`): las lecturas nuevas venían de `nr+920610`, el método 42 de esta vtable.
//!
//! Con los contadores por método también se sabe dónde está el cursor (ver [`panel`]).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::win;

const VTABLE_RVA: usize = 0x2C4_B078;
const VTABLE_LEN: usize = 50;
/// El CompleteObjectLocator de RTTI que va justo antes de la vtable (control de versión).
const COL_RVA: usize = 0x335_4BB8;
/// Método 42: el que lee las reliquias para mostrarlas. Se llama una vez por frame.
const KNOWN_METHOD: (usize, usize) = (42, 0x92_0610);
/// Métodos 38 a 41: una vez por frame con el cursor en los slots y dos con el cursor a la derecha.
const RIGHT_METHODS: std::ops::Range<usize> = 38..42;
const ACTIVE_WINDOW: Duration = Duration::from_millis(300);
const PANEL_WINDOW: Duration = Duration::from_millis(200);

static INSTALLED: AtomicBool = AtomicBool::new(false);
/// Llamadas a cada método de la ventana (las incrementa el código del gancho).
static CALLS: [AtomicU64; VTABLE_LEN] = [const { AtomicU64::new(0) }; VTABLE_LEN];
/// (último valor visto del total de llamadas, cuándo cambió).
static LAST: Mutex<Option<(u64, Instant)>> = Mutex::new(None);

pub fn install(base: usize) -> Result<(), String> {
    if INSTALLED.load(Ordering::SeqCst) {
        return Ok(());
    }
    let vtable = base + VTABLE_RVA;
    let col: usize = win::read(vtable - 8).ok_or("no puedo leer la vtable")?;
    let known: usize = win::read(vtable + KNOWN_METHOD.0 * 8).ok_or("no puedo leer la vtable")?;
    if col != base + COL_RVA || known != base + KNOWN_METHOD.1 {
        return Err(format!(
            "la vtable de AntiqueEquipDialog no es la esperada (COL {col:#X}, método 42 {known:#X})"
        ));
    }
    let originals: [usize; VTABLE_LEN] = win::read(vtable).ok_or("no puedo leer la vtable")?;
    const THUNK: usize = 0x20;
    let code =
        win::alloc_executable_near(base, VTABLE_LEN * THUNK).ok_or("no pude reservar memoria")?;
    for (i, &original) in originals.iter().enumerate() {
        let mut t = vec![0x48, 0xB8]; // mov rax, &CALLS[i]
        t.extend_from_slice(&(&CALLS[i] as *const AtomicU64 as u64).to_le_bytes());
        t.extend_from_slice(&[0xF0, 0x48, 0xFF, 0x00]); // lock inc qword ptr [rax]
        t.extend_from_slice(&[0x48, 0xB8]); // mov rax, método original
        t.extend_from_slice(&(original as u64).to_le_bytes());
        t.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
        // SAFETY: memoria nuestra, recién reservada, con lugar para todos los ganchos.
        unsafe {
            std::ptr::copy_nonoverlapping(t.as_ptr(), (code + i * THUNK) as *mut u8, t.len())
        };
    }
    // SAFETY: la vtable está en .rdata; se habilita escritura solo para cambiar sus punteros.
    unsafe {
        let mut old = 0;
        if win::VirtualProtect(vtable as *mut _, VTABLE_LEN * 8, 0x04, &mut old) == 0 {
            return Err("VirtualProtect falló".into());
        }
        for i in 0..VTABLE_LEN {
            std::ptr::write_volatile((vtable + i * 8) as *mut usize, code + i * THUNK);
        }
        win::VirtualProtect(vtable as *mut _, VTABLE_LEN * 8, old, &mut old);
    }
    INSTALLED.store(true, Ordering::SeqCst);
    Ok(())
}

fn calls(i: usize) -> u64 {
    CALLS[i].load(Ordering::Relaxed)
}

/// ¿Se llamó a algún método de la ventana de Relic Rites en los últimos 300 ms?
pub fn is_open() -> bool {
    if !INSTALLED.load(Ordering::Relaxed) {
        return false;
    }
    let now_calls: u64 = (0..VTABLE_LEN).map(calls).sum();
    let now = Instant::now();
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    match *last {
        Some((calls, at)) if calls == now_calls => now.duration_since(at) < ACTIVE_WINDOW,
        _ => {
            let first = last.is_none();
            *last = Some((now_calls, now));
            // En la primera consulta no sabemos si cambió: se toma como cerrado.
            !first
        }
    }
}

/// Dónde está el cursor de Relic Rites. Cambia la barra de controles de abajo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// En los slots de arriba (reliquias equipadas): "F:OK Q:Close ... M:All Presets".
    Slots,
    /// En la grilla de la derecha: recorriéndola ("1:Swap screen 3:Save as Preset M:All
    /// Presets") o eligiendo la reliquia para un slot (solo "1:Swap screen").
    Right,
}

struct PanelSample {
    at: Instant,
    right: u64,
    known: u64,
    panel: Option<Panel>,
}

static PANEL: Mutex<Option<PanelSample>> = Mutex::new(None);

/// Panel con el cursor, según cuántas veces por frame se llama a los métodos 38 a 41 (comparado
/// con el 42, que va una vez por frame): el doble con el cursor a la derecha. Medido el
/// 2026-11-05 con 9 mediciones de 1 s alternando slots, grilla y elección (59 contra 119 por
/// segundo); en la memoria de la ventana no apareció ningún valor estable para esto. Se
/// recalcula cada 200 ms; `None` hasta tener la primera medición.
pub fn panel() -> Option<Panel> {
    let right: u64 = RIGHT_METHODS.map(calls).sum();
    let known = calls(KNOWN_METHOD.0);
    let now = Instant::now();
    let mut sample = PANEL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(s) = sample.as_mut() else {
        *sample = Some(PanelSample {
            at: now,
            right,
            known,
            panel: None,
        });
        return None;
    };
    if now.duration_since(s.at) < PANEL_WINDOW {
        return s.panel;
    }
    let (d_right, d_known) = (right - s.right, known - s.known);
    // Con la ventana cerrada no hay llamadas: se conserva la última medición.
    if d_known > 0 {
        let per_frame = d_right as f64 / (RIGHT_METHODS.len() as u64 * d_known) as f64;
        s.panel = Some(if per_frame > 1.5 {
            Panel::Right
        } else {
            Panel::Slots
        });
    }
    s.at = now;
    s.right = right;
    s.known = known;
    s.panel
}
