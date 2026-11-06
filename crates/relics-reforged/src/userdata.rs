//! Datos del mod que no van en su carpeta: `%APPDATA%\RelicsReforged\`.
//!
//! - `relics_reforged_forjas.txt`: el registro de forjas (ver [`forge`](crate::forge)). Antes
//!   estaba junto a la DLL; al arrancar se mueve acá si hace falta. Así sobrevive a una
//!   actualización del mod aunque se borre su carpeta.
//! - `backups\`: copias del save, una antes de la primera forja de cada sesión de juego. Se
//!   guardan las últimas [`KEEP_BACKUPS`].
//!
//! El save que se respalda es el último que escribió el juego en `%APPDATA%\Nightreign\<id>\`
//! (el normal `NR0000.sl2`, la copia de me3 o el de Seamless `NR0000.co2`): los saves pesan
//! ~20 MB y los demás archivos de la carpeta (`.bak`, `steam_autocloud.vdf`) no se miran.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::SystemTime;

use crate::forge::FORGED_FILE;

const KEEP_BACKUPS: usize = 10;
/// Los saves de Nightreign pesan ~20 MB: algo mucho más chico no es un save.
const MIN_SAVE_SIZE: u64 = 1 << 20;

static DIR: OnceLock<PathBuf> = OnceLock::new();
/// Ya se respaldó el save en esta sesión.
static BACKED_UP: AtomicBool = AtomicBool::new(false);

fn appdata() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(PathBuf::from)
}

/// La carpeta de datos (`%APPDATA%\RelicsReforged`). Si no se puede usar, la de la DLL.
pub fn dir() -> PathBuf {
    DIR.get().cloned().unwrap_or_else(|| PathBuf::from("."))
}

/// Crea la carpeta de datos y mueve ahí el registro de forjas si estaba junto a la DLL.
/// Devuelve una línea para el log.
pub fn init(dll_dir: &Path) -> String {
    let dir = appdata()
        .map(|a| a.join("RelicsReforged"))
        .filter(|d| std::fs::create_dir_all(d).is_ok())
        .unwrap_or_else(|| dll_dir.to_path_buf());
    let _ = DIR.set(dir.clone());
    let old = dll_dir.join(FORGED_FILE);
    let new = dir.join(FORGED_FILE);
    let moved = if old != new && old.exists() && !new.exists() {
        match std::fs::copy(&old, &new) {
            Ok(_) => {
                // Se deja el viejo renombrado, por las dudas.
                let _ = std::fs::rename(&old, old.with_extension("txt.migrado"));
                format!(" (registro de forjas movido desde {})", old.display())
            }
            Err(e) => format!(" (no pude mover el registro de forjas: {e})"),
        }
    } else {
        String::new()
    };
    format!("Datos del mod en {}{moved}", dir.display())
}

/// El save que escribió el juego más recientemente.
fn current_save() -> Option<PathBuf> {
    let root = appdata()?.join("Nightreign");
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for user in std::fs::read_dir(root).ok()?.flatten() {
        let Ok(files) = std::fs::read_dir(user.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            let name = file.file_name().to_string_lossy().to_ascii_lowercase();
            let Ok(meta) = file.metadata() else {
                continue;
            };
            if !meta.is_file() || meta.len() < MIN_SAVE_SIZE || name.ends_with(".bak") {
                continue;
            }
            let Ok(modified) = meta.modified() else {
                continue;
            };
            if newest.as_ref().is_none_or(|(t, _)| modified > *t) {
                newest = Some((modified, path));
            }
        }
    }
    newest.map(|(_, p)| p)
}

#[repr(C)]
#[derive(Default)]
struct SystemTimeWin {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    millis: u16,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLocalTime(time: *mut SystemTimeWin);
}

/// Fecha y hora local para el nombre del respaldo: `2026-11-06_01-30-05`.
fn stamp() -> String {
    let mut t = SystemTimeWin::default();
    // SAFETY: GetLocalTime solo escribe en `t`.
    unsafe { GetLocalTime(&mut t) };
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

/// Antes de la primera forja de la sesión: copia el save actual a `backups\`. `Ok(None)` si ya
/// se hizo en esta sesión.
pub fn backup_save_once() -> Result<Option<PathBuf>, String> {
    if BACKED_UP.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let save = current_save().ok_or("no encontré el save en %APPDATA%\\Nightreign")?;
    let backups = dir().join("backups");
    std::fs::create_dir_all(&backups).map_err(|e| e.to_string())?;
    let name = save
        .file_name()
        .map_or("save".into(), |n| n.to_string_lossy().into_owned());
    let dest = backups.join(format!("{}_{name}", stamp()));
    std::fs::copy(&save, &dest).map_err(|e| format!("{}: {e}", save.display()))?;
    BACKED_UP.store(true, Ordering::SeqCst);
    // Solo los últimos KEEP_BACKUPS (el nombre empieza con la fecha: el orden alfabético sirve).
    // Solo se miran los respaldos propios: lo demás que haya en la carpeta no se toca.
    if let Ok(entries) = std::fs::read_dir(&backups) {
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(|c: char| c.is_ascii_digit()))
            .map(|e| e.path())
            .collect();
        files.sort();
        let extra = files.len().saturating_sub(KEEP_BACKUPS);
        for old in &files[..extra] {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(Some(dest))
}
