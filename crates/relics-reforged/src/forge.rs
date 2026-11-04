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
//! Las marcas de "forjada" se guardan en `relics_reforged_forjas.txt`, junto a la DLL, indexadas
//! por el número de orden de la entrada de inventario (estable entre sesiones).

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use relic_rules::lang::{self, tr, Lang};
use relic_rules::{plan_transfer, EffectId, GameData, Relic, Rules, TransferRequest};

use crate::game::{Game, LiveRelic};
use crate::mainthread;

pub const FORGED_FILE: &str = "relics_reforged_forjas.txt";

/// Efectos forjados por número de orden de la reliquia.
pub struct ForgedRegistry(BTreeMap<u32, Vec<EffectId>>);

impl ForgedRegistry {
    pub fn load(dir: &Path) -> ForgedRegistry {
        let mut map = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(dir.join(FORGED_FILE)) {
            for line in text.lines() {
                let mut parts = line.split_whitespace();
                let Some(Ok(key)) = parts.next().map(|k| u32::from_str_radix(k, 16)) else {
                    continue;
                };
                let effects: Vec<EffectId> = parts.filter_map(|e| e.parse().ok()).collect();
                map.insert(key, effects);
            }
        }
        ForgedRegistry(map)
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let mut text = String::from("# orden_de_inventario(hex) efectos_forjados...\n");
        for (key, effects) in &self.0 {
            if effects.is_empty() {
                continue;
            }
            let list: Vec<String> = effects.iter().map(|e| e.to_string()).collect();
            text.push_str(&format!("{key:X} {}\n", list.join(" ")));
        }
        std::fs::write(dir.join(FORGED_FILE), text)
    }

    pub fn get(&self, key: u32) -> &[EffectId] {
        self.0.get(&key).map_or(&[], |v| v.as_slice())
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
            return Err(format!(
                "{}: {role} ({:08X}) {reason}",
                tr("no se puede forjar", "can't forge"),
                s.handle
            ));
        }
    }
    let relics: Vec<Relic> = snaps.iter().map(to_relic).collect();
    let murk = game
        .murk()
        .ok_or(tr("no puedo leer el murk", "can't read the murk"))?;
    let mut registry = ForgedRegistry::load(dll_dir);
    let target_order = snaps[1].order;

    let sacrifice_refs: Vec<&Relic> = relics[2..].iter().collect();
    let plan = plan_transfer(
        data,
        &Rules::default(),
        &TransferRequest {
            donor: &relics[0],
            donor_slot: donor_slot - 1,
            target: &relics[1],
            target_slot: target_slot - 1,
            target_forged: registry.get(target_order),
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
    let names = |r: &Relic| -> String {
        r.effects()
            .map(|e| {
                data.effects
                    .get(&e)
                    .map_or(e.to_string(), |i| crate::ui::clean_name(&i.name))
            })
            .collect::<Vec<_>>()
            .join("  |  ")
    };
    let relic_name = data
        .relic_types
        .get(&relics[1].item_id)
        .map_or(tr("La receptora", "The target").to_string(), |t| {
            t.name.clone()
        });
    let forged_note = |forged: &[EffectId]| {
        let limit = Rules::default().forge_limit[relics[1].effect_count().clamp(1, 3) - 1];
        format!(
            "{}/{limit} {}",
            forged.len(),
            tr("efectos forjados", "forged effects")
        )
    };
    if dry_run {
        let (names, cost, points, note) = (
            names(&plan.new_target),
            plan.murk_cost,
            plan.sacrifice_points,
            forged_note(&plan.new_target_forged),
        );
        return Ok(match lang::get() {
            Lang::Es => format!(
                "Resultado: {relic_name} quedaría con  {names}\nCosto: {cost} de murk \
                 ({murk} -> {new_murk}). Sacrificios: {points} puntos. Quedaría con {note}."
            ),
            Lang::En => format!(
                "Result: {relic_name} would have  {names}\nCost: {cost} murk \
                 ({murk} -> {new_murk}). Sacrifices: {points} points. It would have {note}."
            ),
        });
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

    registry
        .0
        .insert(target_order, plan.new_target_forged.clone());
    for s in snaps.iter().filter(|s| s.handle != target) {
        registry.0.remove(&s.order);
    }
    let registry_note = match registry.save(dll_dir) {
        Ok(()) => String::new(),
        Err(e) => format!(" (no pude guardar {FORGED_FILE}: {e})"),
    };

    let (names, cost, note, ids) = (
        names(&plan.new_target),
        plan.murk_cost,
        forged_note(&plan.new_target_forged),
        &plan.new_target_forged,
    );
    Ok(match lang::get() {
        Lang::Es => format!(
            "Forja hecha: {relic_name} ahora tiene  {names}\nMurk {murk} -> {new_murk} \
             (costo {cost}). {note}. [{target:08X}, {ids:?}]{registry_note}"
        ),
        Lang::En => format!(
            "Forge done: {relic_name} now has  {names}\nMurk {murk} -> {new_murk} \
             (cost {cost}). {note}. [{target:08X}, {ids:?}]{registry_note}"
        ),
    })
}
