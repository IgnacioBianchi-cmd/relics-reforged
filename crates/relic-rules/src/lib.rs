//! # relic-rules
//!
//! Motor de reglas de **Relics Reforged**, un mod de Elden Ring Nightreign que permite transferir
//! un efecto de una reliquia a otra.
//!
//! - [`data`]: datos del juego (efectos, tipos de reliquia, tablas de roll).
//! - [`relic`]: modelo de una reliquia y su clasificación (aleatoria o fija).
//! - [`validate`]: ¿podría el juego haber generado esta reliquia?
//! - [`transfer`]: evalúa una transferencia y calcula su costo.
//! - [`regulation`]: lee los params desde el `regulation.bin` del juego.
//!
//! El motor no lee ni escribe la memoria del juego: es lógica pura y testeable.

pub mod data;
pub mod lang;
pub mod regulation;
pub mod relic;
pub mod transfer;
pub mod validate;

pub use data::{
    DataError, EffectFilters, EffectId, EffectInfo, FilterCategory, FilterFamily, FilterGroup,
    GameData, ItemId, RelicType, TableId, EMPTY,
};
pub use regulation::RegulationError;
pub use relic::{relic_kind, Relic, RelicKind, Slot};
pub use transfer::{plan_transfer, Role, Rules, TransferError, TransferPlan, TransferRequest};
pub use validate::{normalize, validate, Violation};

#[cfg(test)]
mod test_support;
