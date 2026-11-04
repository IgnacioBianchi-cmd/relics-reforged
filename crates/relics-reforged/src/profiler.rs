//! Perfilador por muestreo, sin debugger: pausa cada hilo del proceso un instante, lee su RIP y
//! las direcciones de retorno candidatas de su pila, y lo reanuda. El resultado va a un archivo
//! de texto que analiza `disasm <volcado> profile <archivo>`.
//!
//! Mientras un hilo está pausado no pedimos memoria ni tomamos locks (podría tener tomado el del
//! heap): todo se escribe en buffers fijos y recién se guarda al reanudarlo.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use crate::win;

const STACK_SLOTS: usize = 32;

struct Sample {
    thread: u32,
    rip: usize,
    stack: [usize; STACK_SLOTS],
    stack_len: usize,
}

/// Muestrea todos los hilos (salvo el nuestro) durante `seconds`, cada `interval_ms`.
/// Devuelve el texto del archivo de salida y un resumen.
pub fn run(
    seconds: u64,
    interval_ms: u64,
    module: (usize, usize),
) -> Result<(String, String), String> {
    let own = unsafe { win::GetCurrentThreadId() };
    let threads = win::process_threads();
    if threads.is_empty() {
        return Err("no pude listar los hilos".into());
    }
    let mut out = String::new();
    let mut handles = Vec::new();
    for &tid in &threads {
        if tid == own {
            continue;
        }
        if let Some(h) = win::open_thread(tid) {
            let start = win::thread_start_address(h).unwrap_or(0);
            let _ = writeln!(out, "T {tid} {start:X}");
            handles.push((tid, h));
        }
    }
    let mut samples = Vec::with_capacity(4096);
    let end = Instant::now() + Duration::from_secs(seconds);
    let mut rounds = 0;
    while Instant::now() < end {
        for &(tid, h) in &handles {
            let mut s = Sample {
                thread: tid,
                rip: 0,
                stack: [0; STACK_SLOTS],
                stack_len: 0,
            };
            // SAFETY: entre suspender y reanudar solo leemos registros y memoria a buffers fijos.
            unsafe {
                if win::SuspendThread(h) == u32::MAX {
                    continue;
                }
                if let Some((rip, rsp)) = win::thread_rip_rsp(h) {
                    s.rip = rip;
                    'scan: for chunk in 0..8 {
                        let Some(values) = win::read::<[usize; 32]>(rsp + chunk * 256) else {
                            break;
                        };
                        for v in values {
                            if v >= module.0 && v < module.1 {
                                s.stack[s.stack_len] = v;
                                s.stack_len += 1;
                                if s.stack_len == STACK_SLOTS {
                                    break 'scan;
                                }
                            }
                        }
                    }
                }
                win::ResumeThread(h);
            }
            if s.rip != 0 {
                samples.push(s);
            }
        }
        rounds += 1;
        std::thread::sleep(Duration::from_millis(interval_ms));
    }
    for (_, h) in handles {
        win::close_handle(h);
    }
    for s in &samples {
        let _ = write!(out, "S {} {:X}", s.thread, s.rip);
        for a in &s.stack[..s.stack_len] {
            let _ = write!(out, " {a:X}");
        }
        out.push('\n');
    }
    let summary = format!(
        "{} hilos, {rounds} rondas, {} muestras",
        threads.len().saturating_sub(1),
        samples.len()
    );
    Ok((out, summary))
}
