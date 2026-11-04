//! Nombres de efectos y reliquias desde los textos cargados por el juego (FMG), en el idioma del
//! jugador. Así el mod no necesita los CSV exportados con Smithbox.
//!
//! Los FMG cargados están en un array con una firma propia: un puntero a sí mismo, la cantidad de
//! categorías (0x206) y después un puntero a FMG (o 0) por categoría. No hay un puntero estático
//! que llegue a él, pero el juego lo reserva cerca de PlayerGameData (a ~2,5 MB en las sesiones
//! probadas), así que se busca primero ahí y, si no aparece, en toda la memoria.
//!
//! En memoria el FMG es igual al archivo (versión 2), salvo que el juego reemplaza el offset de la
//! tabla de textos (`+0x18`) por su dirección.

use std::collections::HashMap;

use relic_rules::GameData;

use crate::win::{read, read_bytes, read_into, writable_regions};

/// Cuánto alrededor de PlayerGameData se busca primero el array de FMG.
const NEAR_PGD: usize = 0x400_0000;
/// Cantidades de categorías que se aceptan (Nightreign 1.3.3.0 tiene 0x206).
const MIN_CATEGORIES: usize = 0x100;
const MAX_CATEGORIES: usize = 0x1000;
const MAX_FMG_SIZE: usize = 0x80_0000;

/// Un FMG cargado: ID → texto.
pub struct Fmg {
    pub texts: HashMap<i32, String>,
}

/// Lee un FMG de la memoria si la cabecera es válida.
fn read_fmg(addr: usize) -> Option<Fmg> {
    let head: [u8; 0x28] = read(addr)?;
    if head[0..4] != [0, 0, 2, 0] || head[8] != 1 {
        return None;
    }
    let u32_at = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let u64_at = |b: &[u8], o: usize| u64::from_le_bytes(b[o..o + 8].try_into().unwrap());
    let size = u32_at(&head, 4) as usize;
    let groups = u32_at(&head, 0x0C) as usize;
    let strings = u32_at(&head, 0x10) as usize;
    let table = u64_at(&head, 0x18) as usize;
    let table = if table == addr + 0x28 + groups * 16 {
        table - addr
    } else {
        table
    };
    if size > MAX_FMG_SIZE || table != 0x28 + groups * 16 || table + strings * 8 > size {
        return None;
    }
    let (file, failed) = read_bytes(addr, size);
    if failed > 0 {
        return None;
    }
    let mut texts = HashMap::new();
    for g in 0..groups {
        let gb = 0x28 + g * 16;
        let first_index = u32_at(&file, gb) as usize;
        let first = u32_at(&file, gb + 4) as i32;
        let last = u32_at(&file, gb + 8) as i32;
        if last < first || first_index + (last - first) as usize >= strings {
            return None;
        }
        for k in 0..=(last - first) as usize {
            let mut off = u64_at(&file, table + (first_index + k) * 8) as usize;
            if off >= addr {
                off -= addr; // por si también los convirtió en direcciones
            }
            if off == 0 || off >= size {
                continue;
            }
            let units: Vec<u16> = file[off..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .take_while(|&u| u != 0)
                .take(1024)
                .collect();
            texts.insert(first + k as i32, String::from_utf16_lossy(&units));
        }
    }
    Some(Fmg { texts })
}

/// Busca el array de FMG en `[from, to)`: un qword que se apunta a sí mismo seguido de una cantidad
/// razonable, y cuyos slots son 0 o FMG válidos (al menos 20).
fn find_fmg_array(from: usize, to: usize) -> Option<(usize, usize)> {
    const CHUNK: usize = 0x10_0000;
    let mut buf = vec![0u8; CHUNK + 16];
    for (base, size) in writable_regions() {
        let (lo, hi) = (base.max(from), (base + size).min(to));
        let mut pos = lo;
        while pos < hi {
            let take = (CHUNK + 16).min(base + size - pos);
            read_into(pos, &mut buf[..take]);
            for off in (0..take.saturating_sub(15)).step_by(8) {
                let q = usize::from_le_bytes(buf[off..off + 8].try_into().unwrap());
                if q != pos + off {
                    continue;
                }
                let count = usize::from_le_bytes(buf[off + 8..off + 16].try_into().unwrap());
                if !(MIN_CATEGORIES..MAX_CATEGORIES).contains(&count) {
                    continue;
                }
                let slots = pos + off + 0x10;
                let (bytes, failed) = read_bytes(slots, count * 8);
                if failed > 0 {
                    continue;
                }
                let valid = bytes
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| usize::from_le_bytes(*c))
                    .filter(|&p| p != 0)
                    .take(64)
                    .filter(|&p| read::<[u8; 4]>(p) == Some([0, 0, 2, 0]))
                    .count();
                if valid >= 20 {
                    return Some((slots, count));
                }
            }
            pos += CHUNK;
        }
    }
    None
}

/// Todos los FMG cargados por el juego, y dónde está su array.
pub fn loaded_fmgs(pgd: usize) -> (Vec<Fmg>, Option<usize>) {
    let near = find_fmg_array(pgd.saturating_sub(NEAR_PGD), pgd + NEAR_PGD);
    let Some((slots, count)) = near.or_else(|| find_fmg_array(0, usize::MAX)) else {
        return (Vec::new(), None);
    };
    let (bytes, _) = read_bytes(slots, count * 8);
    let fmgs = bytes
        .as_chunks::<8>()
        .0
        .iter()
        .map(|c| usize::from_le_bytes(*c))
        .filter(|&p| p > 0x1_0000)
        .filter_map(read_fmg)
        .collect();
    (fmgs, Some(slots))
}
/// Combina los FMG de nombres para esos IDs: se usan los que cubren al menos 20 IDs (o el 5 %), cuyos
/// textos son en su mayoría de esos IDs (así un FMG de otra cosa con IDs que coinciden por
/// casualidad no cuenta) y cortos (nombres, no descripciones), de mayor a menor cobertura; para cada ID gana el
/// primero que lo tiene. Devuelve los nombres y una descripción de los FMG usados.
fn merged_names(fmgs: &[Fmg], ids: &[i32]) -> (HashMap<i32, String>, String) {
    let min = (ids.len() / 20).max(20);
    let mut candidates: Vec<(usize, &Fmg, usize, usize)> = fmgs
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let found: Vec<&String> = ids
                .iter()
                .filter_map(|id| f.texts.get(id))
                .filter(|t| !t.is_empty())
                .collect();
            let avg = found.iter().map(|s| s.chars().count()).sum::<usize>() / found.len().max(1);
            (i, f, found.len(), avg)
        })
        .filter(|(_, f, n, avg)| *n >= min && *n * 2 >= f.texts.len() && *avg < 80)
        .collect();
    candidates.sort_by_key(|(_, _, n, _)| std::cmp::Reverse(*n));
    let mut names = HashMap::new();
    for id in ids {
        for (_, f, _, _) in &candidates {
            if let Some(t) = f.texts.get(id).filter(|t| !t.is_empty()) {
                // Algunos nombres traen saltos de línea para el menú del juego.
                let clean = t.split_whitespace().collect::<Vec<_>>().join(" ");
                names.insert(*id, clean);
                break;
            }
        }
    }
    let used = candidates
        .iter()
        .map(|(i, _, n, avg)| format!("#{i}: {n} ids, {avg} car."))
        .collect::<Vec<_>>()
        .join("; ");
    (names, used)
}

/// Completa los nombres de efectos y reliquias desde los textos del juego. Devuelve un resumen.
pub fn apply_names(data: &mut GameData, fmgs: &[Fmg]) -> String {
    let effect_ids: Vec<i32> = data.effects.values().map(|e| e.text_id).collect();
    let relic_ids: Vec<i32> = data.relic_types.keys().map(|&id| id as i32).collect();
    let (effect_names, effect_fmgs) = merged_names(fmgs, &effect_ids);
    let (relic_names, relic_fmgs) = merged_names(fmgs, &relic_ids);
    let mut effects = 0;
    for e in data.effects.values_mut() {
        if let Some(t) = effect_names.get(&e.text_id) {
            e.name = t.clone();
            effects += 1;
        }
    }
    let mut relics = 0;
    for (id, r) in data.relic_types.iter_mut() {
        if let Some(t) = relic_names.get(&(*id as i32)) {
            r.name = t.clone();
            relics += 1;
        }
    }
    format!(
        "{effects}/{} efectos [{effect_fmgs}], {relics}/{} reliquias [{relic_fmgs}]",
        data.effects.len(),
        data.relic_types.len()
    )
}
