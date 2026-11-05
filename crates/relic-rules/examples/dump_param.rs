//! Vuelca las filas de un PARAM de `regulation.bin` como enteros de 32 bits (para relevamiento).
//!
//! cargo run -p relic-rules --example dump_param -- <regulation.bin> <Nombre.param> <tamaño de fila>

use relic_rules::regulation::{bnd4_files, decompress_dcx, decrypt, param_rows};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let raw = std::fs::read(&args[0]).expect("leer regulation.bin");
    let row_size: usize = args[2].parse().expect("tamaño de fila");
    let bnd = decompress_dcx(&decrypt(&raw).expect("descifrar")).expect("descomprimir");
    let files = bnd4_files(&bnd).expect("BND4");
    let (_, param) = files
        .iter()
        .find(|(n, _)| n.rsplit(['\\', '/']).next() == Some(args[1].as_str()))
        .expect("no está ese PARAM");
    for (id, row) in param_rows(param, row_size).expect("filas") {
        let words: Vec<String> = row
            .chunks(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()).to_string())
            .collect();
        println!("{id}\t{}", words.join("\t"));
    }
}
