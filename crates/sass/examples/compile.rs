//! Compiles the SCSS from standard input in both styles and shows the CSS
//! and the source map (as `asset:p/web/a.scss`), to compare by hand with
//! dart-sass.
use dartforge_sass::sass_builder::{compilar, Ativo, Disco, Pacotes};
use dartforge_sass::{OutputStyle, VersaoDartSass};

/// The imitated version: `SASS_VERSION` (`1.66.0` or `1.102.0`, the
/// default).
fn version() -> VersaoDartSass {
    std::env::var("SASS_VERSION")
        .ok()
        .and_then(|v| VersaoDartSass::do_lock(&v))
        .unwrap_or_default()
}
use std::io::Read;

fn main() {
    let mut source = String::new();
    std::io::stdin()
        .read_to_string(&mut source)
        .expect("input");
    let input = Ativo {
        pacote: "p".into(),
        caminho: "web/a.scss".into(),
    };
    for (style, name) in [
        (OutputStyle::Expanded, "expanded"),
        (OutputStyle::Compressed, "compressed"),
    ] {
        match compilar(
            &source,
            &input,
            &Pacotes::new(),
            &Disco,
            style,
            true,
            version(),
        ) {
            Ok(c) => println!("--- {name}\n{}\nMAP {}", c.css, c.mapa.unwrap_or_default()),
            Err(e) => println!("ERR {e}"),
        }
    }
}
