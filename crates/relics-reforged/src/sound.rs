//! Sonidos de menú del propio juego.
//!
//! Los menús piden sus sonidos con `nr+870290`, que recibe una estructura de 24 bytes: ID del
//! sonido, prioridad, canal y un puntero a una etiqueta de depuración (texto UTF-16 en japonés).
//! Con el canal 1, el pedido queda en una cola de un sonido por frame (`nr+39AA560`; gana el de
//! mayor prioridad) y el juego lo reproduce en su próximo frame (`nr+8701D0`). Como solo escribe
//! esa cola, se puede llamar desde el hilo de render.
//!
//! IDs vistos el 2026-11-05 vigilando `nr+870290` en Relic Rites (etiqueta y prioridad con la
//! que los pide el juego): 0 mover el cursor (カーソル移動, 0), 100 confirmar (決定, 2),
//! 200 cancelar (キャンセル, 2), 500 cambiar de pestaña (タブ移動, 1), 600 error (エラー, 2),
//! 1000 abrir un menú (メニューオープン, 10), 1020 cerrar un menú (メニュークローズ, 0).

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::win;

const PLAY_RVA: usize = 0x87_0290;
/// Primeros bytes de `nr+870290` en 1.3.3.0 (control de versión antes de llamarla).
const PLAY_PROLOGUE: [u8; 8] = [0x40, 0x53, 0x48, 0x83, 0xEC, 0x20, 0x8B, 0x11];
/// Texto "null" (UTF-16) que el juego usa como etiqueta por defecto.
const NULL_LABEL_RVA: usize = 0x32E_CEA8;
/// Canal: a la cola de un sonido por frame.
const QUEUED: i32 = 1;

/// Mover el cursor.
pub fn cursor() {
    play(0, 0);
}
/// Confirmar.
pub fn decide() {
    play(100, 2);
}
/// Cancelar o volver.
pub fn cancel() {
    play(200, 2);
}
/// Cambiar de pestaña.
pub fn tab() {
    play(500, 1);
}
/// Error.
pub fn error() {
    play(600, 2);
}
/// Abrir un menú.
pub fn open() {
    play(1000, 10);
}
/// Cerrar un menú.
pub fn close() {
    play(1020, 0);
}

#[repr(C)]
struct Request {
    id: i32,
    priority: i32,
    channel: i32,
    _pad: i32,
    label: usize,
}

static BASE: AtomicUsize = AtomicUsize::new(0);

/// Verifica la función y la deja lista. `Err` si el ejecutable no es el esperado.
pub fn install(base: usize) -> Result<(), String> {
    let found: [u8; 8] = win::read(base + PLAY_RVA).ok_or("no puedo leer nr+870290")?;
    if found != PLAY_PROLOGUE {
        return Err(format!("nr+870290 no es la esperada: {found:02X?}"));
    }
    BASE.store(base, Ordering::SeqCst);
    Ok(())
}

/// Pide un sonido de menú del juego. No hace nada si no se instaló.
pub fn play(id: i32, priority: i32) {
    let base = BASE.load(Ordering::Relaxed);
    if base == 0 {
        return;
    }
    let req = Request {
        id,
        priority,
        channel: QUEUED,
        _pad: 0,
        label: base + NULL_LABEL_RVA,
    };
    // SAFETY: función del juego verificada en `install`; recibe un puntero a la estructura, que
    // copia antes de volver.
    unsafe {
        let f: unsafe extern "system" fn(*const Request) = std::mem::transmute(base + PLAY_RVA);
        f(&req);
    }
}
