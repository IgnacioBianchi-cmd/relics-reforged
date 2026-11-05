//! ¿Está abierto el menú de Relic Rites?
//!
//! La ventana de Relic Rites es la clase `CS::AntiqueEquipDialog` ("Antique" = reliquia en el
//! código del juego), con su vtable en `nr+2C4B078` (50 métodos). Se reemplaza cada puntero de la
//! vtable por un código propio que suma 1 a un contador y salta al método original. Mientras la
//! ventana existe, el juego llama a sus métodos en cada frame: si el contador cambió hace menos de
//! 300 ms, Relic Rites está abierto.
//!
//! Encontrado el 2026-11-05 vigilando las lecturas de una reliquia con el menú abierto
//! (`watchr`): las lecturas nuevas venían de `nr+920610`, el método 42 de esta vtable.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::win;

const VTABLE_RVA: usize = 0x2C4_B078;
const VTABLE_LEN: usize = 50;
/// El CompleteObjectLocator de RTTI que va justo antes de la vtable (control de versión).
const COL_RVA: usize = 0x335_4BB8;
/// Método 42: el que lee las reliquias para mostrarlas.
const KNOWN_METHOD: (usize, usize) = (42, 0x92_0610);
const ACTIVE_WINDOW: Duration = Duration::from_millis(300);

static INSTALLED: AtomicBool = AtomicBool::new(false);
/// Cantidad de llamadas a métodos de la ventana (la incrementa el código del gancho).
static CALLS: AtomicU64 = AtomicU64::new(0);
/// (último valor visto del contador, cuándo cambió).
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
    let counter = &CALLS as *const AtomicU64 as u64;
    for (i, &original) in originals.iter().enumerate() {
        let mut t = vec![0x48, 0xB8]; // mov rax, &CALLS
        t.extend_from_slice(&counter.to_le_bytes());
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

/// ¿Se llamó a algún método de la ventana de Relic Rites en los últimos 300 ms?
pub fn is_open() -> bool {
    if !INSTALLED.load(Ordering::Relaxed) {
        return false;
    }
    let now_calls = CALLS.load(Ordering::Relaxed);
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
