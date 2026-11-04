//! Análisis estático del volcado de `nightreign.exe` que genera el comando `dumpimage` de la DLL.
//! Como es una imagen en memoria, cada RVA es directamente un offset del archivo.
//!
//! ```text
//! disasm <volcado> fn <rva>...           función que contiene cada RVA (según .pdata), desensamblada
//! disasm <volcado> at <rva> <antes> <después>   bytes alrededor de una RVA, sin buscar la función
//! disasm <volcado> callers <rva>...      instrucciones `call rel32` que llaman a esa RVA
//! ```
//!
//! Las RVAs van en hex (con o sin `0x`, o como `nr+69E73B`).

use std::fmt::Write as _;

use iced_x86::{
    Decoder, DecoderOptions, FlowControl, Formatter, Instruction, IntelFormatter, OpKind,
};

struct Image {
    bytes: Vec<u8>,
    /// (inicio, fin) de cada función de `.pdata`, ordenadas.
    functions: Vec<(u32, u32)>,
    /// (inicio, fin) de las secciones ejecutables.
    code: Vec<(u32, u32)>,
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}

fn u16_at(b: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}

impl Image {
    fn load(path: &str) -> Image {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("no pude leer {path}: {e}"));
        let nt = u32_at(&bytes, 0x3C) as usize;
        assert_eq!(u32_at(&bytes, nt), 0x4550, "no es una imagen PE");
        let sections = u16_at(&bytes, nt + 6) as usize;
        let opt_size = u16_at(&bytes, nt + 20) as usize;
        let opt = nt + 24;
        // Directorio 3 = excepciones (.pdata).
        let pdata_rva = u32_at(&bytes, opt + 112 + 3 * 8) as usize;
        let pdata_len = u32_at(&bytes, opt + 112 + 3 * 8 + 4) as usize;
        let mut functions: Vec<(u32, u32)> = (0..pdata_len / 12)
            .map(|i| {
                let e = pdata_rva + i * 12;
                (u32_at(&bytes, e), u32_at(&bytes, e + 4))
            })
            .filter(|&(s, e)| s < e)
            .collect();
        functions.sort_unstable();
        let mut code = Vec::new();
        for i in 0..sections {
            let s = opt + opt_size + i * 40;
            let vsize = u32_at(&bytes, s + 8);
            let rva = u32_at(&bytes, s + 12);
            let flags = u32_at(&bytes, s + 36);
            if flags & 0x2000_0000 != 0 {
                code.push((rva, rva + vsize));
            }
        }
        Image {
            bytes,
            functions,
            code,
        }
    }

    fn function_of(&self, rva: u32) -> Option<(u32, u32)> {
        let i = self.functions.partition_point(|&(s, _)| s <= rva);
        let (s, e) = *self.functions.get(i.checked_sub(1)?)?;
        (rva < e).then_some((s, e))
    }

    fn disasm(&self, start: u32, end: u32, mark: &[u32]) -> String {
        let end = end.min(self.bytes.len() as u32);
        let code = &self.bytes[start as usize..end as usize];
        let mut decoder = Decoder::with_ip(64, code, start as u64, DecoderOptions::NONE);
        let mut fmt = IntelFormatter::new();
        fmt.options_mut().set_rip_relative_addresses(true);
        let mut out = String::new();
        let mut text = String::new();
        let mut instr = Instruction::default();
        while decoder.can_decode() {
            decoder.decode_out(&mut instr);
            text.clear();
            fmt.format(&instr, &mut text);
            let rva = instr.ip() as u32;
            let marker = if mark.contains(&rva) { ">>" } else { "  " };
            let hex: String = code[(rva - start) as usize..(rva - start) as usize + instr.len()]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect();
            let _ = write!(out, "{marker} {rva:08X}  {hex:<24} {text}");
            if let Some(note) = note(&instr) {
                let _ = write!(out, "    ; {note}");
            }
            out.push('\n');
        }
        out
    }

    fn callers(&self, target: u32) -> Vec<u32> {
        let mut out = Vec::new();
        for &(s, e) in &self.code {
            let e = e.min(self.bytes.len() as u32);
            for rva in s..e.saturating_sub(5) {
                if self.bytes[rva as usize] == 0xE8 {
                    let rel = u32_at(&self.bytes, rva as usize + 1) as i32;
                    if (rva as i64 + 5 + rel as i64) == target as i64 {
                        out.push(rva);
                    }
                }
            }
        }
        out
    }
}

/// Anota destinos de saltos/llamadas y direcciones RIP-relativas como `nr+RVA`.
fn note(instr: &Instruction) -> Option<String> {
    match instr.flow_control() {
        FlowControl::Call | FlowControl::UnconditionalBranch | FlowControl::ConditionalBranch
            if instr.op0_kind() == OpKind::NearBranch64 =>
        {
            return Some(format!("-> nr+{:X}", instr.near_branch_target()));
        }
        _ => {}
    }
    instr
        .is_ip_rel_memory_operand()
        .then(|| format!("[nr+{:X}]", instr.ip_rel_memory_address()))
}

fn parse_rva(s: &str) -> u32 {
    let s = s.trim_start_matches("nr+").trim_start_matches("0x");
    u32::from_str_radix(s, 16).unwrap_or_else(|_| panic!("RVA inválida: {s}"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path, cmd, rest @ ..] = args.as_slice() else {
        eprintln!("uso: disasm <volcado> fn|at|callers <rva>...");
        std::process::exit(2);
    };
    let image = Image::load(path);
    eprintln!(
        "{} funciones en .pdata, {} secciones de código",
        image.functions.len(),
        image.code.len()
    );
    match cmd.as_str() {
        "fn" => {
            let marks: Vec<u32> = rest.iter().map(|s| parse_rva(s)).collect();
            let mut done = Vec::new();
            for &rva in &marks {
                match image.function_of(rva) {
                    Some((s, e)) if !done.contains(&s) => {
                        done.push(s);
                        println!(
                            "===== función nr+{s:X} .. nr+{e:X} ({} bytes), contiene nr+{rva:X} =====",
                            e - s
                        );
                        print!("{}", image.disasm(s, e, &marks));
                    }
                    Some(_) => {}
                    None => println!("===== nr+{rva:X}: no está en .pdata ====="),
                }
            }
        }
        "at" => {
            let rva = parse_rva(&rest[0]);
            let before: u32 = rest.get(1).map_or(0x40, |s| s.parse().unwrap());
            let after: u32 = rest.get(2).map_or(0x40, |s| s.parse().unwrap());
            print!("{}", image.disasm(rva - before, rva + after, &[rva]));
        }
        "callers" => {
            for target in rest.iter().map(|s| parse_rva(s)) {
                let calls = image.callers(target);
                println!("nr+{target:X}: {} llamadas directas", calls.len());
                for c in calls {
                    let f = image
                        .function_of(c)
                        .map_or("?".to_string(), |(s, _)| format!("nr+{s:X}"));
                    println!("  nr+{c:X} (en {f})");
                }
            }
        }
        _ => {
            eprintln!("comando desconocido: {cmd}");
            std::process::exit(2);
        }
    }
}
