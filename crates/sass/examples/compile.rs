//! Compila o SCSS da entrada padrão nos dois estilos e mostra o CSS e o
//! mapa (como `asset:p/web/a.scss`), para comparar à mão com o dart-sass.
use dartforge_sass::sass_builder::{compilar, Ativo, Disco, Pacotes};
use dartforge_sass::{OutputStyle, VersaoDartSass};

/// A versão imitada: `SASS_VERSAO` (`1.66.0` ou `1.102.0`, o padrão).
fn versao() -> VersaoDartSass {
    std::env::var("SASS_VERSAO")
        .ok()
        .and_then(|v| VersaoDartSass::do_lock(&v))
        .unwrap_or_default()
}
use std::io::Read;

fn main() {
    let mut fonte = String::new();
    std::io::stdin()
        .read_to_string(&mut fonte)
        .expect("entrada");
    let entrada = Ativo {
        pacote: "p".into(),
        caminho: "web/a.scss".into(),
    };
    for (estilo, nome) in [
        (OutputStyle::Expanded, "expanded"),
        (OutputStyle::Compressed, "compressed"),
    ] {
        match compilar(
            &fonte,
            &entrada,
            &Pacotes::new(),
            &Disco,
            estilo,
            true,
            versao(),
        ) {
            Ok(c) => println!("--- {nome}\n{}\nMAP {}", c.css, c.mapa.unwrap_or_default()),
            Err(e) => println!("ERR {e}"),
        }
    }
}
