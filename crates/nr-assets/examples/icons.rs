//! Arma el atlas de íconos fuera del juego y lo guarda como PNG, para revisarlo.
//!
//! cargo run -p nr-assets --example icons -- <carpeta Game> <salida.png> NOMBRE...

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (game, out, names) = (Path::new(&args[0]), &args[1], &args[2..]);
    let start = std::time::Instant::now();
    let atlas = nr_assets::load_menu_icons(game, names).expect("leer íconos");
    println!(
        "{}x{}, {} de {} íconos en {:?}",
        atlas.width,
        atlas.height,
        atlas.rects.len(),
        names.len(),
        start.elapsed()
    );
    let file = std::fs::File::create(out).expect("crear PNG");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), atlas.width, atlas.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .and_then(|mut w| w.write_image_data(&atlas.rgba))
        .expect("escribir PNG");
}
