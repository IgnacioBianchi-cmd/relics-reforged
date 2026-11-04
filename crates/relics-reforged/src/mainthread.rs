//! Ejecutar trabajos en el hilo principal del juego, entre un frame y el siguiente.
//!
//! El ciclo principal (`nr+DCB340`) llama a `nr+F2F640` (un frame entero) desde `nr+DCB37F`.
//! Cambiamos el destino de esa llamada por un pequeño código propio, reservado cerca del
//! ejecutable, que guarda los registros, corre `on_frame` y salta a `nr+F2F640`. Solo se escriben
//! los 4 bytes alineados del desplazamiento de la llamada, de una vez.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use crate::win;

const CALL_SITE_RVA: usize = 0xDC_B37F;
const FRAME_FN_RVA: usize = 0xF2_F640;
/// `call nr+F2F640` tal como está en el ejecutable.
const CALL_SITE_BYTES: [u8; 5] = [0xE8, 0xBC, 0x42, 0x16, 0x00];

type Job = Box<dyn FnOnce() + Send>;

static INSTALLED: AtomicBool = AtomicBool::new(false);
static FRAMES: AtomicU64 = AtomicU64::new(0);
static QUEUE: Mutex<Vec<Job>> = Mutex::new(Vec::new());

pub fn is_installed() -> bool {
    INSTALLED.load(Ordering::SeqCst)
}

/// Frames que pasaron por el gancho desde que se instaló.
pub fn frames() -> u64 {
    FRAMES.load(Ordering::Relaxed)
}

/// Lo llama el código del gancho una vez por frame, en el hilo principal.
extern "win64" fn on_frame() {
    FRAMES.fetch_add(1, Ordering::Relaxed);
    let jobs = match QUEUE.try_lock() {
        Ok(mut q) if !q.is_empty() => std::mem::take(&mut *q),
        _ => return,
    };
    for job in jobs {
        // Un panic no puede cruzar al código del juego.
        let _ = catch_unwind(AssertUnwindSafe(job));
    }
}

/// Instala el gancho. Verifica los bytes originales antes de tocar nada.
pub fn install(base: usize) -> Result<(), String> {
    if is_installed() {
        return Ok(());
    }
    let site = base + CALL_SITE_RVA;
    let original: [u8; 5] = win::read(site).ok_or("no puedo leer el punto del gancho")?;
    if original != CALL_SITE_BYTES {
        return Err(format!(
            "los bytes en nr+{CALL_SITE_RVA:X} no son los esperados: {original:02X?}"
        ));
    }
    let frame_fn = base + FRAME_FN_RVA;
    let thunk = win::alloc_executable_near(base, 0x100)
        .ok_or("no pude reservar memoria cerca del juego")?;
    let code = thunk_code(on_frame as extern "win64" fn() as usize, frame_fn);
    // SAFETY: la memoria es nuestra, recién reservada, de 0x100 bytes.
    unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), thunk as *mut u8, code.len()) };
    let rel = thunk as i64 - (site as i64 + 5);
    let rel: i32 = rel
        .try_into()
        .map_err(|_| "la memoria quedó a más de 2 GB")?;
    // SAFETY: cambiamos 4 bytes alineados del desplazamiento de un `call` cuyo contenido
    // verificamos; la escritura de 4 bytes alineados es atómica.
    unsafe {
        let disp = (site + 1) as *mut u32;
        debug_assert_eq!(disp as usize % 4, 0);
        let mut old = 0;
        if win::VirtualProtect(site as *mut _, 5, 0x40, &mut old) == 0 {
            return Err("VirtualProtect falló".into());
        }
        std::ptr::write_volatile(disp, rel as u32);
        win::VirtualProtect(site as *mut _, 5, old, &mut old);
        win::flush_instruction_cache(site, 5);
    }
    INSTALLED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Código del gancho: guarda rcx, rdx, r8, r9 y xmm0-3, llama a `hook`, los restaura y salta a
/// `target` (así `target` devuelve directo al juego, con su propio valor de retorno).
fn thunk_code(hook: usize, target: usize) -> Vec<u8> {
    let mut c = vec![
        0x51, // push rcx
        0x52, // push rdx
        0x41, 0x50, // push r8
        0x41, 0x51, // push r9
        0x48, 0x83, 0xEC, 0x68, // sub rsp, 0x68 (queda alineada a 16)
        0xF3, 0x0F, 0x7F, 0x44, 0x24, 0x20, // movdqu [rsp+20h], xmm0
        0xF3, 0x0F, 0x7F, 0x4C, 0x24, 0x30, // movdqu [rsp+30h], xmm1
        0xF3, 0x0F, 0x7F, 0x54, 0x24, 0x40, // movdqu [rsp+40h], xmm2
        0xF3, 0x0F, 0x7F, 0x5C, 0x24, 0x50, // movdqu [rsp+50h], xmm3
        0x48, 0xB8, // mov rax, imm64
    ];
    c.extend_from_slice(&(hook as u64).to_le_bytes());
    c.extend_from_slice(&[
        0xFF, 0xD0, // call rax
        0xF3, 0x0F, 0x6F, 0x44, 0x24, 0x20, // movdqu xmm0, [rsp+20h]
        0xF3, 0x0F, 0x6F, 0x4C, 0x24, 0x30, // movdqu xmm1, [rsp+30h]
        0xF3, 0x0F, 0x6F, 0x54, 0x24, 0x40, // movdqu xmm2, [rsp+40h]
        0xF3, 0x0F, 0x6F, 0x5C, 0x24, 0x50, // movdqu xmm3, [rsp+50h]
        0x48, 0x83, 0xC4, 0x68, // add rsp, 0x68
        0x41, 0x59, // pop r9
        0x41, 0x58, // pop r8
        0x5A, // pop rdx
        0x59, // pop rcx
        0x48, 0xB8, // mov rax, imm64
    ]);
    c.extend_from_slice(&(target as u64).to_le_bytes());
    c.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
    c
}

/// Ejecuta `f` en el hilo principal y espera el resultado.
pub fn run<R: Send + 'static>(
    f: impl FnOnce() -> R + Send + 'static,
    timeout: Duration,
) -> Result<R, String> {
    if !is_installed() {
        return Err("el gancho del hilo principal no está instalado (comando `hook`)".into());
    }
    let (tx, rx) = mpsc::channel();
    // Si vencemos el tiempo de espera, el trabajo no tiene que correr más tarde por sorpresa.
    let cancelled = Arc::new(AtomicBool::new(false));
    let flag = cancelled.clone();
    let job: Job = Box::new(move || {
        if !flag.load(Ordering::SeqCst) {
            let _ = tx.send(f());
        }
    });
    QUEUE.lock().unwrap_or_else(|e| e.into_inner()).push(job);
    match rx.recv_timeout(timeout) {
        Ok(r) => Ok(r),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            cancelled.store(true, Ordering::SeqCst);
            // Pudo haber terminado justo ahora.
            rx.try_recv().map_err(|_| {
                "el hilo principal no ejecutó el trabajo a tiempo (¿pantalla de carga?); quedó cancelado".into()
            })
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("el trabajo falló (panic)".into()),
    }
}
