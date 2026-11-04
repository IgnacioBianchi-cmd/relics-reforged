//! Rastreador de escrituras sin debugger.
//!
//! Marca con `PAGE_GUARD` la página que contiene un objeto (por ejemplo, una reliquia). Cada
//! acceso a esa página dispara una excepción que atiende nuestro manejador vectorizado: si es una
//! escritura dentro del objeto, anota la instrucción (RIP), algunos registros y las direcciones de
//! retorno candidatas que encuentra en la pila. Después pide un paso a paso (flag TF) para volver
//! a armar la guarda en cuanto la instrucción termina.
//!
//! Sirve para descubrir qué código del juego modifica o borra una reliquia (por ejemplo, al
//! venderla) sin adjuntar un debugger, que el juego detecta y lo hace crashear.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, Once};

use crate::win::{self, ExceptionPointers, PAGE_GUARD};

const STATUS_GUARD_PAGE_VIOLATION: u32 = 0x8000_0001;
const STATUS_SINGLE_STEP: u32 = 0x8000_0004;
const CONTINUE_EXECUTION: i32 = -1;
const CONTINUE_SEARCH: i32 = 0;
const TRAP_FLAG: u32 = 0x100;
const MAX_HITS: usize = 512;
const STACK_SLOTS: usize = 24;

// Offsets en EXCEPTION_RECORD y CONTEXT (x64).
const REC_INFO: usize = 0x20;
const CTX_EFLAGS: usize = 0x44;
const CTX_RCX: usize = 0x80;
const CTX_RDX: usize = 0x88;
const CTX_RBX: usize = 0x90;
const CTX_RSP: usize = 0x98;
const CTX_RSI: usize = 0xA8;
const CTX_RDI: usize = 0xB0;
const CTX_R8: usize = 0xB8;
const CTX_R9: usize = 0xC0;
const CTX_RIP: usize = 0xF8;

#[derive(Clone, Debug)]
pub struct Hit {
    pub thread: u32,
    pub rip: usize,
    /// Dirección escrita.
    pub addr: usize,
    /// rcx, rdx, r8, r9, rbx, rsi, rdi en el momento de la escritura.
    pub regs: [usize; 7],
    /// Valores de la pila que apuntan dentro de `nightreign.exe` (candidatos a direcciones de
    /// retorno). Array fijo: el manejador no puede pedir memoria (podría estar interrumpiendo
    /// al propio allocator del juego).
    pub stack: [usize; STACK_SLOTS],
    pub stack_len: usize,
    /// `false` si es una lectura (solo en modo lecturas).
    pub write: bool,
    /// Cuántas veces pasó esa misma instrucción (se agrupa por RIP y tipo de acceso).
    pub count: u32,
}

static INSTALL: Once = Once::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);
static READ_MODE: AtomicBool = AtomicBool::new(false);
static PAGE: AtomicUsize = AtomicUsize::new(0);
static PROTECT: AtomicUsize = AtomicUsize::new(0);
static START: AtomicUsize = AtomicUsize::new(0);
static END: AtomicUsize = AtomicUsize::new(0);
static MOD_START: AtomicUsize = AtomicUsize::new(0);
static MOD_END: AtomicUsize = AtomicUsize::new(0);
static READS: AtomicU64 = AtomicU64::new(0);
static OUTSIDE: AtomicU64 = AtomicU64::new(0);
static DROPPED: AtomicU64 = AtomicU64::new(0);
static HITS: Mutex<Vec<Hit>> = Mutex::new(Vec::new());

thread_local! {
    static REARM: Cell<bool> = const { Cell::new(false) };
}

/// Contadores desde que se armó la guarda: (lecturas en el objeto, accesos fuera del objeto
/// pero en la misma página, escrituras descartadas por falta de lugar).
pub fn counters() -> (u64, u64, u64) {
    (
        READS.load(Ordering::Relaxed),
        OUTSIDE.load(Ordering::Relaxed),
        DROPPED.load(Ordering::Relaxed),
    )
}

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Empieza a vigilar `[start, start + len)`. El objeto no puede cruzar el borde de una página.
/// Con `reads`, también anota las lecturas (agrupadas por instrucción).
pub fn watch(start: usize, len: usize, module: (usize, usize), reads: bool) -> Result<(), String> {
    if is_active() {
        return Err("ya hay una vigilancia activa".into());
    }
    let page = start & !0xFFF;
    if (start + len - 1) & !0xFFF != page {
        return Err("el objeto cruza dos páginas".into());
    }
    let protect = win::page_protection(page).ok_or("la página no está en memoria")?;
    if protect & PAGE_GUARD != 0 {
        return Err("la página ya tiene PAGE_GUARD (¿la usa el juego?)".into());
    }
    INSTALL.call_once(|| {
        // SAFETY: el manejador es una función estática que vive todo el proceso.
        unsafe { win::AddVectoredExceptionHandler(1, handler) };
    });
    {
        let mut hits = HITS.lock().unwrap_or_else(|e| e.into_inner());
        hits.clear();
        hits.reserve(MAX_HITS);
    }
    READ_MODE.store(reads, Ordering::Relaxed);
    READS.store(0, Ordering::Relaxed);
    OUTSIDE.store(0, Ordering::Relaxed);
    DROPPED.store(0, Ordering::Relaxed);
    MOD_START.store(module.0, Ordering::Relaxed);
    MOD_END.store(module.1, Ordering::Relaxed);
    START.store(start, Ordering::Relaxed);
    END.store(start + len, Ordering::Relaxed);
    PROTECT.store(protect as usize, Ordering::Relaxed);
    PAGE.store(page, Ordering::Relaxed);
    ACTIVE.store(true, Ordering::SeqCst);
    if !arm(page, protect) {
        ACTIVE.store(false, Ordering::SeqCst);
        return Err("VirtualProtect falló".into());
    }
    Ok(())
}

/// Deja de vigilar y saca la guarda de la página.
pub fn unwatch() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    let page = PAGE.load(Ordering::Relaxed);
    let protect = PROTECT.load(Ordering::Relaxed) as u32;
    let mut old = 0;
    // SAFETY: restauramos la protección original de una página que existe.
    unsafe { win::VirtualProtect(page as *mut _, 0x1000, protect, &mut old) };
    // PAGE queda seteada a propósito: si llega tarde una excepción de esta página, la atendemos.
}

/// Saca las escrituras anotadas hasta ahora.
pub fn take_hits() -> Vec<Hit> {
    let mut hits = HITS.lock().unwrap_or_else(|e| e.into_inner());
    let out = hits.clone();
    hits.clear();
    out
}

fn arm(page: usize, protect: u32) -> bool {
    let mut old = 0;
    // SAFETY: cambiamos la protección de una página existente, conservando la original + guarda.
    unsafe { win::VirtualProtect(page as *mut _, 0x1000, protect | PAGE_GUARD, &mut old) != 0 }
}

/// # Safety
/// La llama Windows con punteros válidos a EXCEPTION_RECORD y CONTEXT.
unsafe extern "system" fn handler(info: *mut ExceptionPointers) -> i32 {
    let rec = (*info).record;
    let ctx = (*info).context;
    let code = *(rec as *const u32);
    match code {
        STATUS_GUARD_PAGE_VIOLATION => {
            let kind = *(rec.add(REC_INFO) as *const usize); // 0 lectura, 1 escritura, 8 ejecución
            let addr = *(rec.add(REC_INFO + 8) as *const usize);
            let page = PAGE.load(Ordering::Relaxed);
            if page == 0 || addr & !0xFFF != page {
                return CONTINUE_SEARCH; // no es nuestra página
            }
            if !ACTIVE.load(Ordering::Relaxed) {
                // Llegó tarde: la guarda ya se consumió, dejamos seguir sin rearmar.
                return CONTINUE_EXECUTION;
            }
            let inside =
                addr >= START.load(Ordering::Relaxed) && addr < END.load(Ordering::Relaxed);
            let write = kind == 1;
            if !inside {
                OUTSIDE.fetch_add(1, Ordering::Relaxed);
            } else {
                if !write {
                    READS.fetch_add(1, Ordering::Relaxed);
                }
                if write || READ_MODE.load(Ordering::Relaxed) {
                    record(ctx, addr, write);
                }
            }
            let eflags = ctx.add(CTX_EFLAGS) as *mut u32;
            *eflags |= TRAP_FLAG;
            REARM.with(|r| r.set(true));
            CONTINUE_EXECUTION
        }
        STATUS_SINGLE_STEP if REARM.with(|r| r.get()) => {
            REARM.with(|r| r.set(false));
            if ACTIVE.load(Ordering::Relaxed) {
                arm(
                    PAGE.load(Ordering::Relaxed),
                    PROTECT.load(Ordering::Relaxed) as u32,
                );
            }
            CONTINUE_EXECUTION
        }
        _ => CONTINUE_SEARCH,
    }
}

unsafe fn record(ctx: *mut u8, addr: usize, write: bool) {
    let reg = |off: usize| *(ctx.add(off) as *const usize);
    let Ok(mut hits) = HITS.try_lock() else {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let rip = reg(CTX_RIP);
    if let Some(h) = hits.iter_mut().find(|h| h.rip == rip && h.write == write) {
        h.count = h.count.saturating_add(1);
        return;
    }
    if hits.len() >= MAX_HITS {
        DROPPED.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let (lo, hi) = (
        MOD_START.load(Ordering::Relaxed),
        MOD_END.load(Ordering::Relaxed),
    );
    let rsp = reg(CTX_RSP);
    let mut stack = [0usize; STACK_SLOTS];
    let mut stack_len = 0;
    // Leemos la pila con ReadProcessMemory: si nos pasamos del final, no hay excepción.
    'scan: for chunk in 0..16 {
        let Some(values) = win::read::<[usize; 32]>(rsp + chunk * 256) else {
            break;
        };
        for v in values {
            if v >= lo && v < hi {
                stack[stack_len] = v;
                stack_len += 1;
                if stack_len == STACK_SLOTS {
                    break 'scan;
                }
            }
        }
    }
    hits.push(Hit {
        thread: win::GetCurrentThreadId(),
        rip,
        write,
        count: 1,
        addr,
        regs: [
            reg(CTX_RCX),
            reg(CTX_RDX),
            reg(CTX_R8),
            reg(CTX_R9),
            reg(CTX_RBX),
            reg(CTX_RSI),
            reg(CTX_RDI),
        ],
        stack,
        stack_len,
    });
}
