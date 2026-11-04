//! Idioma de los textos que ve el jugador: español o inglés, según el idioma del juego.
//! El mod lo fija al arrancar ([`set`]); por defecto, inglés.

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Es,
    En,
}

static LANG: AtomicU8 = AtomicU8::new(Lang::En as u8);

pub fn set(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn get() -> Lang {
    if LANG.load(Ordering::Relaxed) == Lang::Es as u8 {
        Lang::Es
    } else {
        Lang::En
    }
}

/// El texto en el idioma actual.
pub fn tr<'a>(es: &'a str, en: &'a str) -> &'a str {
    match get() {
        Lang::Es => es,
        Lang::En => en,
    }
}
