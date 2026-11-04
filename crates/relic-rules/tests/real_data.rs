//! Tests contra datos reales: los CSV de Smithbox (`data/export/`) y las reliquias de un save
//! (`data/save/relics.json`, generado con `tools/relic_survey.py`).
//!
//! Esos archivos no van en git (son del juego y del jugador). Si faltan, los tests avisan y pasan.

use std::path::PathBuf;

use relic_rules::*;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

struct SaveRelic {
    kind_label: String,
    name: String,
    relic: Relic,
}

fn load() -> Option<(GameData, Vec<SaveRelic>)> {
    let root = repo_root();
    let export = root.join("data/export");
    let save = root.join("data/save/relics.json");
    if !export.join("AttachEffectParam.csv").exists() || !save.exists() {
        eprintln!(
            "(sin datos reales en {}: se saltea el test)",
            root.display()
        );
        return None;
    }
    let data = GameData::from_csv_dir(&export).expect("cargar CSV");
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&save).unwrap()).expect("leer relics.json");
    let arr = |v: &serde_json::Value| -> [u32; 4] {
        let a = v.as_array().unwrap();
        std::array::from_fn(|i| a[i].as_u64().unwrap() as u32)
    };
    let relics = raw
        .as_array()
        .unwrap()
        .iter()
        .map(|r| SaveRelic {
            kind_label: r["type"].as_str().unwrap().to_string(),
            name: r["item_name"].as_str().unwrap().to_string(),
            relic: Relic::from_raw(
                r["item"].as_u64().unwrap() as u32,
                arr(&r["effects"]),
                arr(&r["curses"]),
            ),
        })
        .collect();
    Some((data, relics))
}

#[test]
fn datos_cargan_completos() {
    let Some((data, _)) = load() else { return };
    assert!(data.effects.len() > 2000, "efectos: {}", data.effects.len());
    assert!(
        data.relic_types.len() > 1300,
        "tipos: {}",
        data.relic_types.len()
    );
    assert!(data.tables.len() > 1000, "tablas: {}", data.tables.len());
}

/// Toda reliquia aleatoria que generó el juego tiene que pasar `validate`.
#[test]
fn todas_las_reliquias_naturales_son_legales() {
    let Some((data, relics)) = load() else { return };
    let mut random = 0;
    let mut failures = Vec::new();
    for r in &relics {
        let kind = relic_kind(&data, r.relic.item_id);
        if r.kind_label == "UniqueRelic" {
            assert!(
                !kind.is_random(),
                "{} figura como única pero es aleatoria",
                r.name
            );
            continue;
        }
        if !kind.is_random() {
            continue;
        }
        random += 1;
        if let Err(v) = validate(&data, &r.relic) {
            failures.push(format!("{}: {v} ({:?})", r.name, r.relic.slots));
        }
    }
    eprintln!("reliquias aleatorias validadas: {random}");
    assert!(random > 0);
    assert!(
        failures.is_empty(),
        "reliquias naturales rechazadas:\n{}",
        failures.join("\n")
    );
}

/// Simula transferencias entre reliquias reales: cada plan aceptado tiene que dar una reliquia legal,
/// y tiene que haber al menos una transferencia posible.
#[test]
fn transferencias_entre_reliquias_reales() {
    let Some((data, relics)) = load() else { return };
    let grands: Vec<&Relic> = relics
        .iter()
        .map(|r| &r.relic)
        .filter(|r| {
            relic_kind(&data, r.item_id)
                == RelicKind::Random {
                    deep: false,
                    size: 3,
                }
        })
        .collect();
    assert!(grands.len() >= 5, "hacen falta al menos 5 grandes normales");

    let rules = Rules::default();
    let (mut ok, mut rejected) = (0, 0);
    for (di, donor) in grands.iter().enumerate() {
        for (ti, target) in grands.iter().enumerate() {
            if di == ti {
                continue;
            }
            let sacs: Vec<&Relic> = grands
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != di && *i != ti)
                .map(|(_, r)| *r)
                .take(3)
                .collect();
            for donor_slot in 0..3 {
                for target_slot in 0..3 {
                    let req = TransferRequest {
                        donor,
                        donor_slot,
                        target,
                        target_slot,
                        target_forged: &[],
                        sacrifices: &sacs,
                        available_murk: 1_000_000,
                    };
                    match plan_transfer(&data, &rules, &req) {
                        Ok(plan) => {
                            ok += 1;
                            assert_eq!(validate(&data, &plan.new_target), Ok(()));
                            assert!(plan
                                .new_target
                                .effects()
                                .any(|e| Some(e) == donor.slots[donor_slot].effect));
                        }
                        Err(TransferError::ResultInvalid(_)) => rejected += 1,
                        Err(e) => panic!("error inesperado: {e}"),
                    }
                }
            }
        }
    }
    eprintln!("transferencias simuladas: {ok} válidas, {rejected} rechazadas por reglas del juego");
    assert!(ok > 0);
}
