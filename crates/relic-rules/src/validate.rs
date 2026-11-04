//! Legalidad de una reliquia: ¿podría haberla generado el juego?
//!
//! Reglas (verificadas contra un save real, ver `docs/relevamiento.md`):
//! 1. El tipo existe y es aleatorio.
//! 2. Tiene exactamente un efecto por slot con tabla, en los primeros slots, sin huecos.
//! 3. Ninguna maldición sin efecto.
//! 4. Sin efectos repetidos ni dos efectos con el mismo `compatibilityId`.
//! 5. Existe una asignación 1 a 1 de efectos a tablas de slot donde cada efecto está en su
//!    tabla y su maldición cumple la tabla de maldición del slot (si la hay, la maldición es
//!    obligatoria y tiene que estar en ella; si no la hay, no puede haber maldición).
//! 6. Los efectos están ordenados por (`overrideEffectId`, ID), como los ordena el juego.

use std::fmt;

use crate::lang::{self, tr, Lang};

use crate::data::{EffectId, GameData, ItemId};
use crate::relic::{relic_kind, Relic, RelicKind, Slot};

/// Por qué una reliquia no es legal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    UnknownRelicType(ItemId),
    NotRandom(ItemId),
    WrongEffectCount {
        expected: usize,
        found: usize,
    },
    GapInSlots,
    CurseWithoutEffect {
        slot: usize,
    },
    UnknownEffect(EffectId),
    DuplicateEffect(EffectId),
    CompatibilityClash {
        a: EffectId,
        b: EffectId,
        compatibility_id: i32,
    },
    NoValidAssignment,
    WrongOrder,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let es = lang::get() == Lang::Es;
        match self {
            Violation::UnknownRelicType(id) if es => {
                write!(f, "tipo de reliquia desconocido ({id})")
            }
            Violation::UnknownRelicType(id) => write!(f, "unknown relic type ({id})"),
            Violation::NotRandom(id) if es => write!(f, "la reliquia {id} tiene efectos fijos"),
            Violation::NotRandom(id) => write!(f, "relic {id} has fixed effects"),
            Violation::WrongEffectCount { expected, found } if es => write!(
                f,
                "esta reliquia lleva {expected} efecto(s) y tiene {found}"
            ),
            Violation::WrongEffectCount { expected, found } => {
                write!(f, "this relic takes {expected} effect(s) and has {found}")
            }
            Violation::GapInSlots => write!(
                f,
                "{}",
                tr(
                    "hay un slot vacío entre efectos",
                    "there is an empty slot between effects"
                )
            ),
            Violation::CurseWithoutEffect { slot } if es => {
                write!(f, "el slot {} tiene maldición pero no efecto", slot + 1)
            }
            Violation::CurseWithoutEffect { slot } => {
                write!(f, "slot {} has a curse but no effect", slot + 1)
            }
            Violation::UnknownEffect(id) if es => write!(f, "efecto desconocido ({id})"),
            Violation::UnknownEffect(id) => write!(f, "unknown effect ({id})"),
            Violation::DuplicateEffect(id) if es => write!(f, "el efecto {id} está repetido"),
            Violation::DuplicateEffect(id) => write!(f, "effect {id} is repeated"),
            Violation::CompatibilityClash { a, b, .. } if es => write!(
                f,
                "los efectos {a} y {b} no pueden ir juntos en una reliquia"
            ),
            Violation::CompatibilityClash { a, b, .. } => {
                write!(f, "effects {a} and {b} can't be on the same relic")
            }
            Violation::NoValidAssignment => write!(
                f,
                "{}",
                tr(
                    "esta combinación de efectos no puede salir en este tipo de reliquia",
                    "this combination of effects can't roll on this type of relic"
                )
            ),
            Violation::WrongOrder => write!(
                f,
                "{}",
                tr(
                    "los efectos no están en el orden del juego",
                    "the effects are not in the game's order"
                )
            ),
        }
    }
}

impl std::error::Error for Violation {}

/// Verifica que la reliquia sea una combinación que el juego podría generar.
pub fn validate(data: &GameData, relic: &Relic) -> Result<(), Violation> {
    let size = match relic_kind(data, relic.item_id) {
        RelicKind::Random { size, .. } => size,
        RelicKind::Fixed => return Err(Violation::NotRandom(relic.item_id)),
        RelicKind::Unknown => return Err(Violation::UnknownRelicType(relic.item_id)),
    };
    let rt = &data.relic_types[&relic.item_id];

    // 2 y 3: forma de los slots.
    for (i, slot) in relic.slots.iter().enumerate() {
        if slot.effect.is_none() && slot.curse.is_some() {
            return Err(Violation::CurseWithoutEffect { slot: i });
        }
    }
    let found = relic.effect_count();
    if found != size {
        return Err(Violation::WrongEffectCount {
            expected: size,
            found,
        });
    }
    if relic.slots[..size].iter().any(|s| s.effect.is_none()) {
        return Err(Violation::GapInSlots);
    }

    // 4: efectos conocidos, sin repetidos ni choques de compatibilidad.
    let effects: Vec<EffectId> = relic.effects().collect();
    for (i, &a) in effects.iter().enumerate() {
        let info_a = data.effects.get(&a).ok_or(Violation::UnknownEffect(a))?;
        for &b in &effects[i + 1..] {
            if a == b {
                return Err(Violation::DuplicateEffect(a));
            }
            let info_b = data.effects.get(&b).ok_or(Violation::UnknownEffect(b))?;
            if info_a.compatibility_id != -1 && info_a.compatibility_id == info_b.compatibility_id {
                return Err(Violation::CompatibilityClash {
                    a,
                    b,
                    compatibility_id: info_a.compatibility_id,
                });
            }
        }
    }

    // 5: asignación de efectos a tablas de slot.
    let tables: Vec<usize> = (0..3).filter(|&i| rt.slot_tables[i] >= 0).collect();
    let fits = |slot: &Slot, table_idx: usize| -> bool {
        let effect = slot.effect.expect("slots[..size] ya verificados");
        if !data.table_contains(rt.slot_tables[table_idx], effect) {
            return false;
        }
        match (rt.curse_tables[table_idx], slot.curse) {
            (t, None) if t < 0 => true,
            (t, Some(c)) if t >= 0 => data.table_contains(t, c),
            _ => false,
        }
    };
    let assigned = permutations(tables.len())
        .into_iter()
        .any(|perm| (0..size).all(|i| fits(&relic.slots[i], tables[perm[i]])));
    if !assigned {
        return Err(Violation::NoValidAssignment);
    }

    // 6: orden.
    if effects
        .windows(2)
        .any(|w| data.sort_key(w[0]) >= data.sort_key(w[1]))
    {
        return Err(Violation::WrongOrder);
    }

    Ok(())
}

/// Reordena los slots con efecto como los ordena el juego. Cada maldición viaja con su efecto.
pub fn normalize(data: &GameData, relic: &mut Relic) {
    let mut filled: Vec<Slot> = relic
        .slots
        .iter()
        .copied()
        .filter(|s| s.effect.is_some())
        .collect();
    filled.sort_by_key(|s| data.sort_key(s.effect.expect("filtrado arriba")));
    let mut slots = [Slot::EMPTY; 3];
    for (dst, src) in slots.iter_mut().zip(filled) {
        *dst = src;
    }
    relic.slots = slots;
}

/// Todas las permutaciones de `0..n` (n ≤ 3, así que son a lo sumo 6).
fn permutations(n: usize) -> Vec<Vec<usize>> {
    fn go(prefix: &mut Vec<usize>, used: &mut [bool], out: &mut Vec<Vec<usize>>) {
        if prefix.len() == used.len() {
            out.push(prefix.clone());
            return;
        }
        for i in 0..used.len() {
            if !used[i] {
                used[i] = true;
                prefix.push(i);
                go(prefix, used, out);
                prefix.pop();
                used[i] = false;
            }
        }
    }
    let mut out = Vec::new();
    go(&mut Vec::new(), &mut vec![false; n], &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::sample_data;

    #[test]
    fn permutations_de_tres() {
        assert_eq!(permutations(3).len(), 6);
        assert_eq!(permutations(1), vec![vec![0]]);
    }

    #[test]
    fn reliquia_normal_valida() {
        let data = sample_data();
        let r = Relic::new(
            1,
            [
                Slot::new(10, None),
                Slot::new(11, None),
                Slot::new(12, None),
            ],
        );
        assert_eq!(validate(&data, &r), Ok(()));
    }

    #[test]
    fn orden_incorrecto() {
        let data = sample_data();
        let r = Relic::new(
            1,
            [
                Slot::new(11, None),
                Slot::new(10, None),
                Slot::new(12, None),
            ],
        );
        assert_eq!(validate(&data, &r), Err(Violation::WrongOrder));
        let mut fixed = r.clone();
        normalize(&data, &mut fixed);
        assert_eq!(validate(&data, &fixed), Ok(()));
    }

    #[test]
    fn choque_de_compatibilidad() {
        let data = sample_data();
        // 12 y 13 comparten compatibilityId 500 (por ejemplo, Vigor +1 y Vigor +2).
        let r = Relic::new(
            1,
            [
                Slot::new(10, None),
                Slot::new(12, None),
                Slot::new(13, None),
            ],
        );
        assert!(matches!(
            validate(&data, &r),
            Err(Violation::CompatibilityClash { .. })
        ));
    }

    #[test]
    fn cantidad_de_efectos() {
        let data = sample_data();
        let r = Relic::new(1, [Slot::new(10, None), Slot::new(11, None), Slot::EMPTY]);
        assert_eq!(
            validate(&data, &r),
            Err(Violation::WrongEffectCount {
                expected: 3,
                found: 2
            })
        );
    }

    #[test]
    fn reliquia_fija_no_es_valida_para_el_mod() {
        let data = sample_data();
        let r = Relic::new(9, [Slot::new(10, None), Slot::EMPTY, Slot::EMPTY]);
        assert_eq!(validate(&data, &r), Err(Violation::NotRandom(9)));
    }

    #[test]
    fn deep_potenciado_exige_maldicion() {
        let data = sample_data();
        // Tipo 2: slot 1 potenciado (tabla 200 + maldición 300), slot 2 normal (tabla 210).
        let ok = Relic::new(
            2,
            [Slot::new(20, Some(30)), Slot::new(22, None), Slot::EMPTY],
        );
        assert_eq!(validate(&data, &ok), Ok(()));

        let sin_maldicion = Relic::new(2, [Slot::new(20, None), Slot::new(22, None), Slot::EMPTY]);
        assert_eq!(
            validate(&data, &sin_maldicion),
            Err(Violation::NoValidAssignment)
        );

        let maldicion_de_mas = Relic::new(
            2,
            [
                Slot::new(20, Some(30)),
                Slot::new(22, Some(31)),
                Slot::EMPTY,
            ],
        );
        assert_eq!(
            validate(&data, &maldicion_de_mas),
            Err(Violation::NoValidAssignment)
        );

        let dos_potenciados = Relic::new(
            2,
            [
                Slot::new(20, Some(30)),
                Slot::new(21, Some(31)),
                Slot::EMPTY,
            ],
        );
        assert_eq!(
            validate(&data, &dos_potenciados),
            Err(Violation::NoValidAssignment)
        );
    }
}
