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

/// Un número con separador de miles: `152.440` en español, `152,440` en inglés.
pub fn number(n: u32) -> String {
    let sep = match get() {
        Lang::Es => '.',
        Lang::En => ',',
    };
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// El texto en el idioma actual.
pub fn tr<'a>(es: &'a str, en: &'a str) -> &'a str {
    match get() {
        Lang::Es => es,
        Lang::En => en,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separador_de_miles() {
        // Ningún test cambia el idioma: queda el de por defecto (inglés).
        assert_eq!(number(0), "0");
        assert_eq!(number(999), "999");
        assert_eq!(number(2_500), "2,500");
        assert_eq!(number(152_440), "152,440");
        assert_eq!(number(1_000_000), "1,000,000");
    }
}
