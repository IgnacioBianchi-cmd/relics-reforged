//! Transferencia de un efecto de una reliquia donante a una receptora.
//!
//! El motor no toca el inventario: recibe las reliquias involucradas, valida todo y devuelve
//! un [`TransferPlan`] con la receptora nueva y el costo. Aplicarlo (borrar la donante y los
//! sacrificios, descontar murk, reescribir la receptora) es trabajo de quien lo llama.

use std::fmt;

use crate::data::{EffectId, GameData};
use crate::relic::{relic_kind, Relic, RelicKind};
use crate::validate::{normalize, validate, Violation};

/// Parámetros de balance. Se pueden ajustar sin tocar la lógica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    /// Cantidad exacta de reliquias de sacrificio por transferencia.
    pub sacrifice_count: usize,
    /// Puntos mínimos de sacrificio según el tamaño de la receptora (1, 2, 3 efectos).
    /// Cada sacrificio aporta tantos puntos como efectos tiene su tipo.
    pub min_points: [u32; 3],
    /// Murk según el tamaño de la receptora (1, 2, 3 efectos).
    pub murk_cost: [u32; 3],
    /// Multiplicador de murk si la receptora es Deep.
    pub deep_murk_multiplier: u32,
    /// Máximo de efectos forjados según el tamaño de la receptora (1, 2, 3 efectos).
    pub forge_limit: [usize; 3],
}

impl Default for Rules {
    fn default() -> Self {
        Rules {
            sacrifice_count: 3,
            min_points: [3, 5, 7],
            murk_cost: [2_500, 5_000, 10_000],
            deep_murk_multiplier: 2,
            forge_limit: [1, 1, 2],
        }
    }
}

/// Todo lo que hace falta para evaluar una transferencia.
#[derive(Debug, Clone)]
pub struct TransferRequest<'a> {
    pub donor: &'a Relic,
    /// Slot de la donante (0..3) cuyo efecto, con su maldición, se transfiere.
    pub donor_slot: usize,
    pub target: &'a Relic,
    /// Slot de la receptora (0..3) que se sobrescribe.
    pub target_slot: usize,
    /// Efectos de la receptora que ya fueron forjados (los registra el mod, no el juego).
    pub target_forged: &'a [EffectId],
    pub sacrifices: &'a [&'a Relic],
    pub available_murk: u32,
}

/// Resultado de una transferencia válida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPlan {
    /// La receptora con el efecto nuevo, ya en el orden del juego.
    pub new_target: Relic,
    /// Efectos forjados de la receptora después de la transferencia.
    pub new_target_forged: Vec<EffectId>,
    pub murk_cost: u32,
    /// Puntos que aportan los sacrificios (informativo, para la interfaz).
    pub sacrifice_points: u32,
}

/// Qué reliquia del pedido tiene el problema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Donor,
    Target,
    Sacrifice(usize),
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Donor => write!(f, "la donante"),
            Role::Target => write!(f, "la receptora"),
            Role::Sacrifice(i) => write!(f, "el sacrificio {}", i + 1),
        }
    }
}

/// Por qué una transferencia no se puede hacer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferError {
    /// La reliquia no es aleatoria (fija o desconocida).
    NotEligible(Role),
    Locked(Role),
    /// La misma reliquia aparece dos veces en el pedido.
    SameRelicTwice(Role, Role),
    /// Mezcla de normal y Deep.
    DeepMismatch(Role),
    /// La receptora no es legal antes de empezar (save modificado a mano, etc.).
    TargetInvalid(Violation),
    EmptyDonorSlot,
    InvalidTargetSlot,
    WrongSacrificeCount {
        expected: usize,
        found: usize,
    },
    NotEnoughPoints {
        needed: u32,
        have: u32,
    },
    ForgeLimitReached {
        limit: usize,
    },
    NotEnoughMurk {
        needed: u32,
        have: u32,
    },
    /// La reliquia resultante no podría haberla generado el juego.
    ResultInvalid(Violation),
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransferError::NotEligible(r) => write!(f, "{r} tiene efectos fijos y no se puede usar"),
            TransferError::Locked(r) => write!(f, "{r} es favorita o está equipada"),
            TransferError::SameRelicTwice(a, b) => write!(f, "{a} y {b} son la misma reliquia"),
            TransferError::DeepMismatch(r) => {
                write!(f, "{r} no es del mismo tipo (normal/Deep) que la receptora")
            }
            TransferError::TargetInvalid(v) => write!(f, "la receptora no es válida: {v}"),
            TransferError::EmptyDonorSlot => write!(f, "ese slot de la donante está vacío"),
            TransferError::InvalidTargetSlot => write!(f, "ese slot de la receptora no existe"),
            TransferError::WrongSacrificeCount { expected, found } => {
                write!(f, "hacen falta {expected} reliquias de sacrificio (elegiste {found})")
            }
            TransferError::NotEnoughPoints { needed, have } => write!(
                f,
                "los sacrificios suman {have} punto(s) y hacen falta {needed} (chica 1, mediana 2, grande 3)"
            ),
            TransferError::ForgeLimitReached { limit } => {
                write!(f, "esta reliquia ya tiene el máximo de {limit} efecto(s) forjado(s)")
            }
            TransferError::NotEnoughMurk { needed, have } => {
                write!(f, "hacen falta {needed} de murk y tenés {have}")
            }
            TransferError::ResultInvalid(v) => write!(f, "la reliquia resultante no sería válida: {v}"),
        }
    }
}

impl std::error::Error for TransferError {}

/// Evalúa una transferencia y, si es válida, devuelve qué hay que aplicar.
pub fn plan_transfer(
    data: &GameData,
    rules: &Rules,
    req: &TransferRequest<'_>,
) -> Result<TransferPlan, TransferError> {
    // Tipos: todas aleatorias, misma familia normal/Deep que la receptora.
    let (target_deep, size) = random_kind(data, req.target, Role::Target)?;
    let (donor_deep, _) = random_kind(data, req.donor, Role::Donor)?;
    if donor_deep != target_deep {
        return Err(TransferError::DeepMismatch(Role::Donor));
    }

    // Identidad: ninguna reliquia repetida en el pedido.
    let mut all: Vec<(Role, &Relic)> = vec![(Role::Donor, req.donor), (Role::Target, req.target)];
    all.extend(
        req.sacrifices
            .iter()
            .enumerate()
            .map(|(i, r)| (Role::Sacrifice(i), *r)),
    );
    for (i, (role_a, a)) in all.iter().enumerate() {
        for (role_b, b) in &all[i + 1..] {
            if std::ptr::eq(*a, *b) {
                return Err(TransferError::SameRelicTwice(*role_a, *role_b));
            }
        }
    }

    // Bloqueadas: la donante y los sacrificios se destruyen.
    if req.donor.locked {
        return Err(TransferError::Locked(Role::Donor));
    }
    for (i, s) in req.sacrifices.iter().enumerate() {
        if s.locked {
            return Err(TransferError::Locked(Role::Sacrifice(i)));
        }
    }

    // La receptora tiene que ser legal de entrada.
    validate(data, req.target).map_err(TransferError::TargetInvalid)?;

    // Slots.
    let moved = *req
        .donor
        .slots
        .get(req.donor_slot)
        .ok_or(TransferError::EmptyDonorSlot)?;
    let moved_effect = moved.effect.ok_or(TransferError::EmptyDonorSlot)?;
    if req.target_slot >= size {
        return Err(TransferError::InvalidTargetSlot);
    }
    let replaced_effect = req.target.slots[req.target_slot]
        .effect
        .expect("receptora validada: los primeros `size` slots tienen efecto");

    // Sacrificios: cantidad, tipo y puntos.
    if req.sacrifices.len() != rules.sacrifice_count {
        return Err(TransferError::WrongSacrificeCount {
            expected: rules.sacrifice_count,
            found: req.sacrifices.len(),
        });
    }
    let mut points = 0u32;
    for (i, s) in req.sacrifices.iter().enumerate() {
        let (deep, s_size) = random_kind(data, s, Role::Sacrifice(i))?;
        if deep != target_deep {
            return Err(TransferError::DeepMismatch(Role::Sacrifice(i)));
        }
        points += s_size as u32;
    }
    let needed = rules.min_points[size - 1];
    if points < needed {
        return Err(TransferError::NotEnoughPoints {
            needed,
            have: points,
        });
    }

    // Límite de forja, contado por ID de efecto (el orden de los slots cambia al normalizar).
    let present: Vec<EffectId> = req.target.effects().collect();
    let mut forged: Vec<EffectId> = req
        .target_forged
        .iter()
        .copied()
        .filter(|e| present.contains(e) && *e != replaced_effect)
        .collect();
    forged.push(moved_effect);
    forged.sort_unstable();
    forged.dedup();
    let limit = rules.forge_limit[size - 1];
    if forged.len() > limit {
        return Err(TransferError::ForgeLimitReached { limit });
    }

    // Murk.
    let mut murk_cost = rules.murk_cost[size - 1];
    if target_deep {
        murk_cost = murk_cost.saturating_mul(rules.deep_murk_multiplier);
    }
    if req.available_murk < murk_cost {
        return Err(TransferError::NotEnoughMurk {
            needed: murk_cost,
            have: req.available_murk,
        });
    }

    // Resultado: el efecto viaja con su maldición, se reordena y tiene que ser legal.
    let mut new_target = req.target.clone();
    new_target.slots[req.target_slot] = moved;
    normalize(data, &mut new_target);
    validate(data, &new_target).map_err(TransferError::ResultInvalid)?;

    Ok(TransferPlan {
        new_target,
        new_target_forged: forged,
        murk_cost,
        sacrifice_points: points,
    })
}

fn random_kind(data: &GameData, relic: &Relic, role: Role) -> Result<(bool, usize), TransferError> {
    match relic_kind(data, relic.item_id) {
        RelicKind::Random { deep, size } => Ok((deep, size)),
        _ => Err(TransferError::NotEligible(role)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relic::Slot;
    use crate::test_support::sample_data;

    fn grand(a: EffectId, b: EffectId, c: EffectId) -> Relic {
        Relic::new(
            1,
            [Slot::new(a, None), Slot::new(b, None), Slot::new(c, None)],
        )
    }

    fn small(a: EffectId) -> Relic {
        Relic::new(3, [Slot::new(a, None), Slot::EMPTY, Slot::EMPTY])
    }

    struct Fixture {
        donor: Relic,
        target: Relic,
        sacrifices: Vec<Relic>,
    }

    fn fixture() -> Fixture {
        Fixture {
            donor: grand(10, 14, 15),
            target: grand(11, 12, 16),
            sacrifices: vec![grand(10, 11, 12), grand(10, 11, 14), small(10)],
        }
    }

    fn request<'a>(
        f: &'a Fixture,
        sacs: &'a [&'a Relic],
        forged: &'a [EffectId],
    ) -> TransferRequest<'a> {
        TransferRequest {
            donor: &f.donor,
            donor_slot: 0, // efecto 10
            target: &f.target,
            target_slot: 2, // pisa el 16
            target_forged: forged,
            sacrifices: sacs,
            available_murk: 50_000,
        }
    }

    #[test]
    fn transferencia_valida() {
        let data = sample_data();
        let f = fixture();
        let sacs: Vec<&Relic> = f.sacrifices.iter().collect();
        let plan = plan_transfer(&data, &Rules::default(), &request(&f, &sacs, &[])).unwrap();
        // 10 se ordena primero.
        assert_eq!(plan.new_target, grand(10, 11, 12));
        assert_eq!(plan.new_target_forged, vec![10]);
        assert_eq!(plan.murk_cost, 10_000);
        assert_eq!(plan.sacrifice_points, 7);
        assert_eq!(validate(&data, &plan.new_target), Ok(()));
    }

    #[test]
    fn puntos_insuficientes() {
        let data = sample_data();
        let mut f = fixture();
        f.sacrifices = vec![small(10), small(11), grand(10, 11, 12)]; // 1 + 1 + 3 = 5
        let sacs: Vec<&Relic> = f.sacrifices.iter().collect();
        let err = plan_transfer(&data, &Rules::default(), &request(&f, &sacs, &[])).unwrap_err();
        assert_eq!(err, TransferError::NotEnoughPoints { needed: 7, have: 5 });
    }

    #[test]
    fn limite_de_forja_y_reforja() {
        let data = sample_data();
        let f = fixture();
        let sacs: Vec<&Relic> = f.sacrifices.iter().collect();
        // 11 y 12 ya forjados: forjar un tercero (pisando el 16) excede el límite de 2.
        let err =
            plan_transfer(&data, &Rules::default(), &request(&f, &sacs, &[11, 12])).unwrap_err();
        assert_eq!(err, TransferError::ForgeLimitReached { limit: 2 });

        // Reforjar un slot ya forjado (pisar el 12) sí se puede.
        let mut req = request(&f, &sacs, &[11, 12]);
        req.target_slot = 1;
        let plan = plan_transfer(&data, &Rules::default(), &req).unwrap();
        assert_eq!(plan.new_target_forged, vec![10, 11]);
    }

    #[test]
    fn efecto_ya_presente_o_incompatible() {
        let data = sample_data();
        let mut f = fixture();
        f.donor = grand(10, 13, 14); // 13 choca con el 12 de la receptora (compatibilityId 500)
        let sacs: Vec<&Relic> = f.sacrifices.iter().collect();
        let mut req = request(&f, &sacs, &[]);
        req.donor_slot = 1;
        let err = plan_transfer(&data, &Rules::default(), &req).unwrap_err();
        assert!(matches!(
            err,
            TransferError::ResultInvalid(Violation::CompatibilityClash { .. })
        ));
    }

    #[test]
    fn murk_insuficiente_y_bloqueadas() {
        let data = sample_data();
        let mut f = fixture();
        let sacs: Vec<&Relic> = f.sacrifices.iter().collect();
        let mut req = request(&f, &sacs, &[]);
        req.available_murk = 9_999;
        assert_eq!(
            plan_transfer(&data, &Rules::default(), &req).unwrap_err(),
            TransferError::NotEnoughMurk {
                needed: 10_000,
                have: 9_999
            }
        );

        f.donor.locked = true;
        let sacs: Vec<&Relic> = f.sacrifices.iter().collect();
        assert_eq!(
            plan_transfer(&data, &Rules::default(), &request(&f, &sacs, &[])).unwrap_err(),
            TransferError::Locked(Role::Donor)
        );
    }

    #[test]
    fn misma_reliquia_dos_veces() {
        let data = sample_data();
        let f = fixture();
        let sacs: Vec<&Relic> = vec![&f.donor, &f.sacrifices[1], &f.sacrifices[2]];
        assert_eq!(
            plan_transfer(&data, &Rules::default(), &request(&f, &sacs, &[])).unwrap_err(),
            TransferError::SameRelicTwice(Role::Donor, Role::Sacrifice(0))
        );
    }

    #[test]
    fn deep_lleva_su_maldicion_y_respeta_slots_potenciados() {
        let data = sample_data();
        let deep = |a: Slot, b: Slot| Relic::new(2, [a, b, Slot::EMPTY]);
        let donor = deep(Slot::new(21, Some(31)), Slot::new(23, None));
        let target = deep(Slot::new(20, Some(30)), Slot::new(22, None));
        let sacrifices = [
            deep(Slot::new(20, Some(30)), Slot::new(22, None)),
            deep(Slot::new(21, Some(31)), Slot::new(22, None)),
            deep(Slot::new(20, Some(30)), Slot::new(23, None)),
        ];
        let sacs: Vec<&Relic> = sacrifices.iter().collect();
        let base = TransferRequest {
            donor: &donor,
            donor_slot: 0, // 21 potenciado, con maldición 31
            target: &target,
            target_slot: 0, // pisa el potenciado 20
            target_forged: &[],
            sacrifices: &sacs,
            available_murk: 50_000,
        };

        // Potenciado sobre potenciado: viaja con su maldición. Murk ×2 por ser Deep.
        let plan = plan_transfer(&data, &Rules::default(), &base).unwrap();
        assert_eq!(plan.new_target.slots[0], Slot::new(21, Some(31)));
        assert_eq!(plan.murk_cost, 10_000);

        // Potenciado sobre el slot normal: quedarían 2 potenciados y el tipo tiene 1 solo.
        let mut req = base.clone();
        req.target_slot = 1;
        assert_eq!(
            plan_transfer(&data, &Rules::default(), &req).unwrap_err(),
            TransferError::ResultInvalid(Violation::NoValidAssignment)
        );

        // Una donante normal no puede aportar a una Deep.
        let normal = grand(10, 11, 12);
        let mut req = base.clone();
        req.donor = &normal;
        assert_eq!(
            plan_transfer(&data, &Rules::default(), &req).unwrap_err(),
            TransferError::DeepMismatch(Role::Donor)
        );
    }
}
