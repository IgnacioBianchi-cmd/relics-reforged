//! Fotos de memoria para encontrar dónde guarda el juego algo (favoritas, equipadas...):
//! `snap` saca una foto de PlayerGameData y del inventario y, desde la segunda, lista qué
//! cambió respecto de la anterior.

use std::fmt::Write as _;
use std::sync::Mutex;

use crate::game::Game;
use crate::win;

/// Cuánto de PlayerGameData se fotografía (sobra: incluye lo que tenga atrás en memoria).
const PGD_SIZE: usize = 0x2_0000;
const MAX_LINES: usize = 300;

struct Snap {
    name: String,
    pgd_addr: usize,
    pgd: Vec<u8>,
    inv: Vec<(u32, [u32; 5])>,
}

static LAST: Mutex<Option<Snap>> = Mutex::new(None);

pub fn take(game: &Game, name: &str) -> Result<String, String> {
    let pgd_addr = game
        .player_game_data()
        .ok_or("no encuentro PlayerGameData")?;
    let (pgd, _) = win::read_bytes(pgd_addr, PGD_SIZE);
    let snap = Snap {
        name: name.to_string(),
        pgd_addr,
        pgd,
        inv: game.inventory_entries(),
    };
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = format!(
        "foto '{name}': PlayerGameData en {pgd_addr:#X}, {} entradas de inventario",
        snap.inv.len()
    );
    if let Some(prev) = last.as_ref() {
        let _ = write!(out, "\ncambios desde '{}':", prev.name);
        let mut lines = 0;
        if prev.pgd_addr != snap.pgd_addr {
            let _ = write!(out, "\n  PlayerGameData se movió: no comparo esa parte");
        } else {
            for off in (0..PGD_SIZE).step_by(4) {
                let a = u32::from_le_bytes(prev.pgd[off..off + 4].try_into().unwrap());
                let b = u32::from_le_bytes(snap.pgd[off..off + 4].try_into().unwrap());
                if a != b {
                    lines += 1;
                    if lines <= MAX_LINES {
                        let _ = write!(out, "\n  pgd+{off:05X}: {a:08X} -> {b:08X}");
                    }
                }
            }
        }
        for (index, entry) in &snap.inv {
            let before = prev.inv.iter().find(|(i, _)| i == index).map(|(_, e)| e);
            if before != Some(entry) {
                lines += 1;
                if lines <= MAX_LINES {
                    let _ = write!(out, "\n  inv[{index}]: {before:08X?} -> {entry:08X?}");
                }
            }
        }
        for (index, entry) in &prev.inv {
            if !snap.inv.iter().any(|(i, _)| i == index) {
                lines += 1;
                if lines <= MAX_LINES {
                    let _ = write!(out, "\n  inv[{index}]: {entry:08X?} -> (no está)");
                }
            }
        }
        let _ = write!(out, "\n  {lines} cambios en total");
    }
    *last = Some(snap);
    Ok(out)
}
