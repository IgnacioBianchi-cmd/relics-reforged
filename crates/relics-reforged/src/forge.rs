//! La forja completa: validar con el motor de reglas y aplicar en el hilo principal.
//!
//! `forge <donante> <slot> <receptora> <slot> <sacrificio1> <sacrificio2> <sacrificio3>`
//! (slots del 1 al 3, en el orden en que los muestra el juego).
//!
//! Orden de lo que se hace en el hilo principal, todo en un mismo frame:
//! 1. Volver a leer todas las reliquias y el murk, y abortar si algo cambió desde la validación.
//! 2. Borrar donante y sacrificios con `RemoveItem` del juego.
//! 3. Reescribir la receptora (efectos, maldiciones y SpEffects).
//! 4. Descontar el murk.
//!
//! No pide un guardado: como todo cambia en un mismo frame, el próximo guardado del juego toma
//! la forja completa o nada de ella.
//!
//! Las marcas de "forjada" se guardan en `relics_reforged_forjas.txt`, junto a la DLL. Cada una
//! lleva el número de orden de la entrada de inventario (estable entre sesiones) y la firma de
//! cómo quedó la reliquia (tipo, efectos y maldiciones): así, con varios saves (el normal y el de
//! Seamless, por ejemplo) cada uno ve solo las marcas de sus propias reliquias.

use std::collections::HashSet;
use std::path::Path;

use relic_rules::lang::{self, tr, Lang};
use relic_rules::{plan_transfer, EffectId, GameData, Relic, Rules, TransferRequest};

use crate::game::{Game, LiveRelic};
use crate::mainthread;

pub const FORGED_FILE: &str = "relics_reforged_forjas.txt";

/// Firma de una reliquia tal como está: tipo, efectos y maldiciones (FNV-1a de 64 bits).
pub fn signature(item_id: u32, effects: &[u32; 4], curses: &[u32; 4]) -> u64 {
    std::iter::once(item_id)
        .chain(effects.iter().copied())
        .chain(curses.iter().copied())
        .flat_map(u32::to_le_bytes)
        .fold(0xCBF2_9CE4_8422_2325u64, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100_0000_01B3)
        })
}

/// Una reliquia forjada.
struct Entry {
    /// Número de orden de su entrada de inventario.
    order: u32,
    /// Firma de cómo quedó después de la última forja. `None` en las líneas viejas (sin firma):
    /// valen para cualquier save hasta que esa reliquia se vuelva a forjar.
    sig: Option<u64>,
    effects: Vec<EffectId>,
    count: usize,
}

/// Una línea vieja (sin firma) es de esta reliquia si la reliquia tiene todos sus efectos forjados.
fn legacy_matches(e: &Entry, effects: &[u32; 4]) -> bool {
    !e.effects.is_empty() && e.effects.iter().all(|x| effects.contains(x))
}

/// Las reliquias forjadas, de todos los saves.
pub struct ForgedRegistry(Vec<Entry>);

impl ForgedRegistry {
    /// Formato de cada línea: `orden(hex) @firma(hex) efectos... #forjas`. Las líneas viejas no
    /// tienen firma ni cantidad (se cuenta una forja por efecto).
    pub fn load(dir: &Path) -> ForgedRegistry {
        let mut entries = Vec::new();
        if let Ok(text) = std::fs::read_to_string(dir.join(FORGED_FILE)) {
            for line in text.lines() {
                let mut parts = line.split_whitespace();
                let Some(Ok(order)) = parts.next().map(|k| u32::from_str_radix(k, 16)) else {
                    continue;
                };
                let (mut sig, mut count) = (None, None);
                let mut effects: Vec<EffectId> = Vec::new();
                for part in parts {
                    if let Some(s) = part.strip_prefix('@') {
                        sig = u64::from_str_radix(s, 16).ok();
                    } else if let Some(n) = part.strip_prefix('#') {
                        count = n.parse().ok();
                    } else {
                        effects.extend(part.parse::<EffectId>().ok());
                    }
                }
                let count = count.unwrap_or(effects.len());
                entries.push(Entry {
                    order,
                    sig,
                    effects,
                    count,
                });
            }
        }
        ForgedRegistry(entries)
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let mut text = String::from(
            "# orden_de_inventario(hex) @firma(hex) efectos_forjados... #cantidad_de_forjas\n",
        );
        for e in &self.0 {
            let list: Vec<String> = e.effects.iter().map(|x| x.to_string()).collect();
            let sig = e.sig.map_or(String::new(), |s| format!(" @{s:X}"));
            text.push_str(&format!(
                "{:X}{sig} {} #{}\n",
                e.order,
                list.join(" "),
                e.count
            ));
        }
        std::fs::write(dir.join(FORGED_FILE), text)
    }

    /// La marca de la reliquia con ese orden y esa firma. Una línea vieja (sin firma) vale solo si
    /// la reliquia todavía tiene todos sus efectos forjados: si no, es de otro save (por ejemplo,
    /// la misma reliquia forjada en Seamless y no en el save normal).
    fn find(&self, order: u32, sig: u64, effects: &[u32; 4]) -> Option<&Entry> {
        let mine = self
            .0
            .iter()
            .find(|e| e.order == order && e.sig == Some(sig));
        mine.or_else(|| {
            self.0
                .iter()
                .find(|e| e.order == order && e.sig.is_none() && legacy_matches(e, effects))
        })
    }

    /// Efectos forjados de la reliquia.
    pub fn get(&self, order: u32, sig: u64, effects: &[u32; 4]) -> &[EffectId] {
        self.find(order, sig, effects)
            .map_or(&[], |e| e.effects.as_slice())
    }

    /// Cuántas veces se forjó la reliquia.
    pub fn count(&self, order: u32, sig: u64, effects: &[u32; 4]) -> usize {
        self.find(order, sig, effects).map_or(0, |e| e.count)
    }

    /// Saca la marca de la reliquia con ese orden y esa firma (y la vieja sin firma que le
    /// corresponde, si hay; las de otros saves quedan).
    fn forget(&mut self, order: u32, sig: u64, effects: &[u32; 4]) {
        self.0.retain(|e| {
            !(e.order == order
                && (e.sig == Some(sig) || (e.sig.is_none() && legacy_matches(e, effects))))
        });
    }

    /// Anota una forja: la reliquia pasó de la firma `old` (con los efectos `old_effects`) a la
    /// `new`.
    fn record(
        &mut self,
        order: u32,
        (old, old_effects): (u64, &[u32; 4]),
        new: u64,
        effects: Vec<EffectId>,
        count: usize,
    ) {
        self.forget(order, old, old_effects);
        self.0.push(Entry {
            order,
            sig: Some(new),
            effects,
            count,
        });
    }
}

struct Snapshot {
    handle: u32,
    live: LiveRelic,
    order: u32,
}

fn snapshot(game: &Game, handle: u32) -> Result<Snapshot, String> {
    let (_, live) = game.relic_by_handle(handle).ok_or_else(|| {
        format!(
            "{} {handle:08X}",
            tr("no hay una reliquia", "there is no relic")
        )
    })?;
    let (_, entry) = game.inventory_entry(handle).ok_or_else(|| {
        format!(
            "{handle:08X} {}",
            tr("no está en el inventario", "is not in the inventory")
        )
    })?;
    if entry[1] != live.item_id | 0x8000_0000 || entry[2] != 1 {
        return Err(format!(
            "{} {handle:08X}: {entry:X?}",
            tr(
                "entrada de inventario inesperada para",
                "unexpected inventory entry for"
            )
        ));
    }
    Ok(Snapshot {
        handle,
        live,
        order: entry[3],
    })
}

fn to_relic(s: &Snapshot) -> Relic {
    Relic::from_raw(s.live.item_id, s.live.effects, s.live.curses)
}

/// Un pedido de forja: handles y slots (del 1 al 3) tal como los escribe el jugador.
pub struct Order {
    pub donor: u32,
    pub donor_slot: usize,
    pub target: u32,
    pub target_slot: usize,
    pub sacrifices: [u32; 3],
    /// Solo validar y mostrar el resultado, sin tocar nada.
    pub dry_run: bool,
}

pub fn forge(game: Game, data: &GameData, dll_dir: &Path, order: Order) -> Result<String, String> {
    let Order {
        donor,
        donor_slot,
        target,
        target_slot,
        sacrifices,
        dry_run,
    } = order;
    let handles = [donor, target, sacrifices[0], sacrifices[1], sacrifices[2]];
    if handles.iter().collect::<HashSet<_>>().len() != handles.len() {
        return Err(tr(
            "hay una reliquia repetida en el pedido",
            "the same relic was chosen twice",
        )
        .into());
    }
    if !(1..=3).contains(&donor_slot) || !(1..=3).contains(&target_slot) {
        return Err(tr("los slots van del 1 al 3", "slots go from 1 to 3").into());
    }
    game.check_game_functions()?;

    let snaps: Vec<Snapshot> = handles
        .iter()
        .map(|&h| snapshot(&game, h))
        .collect::<Result<_, _>>()?;
    // La donante y los sacrificios se destruyen: no pueden ser favoritas ni estar en uso.
    for (i, s) in snaps.iter().enumerate() {
        if i == 1 {
            continue; // la receptora sí puede (es justamente la que el jugador quiere mejorar)
        }
        if let Some(reason) = game.lock_reason(s.handle) {
            let role = if i == 0 {
                tr("la donante", "the donor").to_string()
            } else {
                format!("{} {}", tr("el sacrificio", "sacrifice"), i - 1)
            };
            let name = data
                .relic_types
                .get(&s.live.item_id)
                .map_or_else(|| format!("{:08X}", s.handle), |t| t.name.clone());
            return Err(format!(
                "{}: {role} ({name}) {reason}",
                tr("no se puede forjar", "can't forge"),
            ));
        }
    }
    let relics: Vec<Relic> = snaps.iter().map(to_relic).collect();
    let murk = game
        .murk()
        .ok_or(tr("no puedo leer el murk", "can't read the murk"))?;
    let mut registry = ForgedRegistry::load(dll_dir);
    let target_order = snaps[1].order;
    let sig_of = |s: &Snapshot| signature(s.live.item_id, &s.live.effects, &s.live.curses);
    let target_sig = sig_of(&snaps[1]);

    let sacrifice_refs: Vec<&Relic> = relics[2..].iter().collect();
    let plan = plan_transfer(
        data,
        &Rules::default(),
        &TransferRequest {
            donor: &relics[0],
            donor_slot: donor_slot - 1,
            target: &relics[1],
            target_slot: target_slot - 1,
            target_forged: registry.get(target_order, target_sig, &snaps[1].live.effects),
            target_forge_count: registry.count(target_order, target_sig, &snaps[1].live.effects),
            sacrifices: &sacrifice_refs,
            available_murk: murk,
        },
    )
    .map_err(|e| format!("{}: {e}", tr("no se puede forjar", "can't forge")))?;
    let sp = plan.new_target.sp_effects(data).ok_or(tr(
        "un efecto no está en los params",
        "an effect is missing from the params",
    ))?;
    let (effects, curses) = plan.new_target.to_raw();
    let new_murk = murk - plan.murk_cost;
    // Si la receptora pasa a la versión nueva de su tipo, se avisa y se escribe el tipo nuevo.
    let upgrade = plan.upgraded_from.map(|_| plan.new_target.item_id);
    let upgrade_note = match plan.upgraded_from {
        Some(old) => format!(
            "\n{} ({old} -> {})",
            tr(
                "La reliquia pasa a su versión nueva (las tablas actuales, con los efectos de los personajes nuevos).",
                "The relic is updated to its newer version (the current tables, with the newer characters' effects).",
            ),
            plan.new_target.item_id
        ),
        None => String::new(),
    };
    // Un efecto por línea (con su maldición), para que el menú los muestre apilados.
    let names = |r: &Relic| -> String {
        r.slots
            .iter()
            .filter_map(|s| {
                let text = crate::ui::effect_text(data, s.effect?, s.curse);
                Some(format!("\n{}{text}", crate::ui::EFFECT_LINE))
            })
            .collect()
    };
    let relic_name = data
        .relic_types
        .get(&relics[1].item_id)
        .map_or(tr("La receptora", "The target").to_string(), |t| {
            t.name.clone()
        });
    let forged_note = |count: usize| {
        let limit = Rules::default().forge_limit[relics[1].effect_count().clamp(1, 3) - 1];
        format!("{count}/{limit}")
    };
    if dry_run {
        let (names, cost, points, note) = (
            names(&plan.new_target),
            lang::number(plan.murk_cost),
            plan.sacrifice_points,
            forged_note(plan.new_target_forge_count),
        );
        let (murk, new_murk) = (lang::number(murk), lang::number(new_murk));
        return Ok(match lang::get() {
            Lang::Es => format!(
                "{relic_name} quedaría así:{names}\nCosto: {cost} de murk ({murk} -> {new_murk})\n\
                 Sacrificios: {points} puntos\nForjas: {note}{upgrade_note}"
            ),
            Lang::En => format!(
                "{relic_name} would become:{names}\nCost: {cost} murk ({murk} -> {new_murk})\n\
                 Sacrifices: {points} points\nForges: {note}{upgrade_note}"
            ),
        });
    }

    // Regla 11: solo en la Roundtable, con el menú de Relic Rites abierto.
    if !game.relic_rites_open() {
        return Err(tr(
            "la forja solo se puede usar con Ritos de reliquias abierto (en la Mesa Redonda)",
            "forging is only available with Relic Rites open (at the Roundtable)",
        )
        .into());
    }

    // Lo que se compara en el hilo principal antes de tocar nada.
    let expected: Vec<(u32, LiveRelic)> =
        snaps.iter().map(|s| (s.handle, s.live.clone())).collect();
    let to_remove: Vec<u32> = [donor].into_iter().chain(sacrifices).collect();

    let applied = mainthread::run(
        move || -> Result<(), String> {
            for (h, live) in &expected {
                match game.relic_by_handle(*h) {
                    Some((_, now)) if now == *live => {}
                    _ => {
                        return Err(format!(
                            "{h:08X} {}",
                            tr(
                                "cambió desde la validación; no se hizo nada",
                                "changed since validation; nothing was done"
                            )
                        ))
                    }
                }
                if game.inventory_entry(*h).is_none() {
                    return Err(format!(
                        "{h:08X} {}",
                        tr(
                            "ya no está en el inventario; no se hizo nada",
                            "is no longer in the inventory; nothing was done"
                        )
                    ));
                }
            }
            if game.murk() != Some(murk) {
                return Err(tr(
                    "el murk cambió desde la validación; no se hizo nada",
                    "the murk changed since validation; nothing was done",
                )
                .into());
            }
            for &h in &to_remove {
                if let Some(reason) = game.lock_reason(h) {
                    return Err(format!(
                        "{h:08X} {reason}; {}",
                        tr("no se hizo nada", "nothing was done")
                    ));
                }
            }
            // A partir de acá se modifica el juego.
            for &h in &to_remove {
                // SAFETY: estamos en el hilo principal, entre dos frames.
                let left = unsafe { game.remove_item(h)? };
                if left != 0 || game.relic_by_handle(h).is_some() {
                    return Err(format!(
                        "{} {h:08X} ({left})",
                        tr(
                            "ATENCIÓN: la forja quedó a medias, no se borró",
                            "WARNING: the forge is incomplete, could not remove"
                        )
                    ));
                }
            }
            if !game.write_slots(target, effects, curses, &sp) {
                return Err(tr(
                    "ATENCIÓN: no se pudo reescribir la receptora; la forja quedó a medias",
                    "WARNING: could not rewrite the target; the forge is incomplete",
                )
                .into());
            }
            if let Some(item_id) = upgrade {
                if !game.write_item_id(target, item_id) {
                    return Err(tr(
                        "ATENCIÓN: no se pudo cambiar el tipo de la receptora; la forja quedó a medias",
                        "WARNING: could not update the target's type; the forge is incomplete",
                    )
                    .into());
                }
            }
            let murk_addr = game.murk_addr().ok_or(tr(
                "ATENCIÓN: no se encontró el murk",
                "WARNING: murk not found",
            ))?;
            if !crate::win::write(murk_addr, new_murk) {
                return Err(tr(
                    "ATENCIÓN: no se pudo descontar el murk",
                    "WARNING: could not charge the murk",
                )
                .into());
            }
            // Sin pedido de guardado: todo cambió en un mismo frame, así que el próximo guardado
            // del juego toma la forja entera o nada de ella.
            Ok(())
        },
        std::time::Duration::from_secs(5),
    )?;
    applied?;

    let new_sig = signature(plan.new_target.item_id, &effects, &curses);
    registry.record(
        target_order,
        (target_sig, &snaps[1].live.effects),
        new_sig,
        plan.new_target_forged.clone(),
        plan.new_target_forge_count,
    );
    for s in snaps.iter().filter(|s| s.handle != target) {
        registry.forget(s.order, sig_of(s), &s.live.effects);
    }
    let registry_note = match registry.save(dll_dir) {
        Ok(()) => String::new(),
        Err(e) => format!(" (no pude guardar {FORGED_FILE}: {e})"),
    };

    let (names, cost, note, ids) = (
        names(&plan.new_target),
        lang::number(plan.murk_cost),
        forged_note(plan.new_target_forge_count),
        &plan.new_target_forged,
    );
    let (murk, new_murk) = (lang::number(murk), lang::number(new_murk));
    Ok(match lang::get() {
        Lang::Es => format!(
            "Forja hecha: {relic_name} ahora tiene:{names}\nMurk {murk} -> {new_murk} \
             (costo {cost}). Forjas: {note}. [{target:08X}, {ids:?}]{registry_note}{upgrade_note}"
        ),
        Lang::En => format!(
            "Forge done: {relic_name} now has:{names}\nMurk {murk} -> {new_murk} \
             (cost {cost}). Forges: {note}. [{target:08X}, {ids:?}]{registry_note}{upgrade_note}"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: [u32; 4] = [u32::MAX; 4];

    /// Firma y efectos de una reliquia de prueba.
    fn relic(effects: [u32; 4]) -> (u64, [u32; 4]) {
        (signature(123, &effects, &EMPTY), effects)
    }

    fn count(reg: &ForgedRegistry, order: u32, r: (u64, [u32; 4])) -> usize {
        reg.count(order, r.0, &r.1)
    }

    fn record(
        reg: &mut ForgedRegistry,
        order: u32,
        old: (u64, [u32; 4]),
        new: (u64, [u32; 4]),
        e: Vec<EffectId>,
        n: usize,
    ) {
        reg.record(order, (old.0, &old.1), new.0, e, n);
    }

    #[test]
    fn cada_save_ve_sus_marcas() {
        let mut reg = ForgedRegistry(Vec::new());
        let original = relic([1, 2, 3, u32::MAX]);
        // Save A: la reliquia del orden 7 se forja y queda con el efecto 9.
        let a = relic([9, 2, 3, u32::MAX]);
        record(&mut reg, 7, original, a, vec![9], 1);
        // Save B (copia de antes de la forja): la misma reliquia sigue como era.
        assert_eq!(count(&reg, 7, original), 0);
        assert_eq!(count(&reg, 7, a), 1);
        // En B se forja distinto: A conserva su marca.
        let b = relic([1, 8, 3, u32::MAX]);
        record(&mut reg, 7, original, b, vec![8], 1);
        assert_eq!(reg.get(7, a.0, &a.1), &[9]);
        assert_eq!(reg.get(7, b.0, &b.1), &[8]);
        // Reforjar en A suma sobre la marca de A.
        let a2 = relic([9, 5, 3, u32::MAX]);
        record(&mut reg, 7, a, a2, vec![5, 9], 2);
        assert_eq!(count(&reg, 7, a2), 2);
        assert_eq!(count(&reg, 7, a), 0);
        assert_eq!(count(&reg, 7, b), 1);
    }

    #[test]
    fn lineas_viejas_sin_firma() {
        let mut reg = ForgedRegistry(vec![Entry {
            order: 3,
            sig: None,
            effects: vec![4],
            count: 1,
        }]);
        // Valen para la reliquia que tiene el efecto forjado, en cualquier save...
        let forged = relic([4, 2, u32::MAX, u32::MAX]);
        assert_eq!(count(&reg, 3, forged), 1);
        // ...pero no para la misma reliquia sin forjar (otro save).
        let other_save = relic([1, 2, u32::MAX, u32::MAX]);
        assert_eq!(count(&reg, 3, other_save), 0);
        // Forjar la del otro save no borra la línea vieja.
        let other_new = relic([1, 7, u32::MAX, u32::MAX]);
        record(&mut reg, 3, other_save, other_new, vec![7], 1);
        assert_eq!(count(&reg, 3, forged), 1);
        assert_eq!(count(&reg, 3, other_new), 1);
        // Reforjar la marcada reemplaza la línea vieja por una con firma.
        let new = relic([4, 6, u32::MAX, u32::MAX]);
        record(&mut reg, 3, forged, new, vec![4, 6], 2);
        assert!(reg.0.iter().all(|e| e.sig.is_some()));
        assert_eq!(count(&reg, 3, new), 2);
    }
}
