//! Búsquedas en la memoria del juego, para ubicar estructuras que no están a la vista.
//!
//! - `find <u32> [<u32> ...]`: secuencias de valores u32 consecutivos (hex, `*` = cualquiera),
//!   alineadas a 4, en toda la memoria privada escribible.
//! - `ptrs <dirección>`: qwords de PlayerGameData (primeros 128 KB) que apuntan a esa dirección
//!   o hasta 64 KB antes (es decir, a un objeto que la contiene).

use std::fmt::Write as _;

use crate::game::Game;
use crate::win;

const MAX_HITS: usize = 64;
const CHUNK: usize = 0x10_0000;

pub fn find(game: &Game, words: &[&str]) -> Result<String, String> {
    let pattern: Vec<Option<u32>> = words
        .iter()
        .map(|w| {
            if *w == "*" {
                Ok(None)
            } else {
                u32::from_str_radix(w.trim_start_matches("0x"), 16)
                    .map(Some)
                    .map_err(|_| format!("valor inválido: {w}"))
            }
        })
        .collect::<Result<_, _>>()?;
    if pattern.iter().all(Option::is_none) {
        return Err("hace falta al menos un valor concreto".into());
    }
    let len = pattern.len() * 4;
    // Un único buffer reutilizado: así la búsqueda no encuentra copias suyas en el heap.
    let mut buf = vec![0u8; CHUNK + len];
    let own = (buf.as_ptr() as usize, buf.as_ptr() as usize + buf.len());
    let mut hits = Vec::new();
    let mut scanned = 0usize;
    'outer: for (base, size) in win::writable_regions() {
        let mut pos = 0;
        while pos < size {
            // Se lee con solapamiento para no perder coincidencias entre trozos.
            let take = (CHUNK + len).min(size - pos);
            win::read_into(base + pos, &mut buf[..take]);
            scanned += take.min(CHUNK);
            let bytes = &buf[..take];
            for off in (0..bytes.len().saturating_sub(len - 1)).step_by(4) {
                let matches = pattern.iter().enumerate().all(|(i, p)| {
                    p.is_none_or(|v| {
                        let o = off + i * 4;
                        u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()) == v
                    })
                });
                let addr = base + pos + off;
                if matches && !(addr >= own.0 && addr < own.1) {
                    hits.push(addr);
                    if hits.len() >= MAX_HITS {
                        break 'outer;
                    }
                }
            }
            pos += CHUNK;
        }
    }
    let pgd = game.player_game_data().unwrap_or(0);
    let mut out = format!(
        "{} coincidencias en {} MB (PlayerGameData en {pgd:#X}):",
        hits.len(),
        scanned / 0x10_0000
    );
    for h in hits {
        let rel = if h >= pgd && h < pgd + 0x2_0000 {
            format!(" (pgd+{:X})", h - pgd)
        } else {
            String::new()
        };
        let _ = write!(out, "\n  {h:#X}{rel}");
    }
    Ok(out)
}

/// Resuelve una dirección escrita por el jugador: hex, `pgd`, `gdm` o `nr` (base del módulo),
/// con un desplazamiento opcional (`pgd+2F4`).
pub fn parse_addr(game: &Game, s: &str) -> Result<usize, String> {
    let (base, off) = match s.split_once('+') {
        Some((b, o)) => (
            b,
            usize::from_str_radix(o.trim_start_matches("0x"), 16)
                .map_err(|_| format!("desplazamiento inválido: {o}"))?,
        ),
        None => (s, 0),
    };
    let base = match base {
        "pgd" => game
            .player_game_data()
            .ok_or("no encuentro PlayerGameData")?,
        "gdm" => game.game_data_man().ok_or("no encuentro GameDataMan")?,
        "nr" => game.base(),
        hex => usize::from_str_radix(hex.trim_start_matches("0x"), 16)
            .map_err(|_| format!("dirección inválida: {hex}"))?,
    };
    Ok(base + off)
}

/// Muestra `len` bytes desde `addr` como u32 en hex.
pub fn mem(addr: usize, len: usize) -> String {
    let len = len.clamp(4, 0x800);
    let (bytes, failed) = win::read_bytes(addr, len);
    let mut out = format!("{addr:#X} ({len:#X} bytes, {failed} páginas ilegibles):");
    for (i, row) in bytes.chunks(16).enumerate() {
        let _ = write!(out, "\n  +{:03X} ", i * 16);
        for w in row.chunks(4) {
            let mut b = [0u8; 4];
            b[..w.len()].copy_from_slice(w);
            let _ = write!(out, " {:08X}", u32::from_le_bytes(b));
        }
    }
    out
}

/// Qwords en `[base, base+len)` que apuntan a `target` o hasta 64 KB antes. `base` puede ser
/// `pgd`, `gdm`, `nr` (todo el ejecutable: variables globales) o una dirección.
pub fn ptrs(game: &Game, target: usize, base: &str, len: Option<usize>) -> Result<String, String> {
    let (start, len, label) = match base {
        "nr" => {
            let size = win::pe_info(game.base()).map_or(0, |(_, s)| s as usize);
            (game.base(), size, "nr".to_string())
        }
        other => (
            parse_addr(game, other)?,
            len.unwrap_or(0x2_0000),
            other.to_string(),
        ),
    };
    let mut out = format!(
        "punteros en {label} ({start:#X}, {len:#X} bytes) hacia {target:#X} o hasta 64 KB antes:"
    );
    let mut n = 0;
    let mut buf = vec![0u8; CHUNK];
    let mut pos = 0;
    while pos < len {
        let take = CHUNK.min(len - pos);
        win::read_into(start + pos, &mut buf[..take]);
        for off in (0..take.saturating_sub(7)).step_by(8) {
            let v = usize::from_le_bytes(buf[off..off + 8].try_into().unwrap());
            if v <= target && target - v < 0x1_0000 {
                n += 1;
                if n <= MAX_HITS {
                    let _ = write!(
                        out,
                        "\n  {label}+{:X} = {v:#X} (objetivo = +{:X})",
                        pos + off,
                        target - v
                    );
                }
            }
        }
        pos += take;
    }
    if n == 0 {
        out.push_str("\n  ninguno");
    }
    Ok(out)
}
