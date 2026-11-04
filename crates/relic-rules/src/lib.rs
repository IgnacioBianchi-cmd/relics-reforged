//! # relic-rules
//!
//! Motor de reglas de **Relics Reforged**, un mod de Elden Ring Nightreign que permite transferir
//! un efecto de una reliquia a otra.
//!
//! - [`data`]: datos del juego (efectos, tipos de reliquia, tablas de roll).
//! - [`relic`]: modelo de una reliquia y su clasificación (aleatoria o fija).
//! - [`validate`]: ¿podría el juego haber generado esta reliquia?
//! - [`transfer`]: evalúa una transferencia y calcula su costo.
//!
//! El motor no lee ni escribe memoria ni archivos del juego: es lógica pura y testeable.

pub mod data;
pub mod relic;
pub mod transfer;
pub mod validate;

pub use data::{DataError, EffectId, EffectInfo, GameData, ItemId, RelicType, TableId, EMPTY};
pub use relic::{relic_kind, Relic, RelicKind, Slot};
pub use transfer::{plan_transfer, Role, Rules, TransferError, TransferPlan, TransferRequest};
pub use validate::{normalize, validate, Violation};

#[cfg(test)]
mod test_support;
